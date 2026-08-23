//! Executable product vocabulary and lifecycle contracts.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

/// The only domain schema version accepted by this release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum SchemaVersion {
    /// Initial Hephaestus domain schema.
    V1 = 1,
}

impl TryFrom<u16> for SchemaVersion {
    type Error = DomainError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value == Self::V1 as u16 {
            return Ok(Self::V1);
        }

        Err(DomainError::UnsupportedSchemaVersion(value))
    }
}

/// Canonical vocabulary shared by ledgers, APIs, and projections.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// A running instance of a Genome.
    Agent,
    /// An immutable agent intelligence configuration.
    Genome,
    /// One hypothesized change to a Genome.
    Mutation,
    /// A Genome produced from one or more parents.
    Descendant,
    /// An ancestry graph of related Genomes.
    Lineage,
    /// An experimentally supported reusable mutation.
    Gene,
    /// A versioned evaluation environment and its Laws.
    World,
    /// A non-evolvable World rule.
    Law,
    /// The promoted Genome for a lineage and World.
    Champion,
    /// The protected candidate evaluation environment.
    Arena,
    /// A multi-generation improvement process.
    Evolution,
    /// A material environment or workload change.
    Drift,
}

/// Validated stable identifier used by domain records.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DomainId(String);

impl DomainId {
    /// Parses a non-empty identifier while preserving its exact value.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyIdentifier`] for empty or whitespace-only input.
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyIdentifier);
        }

        Ok(Self(value))
    }

    /// Returns the stable identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Minimal versioned fixture proving the shared domain contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainFixture {
    /// Version used to decode the fixture.
    pub schema_version: SchemaVersion,
    /// Canonical entity represented by the fixture.
    pub entity: EntityKind,
    /// Stable entity identifier.
    pub id: DomainId,
}

impl DomainFixture {
    /// Creates a validated fixture from already-typed fields.
    #[must_use]
    pub const fn new(schema_version: SchemaVersion, entity: EntityKind, id: DomainId) -> Self {
        Self {
            schema_version,
            entity,
            id,
        }
    }

    /// Parses a JSON fixture and rejects unknown schema versions.
    ///
    /// # Errors
    ///
    /// Returns a structured domain error for malformed JSON, unsupported versions,
    /// or invalid identifiers.
    pub fn from_json(json: &str) -> Result<Self, DomainError> {
        let raw: RawDomainFixture = serde_json::from_str(json)
            .map_err(|error| DomainError::InvalidFixture(error.to_string()))?;
        Ok(Self::new(
            SchemaVersion::try_from(raw.schema_version)?,
            raw.entity,
            DomainId::parse(raw.id)?,
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDomainFixture {
    schema_version: u16,
    entity: EntityKind,
    id: String,
}

/// Lifecycle status of an immutable Genome record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenomeStatus {
    /// Created but not yet validated.
    Experimental,
    /// Eligible for Arena selection.
    Validated,
    /// Currently promoted for a lineage and World.
    Champion,
    /// Archived and no longer scheduled.
    Retired,
    /// Isolated because validation or safety failed.
    Quarantined,
}

impl GenomeStatus {
    /// Applies one declared lifecycle transition.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidGenomeTransition`] for undeclared or terminal
    /// transitions.
    pub const fn transition_to(self, next: Self) -> Result<Self, DomainError> {
        match (self, next) {
            (Self::Experimental, Self::Validated | Self::Quarantined)
            | (Self::Validated, Self::Champion | Self::Retired | Self::Quarantined)
            | (Self::Champion, Self::Retired | Self::Quarantined) => Ok(next),
            _ => Err(DomainError::InvalidGenomeTransition {
                current: self,
                next,
            }),
        }
    }
}

/// Surfaces that a proposed mutation may attempt to change.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationTarget {
    /// Evolvable prompts, tools, memory, context, topology, and runtime policy.
    Harness,
    /// Non-evolvable World physics.
    Law,
    /// Protected scoring implementation and sealed holdouts.
    Evaluator,
}

impl MutationTarget {
    /// Authorizes only evolvable harness mutations.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::ProtectedMutationTarget`] for Laws and evaluators.
    pub const fn authorize(self) -> Result<(), DomainError> {
        match self {
            Self::Harness => Ok(()),
            Self::Law | Self::Evaluator => Err(DomainError::ProtectedMutationTarget(self)),
        }
    }
}

/// Fail-closed violations of the shared domain contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    /// The serialized schema is not supported by this release.
    UnsupportedSchemaVersion(u16),
    /// An identifier was empty or contained only whitespace.
    EmptyIdentifier,
    /// A Genome lifecycle transition was not declared by the constitution.
    InvalidGenomeTransition {
        /// Current persisted status.
        current: GenomeStatus,
        /// Requested next status.
        next: GenomeStatus,
    },
    /// A mutation targeted a surface outside evolution.
    ProtectedMutationTarget(MutationTarget),
    /// A serialized fixture was malformed.
    InvalidFixture(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for DomainError {}
