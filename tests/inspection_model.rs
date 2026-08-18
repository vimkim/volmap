use volmap::model::{
    Availability, Coverage, DerivedEvidence, EntityId, EntityReference, EvidenceId, FileId,
    InspectionRevision, InterpretedEvidence, ObservedEvidence, Oid, PageAllocationClass, PageId,
    PinnedFormatProfile, ReadOutcome, ReferenceResolution, RuleId, SlotId, SnapshotId,
    TdeInspectionState, Vfid, VolId, VolumeByteRange, VolumeEntityId,
};

fn snapshot(byte: u8) -> SnapshotId {
    SnapshotId::from_bytes([byte; 16])
}

#[test]
fn physical_identity_is_always_scoped_by_the_snapshot() {
    let physical = VolId::new(2).unwrap();
    let first = EntityId::Volume(VolumeEntityId::new(snapshot(1), physical));
    let second = EntityId::Volume(VolumeEntityId::new(snapshot(2), physical));

    assert_ne!(first, second);
    let unresolved = EntityReference::new(first, ReferenceResolution::Unresolved);
    assert_eq!(unresolved.resolution(), ReferenceResolution::Unresolved);
    assert_eq!(unresolved.target(), first);
}

#[test]
fn evidence_records_safe_locators_and_provenance_without_source_bytes() {
    let range = VolumeByteRange::new(
        VolumeEntityId::new(snapshot(7), VolId::new(0).unwrap()),
        32,
        25,
    )
    .unwrap();
    let observed = ObservedEvidence::new(EvidenceId::new(1), range, ReadOutcome::Read);
    let interpreted = InterpretedEvidence::new(
        EvidenceId::new(2),
        PinnedFormatProfile::FeatOosE1e651de,
        RuleId::new("volume.header.magic").unwrap(),
        vec![observed.id()],
    )
    .unwrap();
    let derived = DerivedEvidence::new(
        EvidenceId::new(3),
        RuleId::new("sector.summary.counts").unwrap(),
        vec![interpreted.id()],
        Vec::new(),
    )
    .unwrap();

    assert_eq!(observed.range().offset(), 32);
    assert_eq!(observed.range().length(), 25);
    assert_eq!(interpreted.observed_inputs(), &[EvidenceId::new(1)]);
    assert_eq!(derived.evidence_inputs(), &[EvidenceId::new(2)]);
}

#[test]
fn evidence_ranges_and_revisions_reject_overflow() {
    let volume = VolumeEntityId::new(snapshot(1), VolId::new(0).unwrap());
    assert!(VolumeByteRange::new(volume, u64::MAX, 1).is_err());
    assert!(InspectionRevision::new(u64::MAX).next().is_err());
}

#[test]
fn orthogonal_inspection_axes_remain_distinct_types() {
    let availability = Availability::EncryptedOpaque;
    let coverage = Coverage::Partial;
    let allocation = PageAllocationClass::Allocated;
    let tde = TdeInspectionState::EncryptedOpaque;

    assert_eq!(availability, Availability::EncryptedOpaque);
    assert_eq!(coverage, Coverage::Partial);
    assert_eq!(allocation, PageAllocationClass::Allocated);
    assert_eq!(tde, TdeInspectionState::EncryptedOpaque);
}

#[test]
fn oid_and_vfid_use_validated_pinned_width_identifiers() {
    let vfid = Vfid::new(VolId::new(1).unwrap(), FileId::new(9).unwrap());
    let oid = Oid::new(
        VolId::new(1).unwrap(),
        PageId::new(12).unwrap(),
        SlotId::new(3).unwrap(),
    );

    assert_eq!(vfid.file_id.get(), 9);
    assert_eq!(oid.slot_id.get(), 3);
}

#[test]
fn negative_physical_identifiers_are_rejected_before_conversion() {
    assert!(VolId::new(-1).is_err());
    assert!(PageId::new(-1).is_err());
    assert!(FileId::new(-1).is_err());
    assert!(SlotId::new(-1).is_err());
}
