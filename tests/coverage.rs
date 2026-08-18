use volmap::diagnostics::{
    ContainmentImpact, CoverageFacet, CoverageLedger, CoverageLedgerId, CoverageLedgerInput,
    CoverageStop, CoverageStopReason, DiagnosticCatalog, DiagnosticCode, DiagnosticOccurrence,
    DiagnosticOccurrenceId, DiagnosticOccurrenceInput, DiagnosticSeverity, Remainder, TrustedTotal,
    ValidationBoundary, ValidationBoundaryKind,
};
use volmap::model::{
    Coverage, EntityId, EntityReference, EvidenceId, ReferenceResolution, SnapshotId,
};

#[test]
fn complete_coverage_requires_a_trusted_fully_conclusive_total() {
    let ledger = CoverageLedger::new(CoverageLedgerInput {
        id: CoverageLedgerId::new(1),
        facet: CoverageFacet::PageEnvelopes,
        coverage: Coverage::Complete,
        evaluated: 10,
        conclusive: 10,
        total: TrustedTotal::Known(10),
        stopped: None,
        related_diagnostics: Vec::new(),
        remainder: Remainder::Known(0),
    })
    .unwrap();

    assert_eq!(ledger.coverage(), Coverage::Complete);
    assert_eq!(ledger.total(), TrustedTotal::Known(10));

    let invalid = CoverageLedger::new(CoverageLedgerInput {
        total: TrustedTotal::Unknown,
        ..ledger.into_input()
    });
    assert!(invalid.is_err());
}

#[test]
fn partial_coverage_names_its_stopped_boundary_reason_and_remainder() {
    let stop = CoverageStop::new(
        ValidationBoundary::new(ValidationBoundaryKind::OosChain, None),
        CoverageStopReason::ResourceLimit,
    );
    let ledger = CoverageLedger::new(CoverageLedgerInput {
        id: CoverageLedgerId::new(2),
        facet: CoverageFacet::OosChain,
        coverage: Coverage::Partial,
        evaluated: 3,
        conclusive: 3,
        total: TrustedTotal::Unknown,
        stopped: Some(stop),
        related_diagnostics: vec![DiagnosticOccurrenceId::from_bytes([9; 16])],
        remainder: Remainder::Unknown,
    })
    .unwrap();

    assert_eq!(
        ledger.stopped().unwrap().reason(),
        CoverageStopReason::ResourceLimit
    );
    assert_eq!(ledger.remainder(), Remainder::Unknown);

    let invalid = CoverageLedger::new(CoverageLedgerInput {
        stopped: None,
        ..ledger.into_input()
    });
    assert!(invalid.is_err());
}

#[test]
fn partial_coverage_rejects_contradictory_remainders_and_unlinked_resource_limits() {
    let resource_stop = CoverageStop::new(
        ValidationBoundary::new(ValidationBoundaryKind::OosChain, None),
        CoverageStopReason::ResourceLimit,
    );
    let contradictory_remainder = CoverageLedger::new(CoverageLedgerInput {
        id: CoverageLedgerId::new(3),
        facet: CoverageFacet::OosChain,
        coverage: Coverage::Partial,
        evaluated: 3,
        conclusive: 3,
        total: TrustedTotal::Known(10),
        stopped: Some(resource_stop),
        related_diagnostics: vec![DiagnosticOccurrenceId::from_bytes([7; 16])],
        remainder: Remainder::Known(99),
    });
    assert!(contradictory_remainder.is_err());

    let unlinked_resource_limit = CoverageLedger::new(CoverageLedgerInput {
        id: CoverageLedgerId::new(4),
        facet: CoverageFacet::OosChain,
        coverage: Coverage::Partial,
        evaluated: 3,
        conclusive: 3,
        total: TrustedTotal::Unknown,
        stopped: Some(resource_stop),
        related_diagnostics: Vec::new(),
        remainder: Remainder::Unknown,
    });
    assert!(unlinked_resource_limit.is_err());
}

#[test]
fn occurrence_classification_comes_from_the_catalog() {
    let subject = EntityReference::new(
        EntityId::Snapshot(SnapshotId::from_bytes([4; 16])),
        ReferenceResolution::Resolved,
    );
    let boundary = ValidationBoundary::new(ValidationBoundaryKind::SlotExtent, Some(subject));
    let occurrence = DiagnosticOccurrence::new(
        DiagnosticCatalog::v1(),
        DiagnosticOccurrenceInput {
            id: DiagnosticOccurrenceId::from_bytes([5; 16]),
            code: DiagnosticCode::SLOT_EXTENT_OVERLAP,
            affected: vec![subject],
            evidence: vec![EvidenceId::new(1)],
            byte_locators: Vec::new(),
            failed_boundary: boundary,
            containment: ContainmentImpact::new(vec![CoverageFacet::SlotDetails], Vec::new()),
            coverage_ledgers: Vec::new(),
            parameters: BTreeMap::default(),
        },
    )
    .unwrap();

    assert_eq!(occurrence.severity(), DiagnosticSeverity::Error);
    assert!(occurrence.is_anomaly());
    assert_eq!(occurrence.code(), DiagnosticCode::SLOT_EXTENT_OVERLAP);
    assert_eq!(occurrence.message(), "Slot extents overlap.");
}
use std::collections::BTreeMap;
