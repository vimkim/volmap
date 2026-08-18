use std::collections::BTreeSet;

use volmap::diagnostics::{
    DiagnosticCatalog, DiagnosticCode, DiagnosticSeverity, InspectionOutcome, OutcomeInputs,
    ValidationBoundaryKind,
};

#[test]
fn v1_catalog_contains_every_mandatory_unique_namespaced_code() {
    let definitions = DiagnosticCatalog::v1().definitions();
    let codes: BTreeSet<_> = definitions
        .iter()
        .map(|definition| definition.code())
        .collect();

    assert_eq!(definitions.len(), 51);
    assert_eq!(codes.len(), definitions.len());
    assert!(definitions.iter().all(|definition| {
        let code = definition.code().as_str();
        code.contains('.')
            && code.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'_'
            })
    }));
}

#[test]
fn catalog_fixes_severity_anomaly_and_containment_in_the_core() {
    let catalog = DiagnosticCatalog::v1();

    let snapshot = catalog
        .definition(DiagnosticCode::SNAPSHOT_MODIFIED)
        .unwrap();
    assert_eq!(snapshot.default_severity(), DiagnosticSeverity::Fatal);
    assert_eq!(snapshot.boundary(), ValidationBoundaryKind::SnapshotTrust);

    let resource = catalog
        .definition(DiagnosticCode::INSPECTION_RESOURCE_LIMIT)
        .unwrap();
    assert_eq!(resource.default_severity(), DiagnosticSeverity::Error);
    assert!(!resource.is_anomaly());

    let overlap = catalog
        .definition(DiagnosticCode::SLOT_EXTENT_OVERLAP)
        .unwrap();
    assert_eq!(overlap.default_severity(), DiagnosticSeverity::Error);
    assert!(overlap.is_anomaly());
    assert_eq!(overlap.boundary(), ValidationBoundaryKind::SlotExtent);
}

#[test]
fn outcome_precedence_is_fixed_without_assigning_cli_exit_integers() {
    assert_eq!(
        InspectionOutcome::classify(OutcomeInputs::default()),
        InspectionOutcome::Success
    );
    assert_eq!(
        InspectionOutcome::classify(OutcomeInputs {
            expected_limitations: true,
            ..OutcomeInputs::default()
        }),
        InspectionOutcome::SuccessLimited
    );
    assert_eq!(
        InspectionOutcome::classify(OutcomeInputs {
            has_error_findings: true,
            ..OutcomeInputs::default()
        }),
        InspectionOutcome::Findings
    );
    assert_eq!(
        InspectionOutcome::classify(OutcomeInputs {
            has_error_findings: true,
            unexpected_incomplete: true,
            ..OutcomeInputs::default()
        }),
        InspectionOutcome::Incomplete
    );
    assert_eq!(
        InspectionOutcome::classify(OutcomeInputs {
            fatal: true,
            unexpected_incomplete: true,
            ..OutcomeInputs::default()
        }),
        InspectionOutcome::Fatal
    );
    assert!(InspectionOutcome::SuccessLimited.is_process_success());
    assert!(!InspectionOutcome::Findings.is_process_success());
}
