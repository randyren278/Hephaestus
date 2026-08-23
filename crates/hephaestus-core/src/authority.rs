//! Fail-closed capability and operator-control laws.

/// The actor requesting an authority transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Actor {
    /// An external operator acting outside the evolutionary runtime.
    Operator,
    /// A candidate Genome or one of its descendants.
    Candidate,
}

/// Authority failures are explicit and stable enough to ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityError {
    /// A child requested authority unavailable to its parent.
    CapabilityEscalation,
    /// A candidate attempted an operator-only transition.
    OperatorRequired,
}

/// Capabilities carried by a runtime worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilitySet(u8);

impl CapabilitySet {
    const WORKSPACE_WRITE: u8 = 0b01;
    const NETWORK: u8 = 0b10;

    /// Creates a capability set from the two initial authority dimensions.
    #[must_use]
    pub const fn new(workspace_write: bool, network: bool) -> Self {
        Self((workspace_write as u8 * Self::WORKSPACE_WRITE) | (network as u8 * Self::NETWORK))
    }

    /// Derives a child capability set without permitting authority escalation.
    ///
    /// # Errors
    ///
    /// Returns [`AuthorityError::CapabilityEscalation`] when `requested`
    /// contains any capability absent from the parent.
    pub const fn derive_child(self, requested: Self) -> Result<Self, AuthorityError> {
        if requested.0 & !self.0 != 0 {
            return Err(AuthorityError::CapabilityEscalation);
        }

        Ok(requested)
    }
}

/// Persistent operator freeze state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreezeState {
    frozen: bool,
}

impl FreezeState {
    /// Creates an engaged freeze state.
    #[must_use]
    pub const fn frozen() -> Self {
        Self { frozen: true }
    }

    /// Reports whether evolution remains frozen.
    #[must_use]
    pub const fn is_frozen(self) -> bool {
        self.frozen
    }

    /// Clears the freeze only for an external operator.
    ///
    /// # Errors
    ///
    /// Returns [`AuthorityError::OperatorRequired`] for candidate requests.
    pub const fn unfreeze(&mut self, actor: Actor) -> Result<(), AuthorityError> {
        if !matches!(actor, Actor::Operator) {
            return Err(AuthorityError::OperatorRequired);
        }

        self.frozen = false;
        Ok(())
    }
}
