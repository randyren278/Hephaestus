//! Deployable exact-match evaluator used by the trusted local Arena scheduler.

use std::io::{Read as _, Write as _};

use hephaestus_arena::evaluator_protocol::{MAX_EVALUATOR_REQUEST_BYTES, evaluate_request};

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(std::io::stderr(), "evaluator rejected request: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let limit = u64::try_from(MAX_EVALUATOR_REQUEST_BYTES)? + 1;
    let mut request = Vec::new();
    std::io::stdin().take(limit).read_to_end(&mut request)?;
    let response = evaluate_request(&request)?;
    std::io::stdout().write_all(&response)?;
    Ok(())
}
