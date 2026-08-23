use hephaestus_core::authority::{Actor, AuthorityError, CapabilitySet, FreezeState};

#[test]
fn child_capabilities_may_narrow_but_never_widen() {
    let parent = CapabilitySet::new(true, false);

    assert_eq!(
        parent.derive_child(CapabilitySet::new(false, false)),
        Ok(CapabilitySet::new(false, false))
    );
    assert_eq!(
        parent.derive_child(CapabilitySet::new(true, true)),
        Err(AuthorityError::CapabilityEscalation)
    );
}

#[test]
fn only_the_operator_can_clear_a_freeze() {
    let mut state = FreezeState::frozen();

    assert_eq!(
        state.unfreeze(Actor::Candidate),
        Err(AuthorityError::OperatorRequired)
    );
    assert!(state.is_frozen());
    assert_eq!(state.unfreeze(Actor::Operator), Ok(()));
    assert!(!state.is_frozen());
}
