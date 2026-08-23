use std::{path::PathBuf, process::ExitCode};

use clap::Parser;
use hephaestus_control::{ControlPlane, data_dir_from_environment};

#[derive(Parser)]
#[command(name = "hephaestusd", about = "Hephaestus canonical local daemon")]
struct Arguments {
    /// Canonical daemon data directory.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Git repository materialized into isolated reference-run worktrees.
    #[arg(long)]
    source_repository: Option<PathBuf>,
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    let data_dir = match arguments
        .data_dir
        .map_or_else(data_dir_from_environment, Ok)
    {
        Ok(path) => path,
        Err(error) => {
            eprintln!("hephaestusd: {error}");
            return ExitCode::FAILURE;
        }
    };
    let source_repository = match arguments
        .source_repository
        .map_or_else(std::env::current_dir, Ok)
    {
        Ok(path) => path,
        Err(error) => {
            eprintln!("hephaestusd: {error}");
            return ExitCode::FAILURE;
        }
    };
    match ControlPlane::open_with_repository(data_dir, source_repository)
        .and_then(ControlPlane::serve)
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("hephaestusd: {error}");
            ExitCode::FAILURE
        }
    }
}
