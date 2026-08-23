use serde::{Deserialize, de::DeserializeOwned};

use crate::CompileError;

pub(crate) const MAX_SOURCE_BYTES: usize = 1_048_576;

/// Supported human-authored document encodings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFormat {
    /// Strict JSON decoded by `serde_json`.
    Json,
    /// YAML decoded directly into typed structures by `serde-saphyr`.
    Yaml,
}

#[derive(Deserialize)]
struct VersionProbe {
    schema_version: u16,
}

pub(crate) fn parse_versioned<T: DeserializeOwned>(
    source: &str,
    format: SourceFormat,
) -> Result<T, CompileError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(CompileError::InputTooLarge {
            bytes: source.len(),
            maximum: MAX_SOURCE_BYTES,
        });
    }
    let version: VersionProbe = parse(source, format)?;
    if version.schema_version != 1 {
        return Err(CompileError::UnsupportedSchemaVersion(
            version.schema_version,
        ));
    }
    parse(source, format)
}

fn parse<T: DeserializeOwned>(source: &str, format: SourceFormat) -> Result<T, CompileError> {
    let parsed = match format {
        SourceFormat::Json => serde_json::from_str(source),
        SourceFormat::Yaml => serde_saphyr::from_str(source)
            .map_err(|_| serde_json::Error::io(std::io::Error::other("invalid YAML"))),
    };
    parsed.map_err(|_| {
        CompileError::Parse(match format {
            SourceFormat::Json => "JSON document does not match the declared schema".to_owned(),
            SourceFormat::Yaml => "YAML document does not match the declared schema".to_owned(),
        })
    })
}

pub(crate) fn require_text(value: &str, field: &'static str) -> Result<(), CompileError> {
    if value.trim().is_empty() {
        return Err(CompileError::EmptyField(field));
    }
    Ok(())
}

pub(crate) fn canonical_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, CompileError> {
    serde_json::to_vec(value).map_err(|error| CompileError::Canonicalization(error.to_string()))
}

pub(crate) fn content_id(namespace: &str, canonical: &[u8]) -> String {
    format!(
        "hephaestus:{namespace}:{}",
        blake3::hash(canonical).to_hex()
    )
}
