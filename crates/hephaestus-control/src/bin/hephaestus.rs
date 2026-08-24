use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use hephaestus_control::{
    ApiResponse, Client, Command, EvaluationRecord, ResponseData, data_dir_from_environment,
};

#[derive(Parser)]
#[command(name = "hephaestus", about = "Hephaestus operator CLI")]
struct Arguments {
    /// Canonical daemon data directory.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Emit the stable API response as JSON.
    #[arg(long)]
    json: bool,
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Show daemon state.
    Status,
    /// Stop new evolution work.
    Freeze,
    /// Resume evolution through the operator boundary.
    Unfreeze,
    /// Terminate all active work.
    Kill {
        /// Confirm that every active run is targeted.
        #[arg(long)]
        all: bool,
    },
    /// Inspect immutable Genome records.
    Genome {
        #[command(subcommand)]
        command: GenomeCommand,
    },
    /// Execute one registered Genome with the offline reference runtime.
    Run {
        /// Content-derived registered Genome identity.
        genome_id: String,
    },
    /// Execute one registered Genome for a World-bound evaluation task.
    Evaluate {
        genome_id: String,
        #[arg(long)]
        task_id: String,
        #[arg(long)]
        input: String,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        #[arg(long, default_value_t = 10_000)]
        wall_millis: u64,
        #[arg(long, default_value_t = 1_048_576)]
        maximum_output_bytes: u64,
        #[arg(long, default_value_t = 0)]
        maximum_cost_microusd: u64,
    },
    /// Run trusted paired evaluations.
    Arena {
        #[command(subcommand)]
        command: ArenaCommand,
    },
    /// Verify and replay the canonical event stream.
    Replay,
    /// Control the local daemon process.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

#[derive(Subcommand)]
enum GenomeCommand {
    /// Show one canonical Genome record.
    Show {
        /// Content-derived Genome identity.
        genome_id: String,
    },
}

#[derive(Subcommand)]
enum ArenaCommand {
    /// Compare a parent and candidate using daemon-owned World tasks and budgets.
    Evaluate {
        /// Stable caller-selected evaluation identity.
        evaluation_id: String,
        /// Content-derived registered parent Genome identity.
        parent_genome_id: String,
        /// Content-derived registered candidate Genome identity.
        candidate_genome_id: String,
    },
}

#[derive(Subcommand)]
enum DaemonCommand {
    /// Request an audited graceful daemon stop.
    Stop,
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    let data_dir = match arguments
        .data_dir
        .map_or_else(data_dir_from_environment, Ok)
    {
        Ok(path) => path,
        Err(error) => {
            eprintln!("hephaestus: {error}");
            return ExitCode::FAILURE;
        }
    };
    let command = match command_from_cli(arguments.command) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("hephaestus: {message}");
            return ExitCode::FAILURE;
        }
    };
    let response = match Client::new(data_dir).request(command) {
        Ok(response) => response,
        Err(error) => {
            eprintln!("hephaestus: {error}");
            return ExitCode::FAILURE;
        }
    };
    if arguments.json {
        println!(
            "{}",
            serde_json::to_string(&response).expect("API response serialization cannot fail")
        );
    } else {
        print_human(&response);
    }
    if response.error.is_some() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn command_from_cli(command: CliCommand) -> Result<Command, &'static str> {
    Ok(match command {
        CliCommand::Status => Command::Status,
        CliCommand::Freeze => Command::Freeze,
        CliCommand::Unfreeze => Command::Unfreeze,
        CliCommand::Kill { all: true } => Command::KillAll,
        CliCommand::Kill { all: false } => return Err("kill requires --all"),
        CliCommand::Genome {
            command: GenomeCommand::Show { genome_id },
        } => Command::GenomeShow { genome_id },
        CliCommand::Run { genome_id } => Command::RunReference { genome_id },
        CliCommand::Evaluate {
            genome_id,
            task_id,
            input,
            seed,
            wall_millis,
            maximum_output_bytes,
            maximum_cost_microusd,
        } => Command::RunEvaluation {
            genome_id,
            task_id,
            input,
            seed,
            wall_millis,
            maximum_output_bytes,
            maximum_cost_microusd,
        },
        CliCommand::Arena {
            command:
                ArenaCommand::Evaluate {
                    evaluation_id,
                    parent_genome_id,
                    candidate_genome_id,
                },
        } => Command::EvaluatePair {
            evaluation_id,
            parent_genome_id,
            candidate_genome_id,
        },
        CliCommand::Replay => Command::Replay,
        CliCommand::Daemon {
            command: DaemonCommand::Stop,
        } => Command::DaemonStop,
    })
}

fn print_human(response: &ApiResponse) {
    match (&response.data, &response.error) {
        (
            Some(ResponseData::Status {
                frozen,
                active_runs,
                event_count,
                genome_count,
            }),
            None,
        ) => println!(
            "frozen={frozen} active_runs={active_runs} events={event_count} genomes={genome_count}"
        ),
        (
            Some(ResponseData::Acknowledged {
                frozen,
                killed_runs,
            }),
            None,
        ) => println!("acknowledged frozen={frozen} killed_runs={killed_runs}"),
        (Some(ResponseData::Genome { genome }), None) => println!(
            "{} {} world={} artifact={} parents={}",
            genome.genome_id,
            genome.name,
            genome.world_id,
            genome.artifact_id,
            genome.parent_ids.join(",")
        ),
        (
            Some(ResponseData::Run {
                run_id,
                genome_id,
                world_id,
                source_revision,
                completion_reason,
                latency_millis,
                actual_cost_microusd,
                stdout_artifact_id,
                stderr_artifact_id,
                trace_artifact_ids,
            }),
            None,
        ) => println!(
            "run={run_id} genome={genome_id} world={world_id} revision={source_revision} reason={completion_reason:?} latency_ms={latency_millis} cost_microusd={actual_cost_microusd} stdout={stdout_artifact_id} stderr={stderr_artifact_id} traces={}",
            trace_artifact_ids.join(",")
        ),
        (Some(ResponseData::Evaluation { evaluation }), None) => {
            println!("{}", evaluation_human(evaluation));
        }
        (
            Some(ResponseData::Replay {
                event_count,
                frozen,
                active_runs,
                projection_hash,
            }),
            None,
        ) => println!(
            "replayed events={event_count} frozen={frozen} active_runs={active_runs} projection={projection_hash}"
        ),
        (_, Some(error)) => eprintln!("{:?}: {}", error.code, error.message),
        _ => eprintln!("invalid daemon response"),
    }
}

fn evaluation_human(evaluation: &EvaluationRecord) -> String {
    format!(
        "evaluation={} parent={} candidate={} world={} candidate_visible={}/{} parent_visible={}/{} event={} sequence={} aggregate={}",
        evaluation.evaluation_id,
        evaluation.parent_genome_id,
        evaluation.candidate_genome_id,
        evaluation.world_id,
        evaluation.candidate_visible_correct,
        evaluation.visible_total,
        evaluation.parent_visible_correct,
        evaluation.visible_total,
        evaluation.event.event_id,
        evaluation.event.sequence,
        evaluation.event.aggregate_id
    )
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Arguments, command_from_cli, evaluation_human};
    use hephaestus_control::{Command, EvaluationEventRecord, EvaluationRecord};

    #[test]
    fn arena_evaluate_maps_positional_identifiers_to_paired_command() {
        let arguments = Arguments::try_parse_from([
            "hephaestus",
            "arena",
            "evaluate",
            "evaluation-1",
            "parent-1",
            "candidate-1",
        ])
        .expect("CLI parses");

        assert_eq!(
            command_from_cli(arguments.command).expect("command maps"),
            Command::EvaluatePair {
                evaluation_id: "evaluation-1".to_owned(),
                parent_genome_id: "parent-1".to_owned(),
                candidate_genome_id: "candidate-1".to_owned(),
            }
        );
    }

    #[test]
    fn evaluation_human_output_is_aggregate_only() {
        let evaluation = EvaluationRecord {
            evaluation_id: "evaluation-1".to_owned(),
            world_id: "world-1".to_owned(),
            parent_genome_id: "parent-1".to_owned(),
            candidate_genome_id: "candidate-1".to_owned(),
            parent_visible_correct: 2,
            candidate_visible_correct: 3,
            visible_total: 4,
            event: EvaluationEventRecord {
                sequence: 9,
                event_id: "evaluation:evaluation-1:recorded".to_owned(),
                aggregate_id: "evaluation:evaluation-1".to_owned(),
                event_type: "evaluation.recorded".to_owned(),
                actor: "arena-plane".to_owned(),
                timestamp_millis: 1_234,
            },
        };

        assert_eq!(
            evaluation_human(&evaluation),
            "evaluation=evaluation-1 parent=parent-1 candidate=candidate-1 world=world-1 candidate_visible=3/4 parent_visible=2/4 event=evaluation:evaluation-1:recorded sequence=9 aggregate=evaluation:evaluation-1"
        );
    }
}
