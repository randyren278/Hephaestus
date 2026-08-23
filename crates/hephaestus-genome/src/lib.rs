//! Immutable Genome and World compilation.

mod compiler;
mod error;
mod genome;
mod world;

pub use compiler::SourceFormat;
pub use error::CompileError;
pub use genome::{CompiledGenome, compile_genome};
pub use world::{CompiledWorld, WorldEvaluationPolicy, compile_world, ensure_comparable};
