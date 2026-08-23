use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use hephaestus_control::{ApiResponse, Client, Command, ResponseData, data_dir_from_environment};

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
    let command = match arguments.command {
        CliCommand::Status => Command::Status,
        CliCommand::Freeze => Command::Freeze,
        CliCommand::Unfreeze => Command::Unfreeze,
        CliCommand::Kill { all: true } => Command::KillAll,
        CliCommand::Kill { all: false } => {
            eprintln!("hephaestus: kill requires --all");
            return ExitCode::FAILURE;
        }
        CliCommand::Genome {
            command: GenomeCommand::Show { genome_id },
        } => Command::GenomeShow { genome_id },
        CliCommand::Run { genome_id } => Command::RunReference { genome_id },
        CliCommand::Replay => Command::Replay,
        CliCommand::Daemon {
            command: DaemonCommand::Stop,
        } => Command::DaemonStop,
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
