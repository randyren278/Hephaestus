use hephaestus_core::domain::{
    DomainError, DomainFixture, DomainId, EntityKind, GenomeStatus, MutationTarget, SchemaVersion,
};

#[test]
fn versioned_domain_fixture_accepts_v1_and_rejects_unknown_versions() {
    let fixture = include_str!("fixtures/domain-v1.json");

    assert_eq!(
        DomainFixture::from_json(fixture),
        Ok(DomainFixture::new(
            SchemaVersion::V1,
            EntityKind::Genome,
            DomainId::parse("coding-g0").expect("fixture id is valid")
        ))
    );
    assert_eq!(
        DomainFixture::from_json(r#"{"schema_version":2,"entity":"genome","id":"g0"}"#),
        Err(DomainError::UnsupportedSchemaVersion(2))
    );
    let stable_id = DomainId::parse(" genome:g0 ").expect("non-empty id is valid");
    assert_eq!(stable_id.as_str(), " genome:g0 ");
    assert_eq!(DomainId::parse("  "), Err(DomainError::EmptyIdentifier));
    assert_eq!(DomainError::EmptyIdentifier.to_string(), "EmptyIdentifier");
    assert!(matches!(
        DomainFixture::from_json("{"),
        Err(DomainError::InvalidFixture(_))
    ));
    assert!(matches!(
        DomainFixture::from_json(
            r#"{"schema_version":1,"entity":"genome","id":"g0","extra":true}"#
        ),
        Err(DomainError::InvalidFixture(_))
    ));
}

#[test]
fn genome_lifecycle_allows_only_declared_transitions() {
    use GenomeStatus::{Champion, Experimental, Quarantined, Retired, Validated};

    let statuses = [Experimental, Validated, Champion, Retired, Quarantined];
    let allowed = [
        (Experimental, Validated),
        (Experimental, Quarantined),
        (Validated, Champion),
        (Validated, Retired),
        (Validated, Quarantined),
        (Champion, Retired),
        (Champion, Quarantined),
    ];

    for current in statuses {
        for next in statuses {
            let result = current.transition_to(next);
            if allowed.contains(&(current, next)) {
                assert_eq!(result, Ok(next));
            } else {
                assert_eq!(
                    result,
                    Err(DomainError::InvalidGenomeTransition { current, next })
                );
            }
        }
    }
}

#[test]
fn mutation_targets_keep_laws_and_evaluators_outside_evolution() {
    assert_eq!(MutationTarget::Harness.authorize(), Ok(()));
    assert_eq!(
        MutationTarget::Law.authorize(),
        Err(DomainError::ProtectedMutationTarget(MutationTarget::Law))
    );
    assert_eq!(
        MutationTarget::Evaluator.authorize(),
        Err(DomainError::ProtectedMutationTarget(
            MutationTarget::Evaluator
        ))
    );
}
