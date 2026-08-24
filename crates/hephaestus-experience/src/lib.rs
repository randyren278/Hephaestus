//! Redacted, bounded, provenance-aware execution evidence.

mod error;
mod integration;
mod record;
mod recorder;
mod redaction;
mod run_result;

pub use error::ExperienceError;
pub use integration::RecordedRuntime;
pub use record::{
    ExperienceInput, ExperienceKind, ExperienceReceipt, Provenance, TraceInput, TraceKind,
    TraceReceipt,
};
pub use recorder::{EvidenceRecorder, RetentionLimits};
pub use redaction::RedactionPolicy;
pub use run_result::{
    RUN_RESULT_SCHEMA_VERSION, RunBudgetReceipt, RunCompletionReason, RunResultReceipt,
    RunResultSigner, RunResultVerifier,
};
