use hephaestus_core::authority::{AuthorityError, CapabilitySet, FreezeState, OperatorToken};

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
    let operator = OperatorToken::from_bytes([0xA5; 32]);
    let impostor = OperatorToken::from_bytes([0x5A; 32]);
    let mut state = FreezeState::frozen(&operator);

    assert_eq!(
        state.unfreeze(&impostor),
        Err(AuthorityError::OperatorRequired)
    );
    assert!(state.is_frozen());
    assert_eq!(state.unfreeze(&operator), Ok(()));
    assert!(!state.is_frozen());
}
