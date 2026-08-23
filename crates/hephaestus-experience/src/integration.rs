use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use hephaestus_runtime::{
    AdapterCapabilities, CapabilityToken, Provider, RunHandle, RunSnapshot, RunSpec,
    RuntimeAdapter, RuntimeError, Sandbox,
};

use crate::{EvidenceRecorder, Provenance, TraceInput, TraceKind};

/// Runtime adapter decorator that makes observable execution evidence mandatory.
pub struct RecordedRuntime<R> {
    inner: R,
    evidence: EvidenceRecorder,
    runs: BTreeMap<String, RecordedRun>,
    completed: BTreeSet<String>,
    sequence: u64,
}

struct RecordedRun {
    provenance: Provenance,
    started: Instant,
}

impl<R> RecordedRuntime<R> {
    /// Wraps a runtime with a durable evidence recorder.
    ///
    /// # Errors
    ///
    /// Rejects an evidence ledger that cannot be replayed and verified.
    pub fn new(inner: R, evidence: EvidenceRecorder) -> Result<Self, RuntimeError> {
        let sequence = u64::try_from(evidence.replay_verified().map_err(evidence_error)?.len())
            .map_err(|_| RuntimeError::Evidence("evidence sequence exceeds u64".to_owned()))?;
        Ok(Self {
            inner,
            evidence,
            runs: BTreeMap::new(),
            completed: BTreeSet::new(),
            sequence,
        })
    }

    /// Records a provider-visible event against the immutable provenance of an active run.
    ///
    /// This is the ingestion boundary for tool calls/results, context metadata, memory IDs,
    /// subagent edges, file/test activity, denials, costs, retries, and model responses.
    /// Hidden chain-of-thought is neither requested nor accepted as a special event class.
    ///
    /// # Errors
    ///
    /// Rejects unknown runs and propagates redaction, retention, or durable storage failures.
    pub fn record_observable(
        &mut self,
        run_id: &str,
        kind: TraceKind,
        fields: BTreeMap<String, String>,
    ) -> Result<(), RuntimeError> {
        let provenance = self
            .runs
            .get(run_id)
            .map(|run| run.provenance.clone())
            .ok_or(RuntimeError::InvalidSpec("run does not exist"))?;
        self.record(provenance, kind, fields)
    }

    /// Returns the wrapped provider-neutral runtime.
    #[must_use]
    pub const fn inner(&self) -> &R {
        &self.inner
    }

    /// Returns mutable access to the wrapped runtime for diagnostics and composition.
    #[must_use]
    pub const fn inner_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Returns the durable recorder for verified replay and artifact reads.
    #[must_use]
    pub const fn evidence(&self) -> &EvidenceRecorder {
        &self.evidence
    }

    fn record(
        &mut self,
        provenance: Provenance,
        kind: TraceKind,
        fields: BTreeMap<String, String>,
    ) -> Result<(), RuntimeError> {
        let timestamp_millis = unix_millis()?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| RuntimeError::Evidence("evidence sequence overflow".to_owned()))?;
        let identity = format!(
            "{}:{}:{kind:?}:{timestamp_millis}",
            provenance.run_id(),
            self.sequence
        );
        let event_id = format!("trace-{}", blake3::hash(identity.as_bytes()).to_hex());
        let input = TraceInput::new(event_id, provenance, kind, timestamp_millis, fields)
            .map_err(evidence_error)?;
        self.evidence.record_trace(input).map_err(evidence_error)?;
        Ok(())
    }

    fn record_failure(
        &mut self,
        provenance: Provenance,
        operation: &'static str,
        error: &RuntimeError,
    ) -> Result<(), RuntimeError> {
        let kind = if matches!(error, RuntimeError::CapabilityDenied) {
            TraceKind::CapabilityDenied
        } else {
            TraceKind::Error
        };
        self.record(
            provenance,
            kind,
            BTreeMap::from([
                ("operation".to_owned(), operation.to_owned()),
                ("error".to_owned(), error.to_string()),
            ]),
        )
    }
}

impl<R: RuntimeAdapter> RuntimeAdapter for RecordedRuntime<R> {
    fn provider(&self) -> Provider {
        self.inner.provider()
    }

    fn report_capabilities(&self) -> AdapterCapabilities {
        self.inner.report_capabilities()
    }

    fn start(
        &mut self,
        spec: &RunSpec,
        sandbox: &Sandbox,
        token: &CapabilityToken,
    ) -> Result<RunHandle, RuntimeError> {
        let provenance = Provenance::new(spec.run_id(), spec.genome_id(), spec.world_id())
            .map_err(evidence_error)?;
        let handle = match self.inner.start(spec, sandbox, token) {
            Ok(handle) => handle,
            Err(error) => {
                self.record_failure(provenance, "start", &error)?;
                return Err(error);
            }
        };
        self.runs.insert(
            spec.run_id().to_owned(),
            RecordedRun {
                provenance: provenance.clone(),
                started: Instant::now(),
            },
        );
        let fields = BTreeMap::from([
            (
                "provider".to_owned(),
                format!("{:?}", self.inner.provider()),
            ),
            (
                "maximum_output_bytes".to_owned(),
                spec.budget().maximum_output_bytes().to_string(),
            ),
            (
                "maximum_cost_microusd".to_owned(),
                spec.budget().maximum_cost_microusd().to_string(),
            ),
            (
                "wall_budget_millis".to_owned(),
                spec.budget().wall().as_millis().to_string(),
            ),
        ]);
        if let Err(error) = self.record(provenance, TraceKind::LifecycleStarted, fields) {
            let _ignored = self.inner.interrupt(spec.run_id());
            self.runs.remove(spec.run_id());
            return Err(error);
        }
        Ok(handle)
    }

    fn resume(
        &mut self,
        spec: &RunSpec,
        sandbox: &Sandbox,
        token: &CapabilityToken,
        checkpoint: &str,
    ) -> Result<RunHandle, RuntimeError> {
        let provenance = Provenance::new(spec.run_id(), spec.genome_id(), spec.world_id())
            .map_err(evidence_error)?;
        let handle = match self.inner.resume(spec, sandbox, token, checkpoint) {
            Ok(handle) => handle,
            Err(error) => {
                self.record_failure(provenance, "resume", &error)?;
                return Err(error);
            }
        };
        self.runs.insert(
            spec.run_id().to_owned(),
            RecordedRun {
                provenance: provenance.clone(),
                started: Instant::now(),
            },
        );
        self.completed.remove(spec.run_id());
        if let Err(error) = self.record(
            provenance,
            TraceKind::CheckpointCreated,
            BTreeMap::from([(
                "checkpoint_hash".to_owned(),
                blake3::hash(checkpoint.as_bytes()).to_hex().to_string(),
            )]),
        ) {
            let _ignored = self.inner.interrupt(spec.run_id());
            self.runs.remove(spec.run_id());
            return Err(error);
        }
        Ok(handle)
    }

    fn interrupt(&mut self, run_id: &str) -> Result<(), RuntimeError> {
        let run = self
            .runs
            .get(run_id)
            .ok_or(RuntimeError::InvalidSpec("run does not exist"))?;
        let provenance = run.provenance.clone();
        let latency_millis = run.started.elapsed().as_millis().to_string();
        if let Err(error) = self.inner.interrupt(run_id) {
            self.record_failure(provenance, "interrupt", &error)?;
            return Err(error);
        }
        if !self.completed.contains(run_id) {
            self.record(
                provenance,
                TraceKind::LifecycleCompleted,
                BTreeMap::from([
                    ("status".to_owned(), "interrupted".to_owned()),
                    (
                        "completion_reason".to_owned(),
                        "operator_interrupt".to_owned(),
                    ),
                    ("latency_millis".to_owned(), latency_millis),
                ]),
            )?;
            self.completed.insert(run_id.to_owned());
        }
        Ok(())
    }

    fn snapshot(&mut self, run_id: &str) -> Result<RunSnapshot, RuntimeError> {
        let run = self
            .runs
            .get(run_id)
            .ok_or(RuntimeError::InvalidSpec("run does not exist"))?;
        let provenance = run.provenance.clone();
        let latency_millis = run.started.elapsed().as_millis().to_string();
        let snapshot = match self.inner.snapshot(run_id) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.record_failure(provenance, "snapshot", &error)?;
                return Err(error);
            }
        };
        if snapshot.status == hephaestus_runtime::RunStatus::Running {
            self.record(
                provenance,
                TraceKind::CheckpointCreated,
                BTreeMap::from([("status".to_owned(), "running".to_owned())]),
            )?;
        } else if !self.completed.contains(run_id) {
            let mut fields = BTreeMap::from([
                ("status".to_owned(), format!("{:?}", snapshot.status)),
                (
                    "completion_reason".to_owned(),
                    completion_reason(snapshot.status).to_owned(),
                ),
                ("latency_millis".to_owned(), latency_millis),
            ]);
            if let Some(exit_code) = snapshot.exit_code {
                fields.insert("exit_code".to_owned(), exit_code.to_string());
            }
            self.record(provenance.clone(), TraceKind::LifecycleCompleted, fields)?;
            if self.inner.provider() == Provider::Deterministic {
                self.record(
                    provenance,
                    TraceKind::CostObserved,
                    BTreeMap::from([("actual_microusd".to_owned(), "0".to_owned())]),
                )?;
            }
            self.completed.insert(run_id.to_owned());
        }
        Ok(snapshot)
    }
}

fn unix_millis() -> Result<i64, RuntimeError> {
    unix_millis_at(SystemTime::now())
}

fn unix_millis_at(now: SystemTime) -> Result<i64, RuntimeError> {
    let millis = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RuntimeError::Evidence("system clock precedes Unix epoch".to_owned()))?
        .as_millis();
    i64::try_from(millis)
        .map_err(|_| RuntimeError::Evidence("system clock exceeds i64 milliseconds".to_owned()))
}

const fn completion_reason(status: hephaestus_runtime::RunStatus) -> &'static str {
    match status {
        hephaestus_runtime::RunStatus::Running => "running",
        hephaestus_runtime::RunStatus::Succeeded => "success",
        hephaestus_runtime::RunStatus::Failed => "provider_failure",
        hephaestus_runtime::RunStatus::Interrupted => "operator_interrupt",
        hephaestus_runtime::RunStatus::TimedOut => "wall_budget_exceeded",
    }
}

fn evidence_error(error: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::Evidence(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;
    use crate::{RedactionPolicy, RetentionLimits};

    #[test]
    fn clock_status_and_sequence_edges_fail_or_map_explicitly() {
        assert!(unix_millis_at(UNIX_EPOCH - Duration::from_secs(1)).is_err());
        assert!(unix_millis_at(UNIX_EPOCH + Duration::from_secs(i64::MAX as u64)).is_err());
        assert_eq!(
            [
                hephaestus_runtime::RunStatus::Running,
                hephaestus_runtime::RunStatus::Succeeded,
                hephaestus_runtime::RunStatus::Failed,
                hephaestus_runtime::RunStatus::Interrupted,
                hephaestus_runtime::RunStatus::TimedOut,
            ]
            .map(completion_reason),
            [
                "running",
                "success",
                "provider_failure",
                "operator_interrupt",
                "wall_budget_exceeded",
            ]
        );

        let directory = tempdir().expect("evidence directory");
        let recorder = EvidenceRecorder::open(
            directory.path().join("events.sqlite3"),
            directory.path().join("artifacts"),
            RedactionPolicy::new([]),
            RetentionLimits::new(1, 1_024).expect("retention limits"),
        )
        .expect("open evidence recorder");
        let mut runtime = RecordedRuntime::new((), recorder).expect("create runtime");
        runtime.sequence = u64::MAX;
        assert!(matches!(
            runtime.record(
                Provenance::new("run", "genome", "world").expect("provenance"),
                TraceKind::Error,
                BTreeMap::new()
            ),
            Err(RuntimeError::Evidence(_))
        ));
    }
}
