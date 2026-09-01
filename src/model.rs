//! Presentation-independent inspection identities and facts.

use core::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdentifierError {
    kind: &'static str,
    value: i64,
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "negative {} identifier: {}",
            self.kind, self.value
        )
    }
}

impl std::error::Error for IdentifierError {}

macro_rules! non_negative_identifier {
    ($name:ident, $raw:ty, $kind:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name($raw);

        impl $name {
            pub fn new(value: $raw) -> Result<Self, IdentifierError> {
                if value < 0 {
                    return Err(IdentifierError {
                        kind: $kind,
                        value: i64::from(value),
                    });
                }
                Ok(Self(value))
            }

            #[must_use]
            pub const fn get(self) -> $raw {
                self.0
            }
        }
    };
}

non_negative_identifier!(VolId, i16, "volume");
non_negative_identifier!(PageId, i32, "page");
non_negative_identifier!(SectorId, i32, "sector");
non_negative_identifier!(FileId, i32, "file");
non_negative_identifier!(SlotId, i16, "slot");

/// Physical page identity in the pinned format.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Vpid {
    pub vol_id: VolId,
    pub page_id: PageId,
}

impl Vpid {
    #[must_use]
    pub const fn new(vol_id: VolId, page_id: PageId) -> Self {
        Self { vol_id, page_id }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Vfid {
    pub vol_id: VolId,
    pub file_id: FileId,
}

impl Vfid {
    #[must_use]
    pub const fn new(vol_id: VolId, file_id: FileId) -> Self {
        Self { vol_id, file_id }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Hfid {
    pub vfid: Vfid,
    pub header_page_id: PageId,
}

impl Hfid {
    #[must_use]
    pub const fn new(vfid: Vfid, header_page_id: PageId) -> Self {
        Self {
            vfid,
            header_page_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Physical identity of a B-tree file and its root page.
pub struct Btid {
    pub vfid: Vfid,
    pub root_page_id: PageId,
}

impl Btid {
    #[must_use]
    pub const fn new(vfid: Vfid, root_page_id: PageId) -> Self {
        Self { vfid, root_page_id }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Oid {
    pub vol_id: VolId,
    pub page_id: PageId,
    pub slot_id: SlotId,
}

impl Oid {
    #[must_use]
    pub const fn new(vol_id: VolId, page_id: PageId, slot_id: SlotId) -> Self {
        Self {
            vol_id,
            page_id,
            slot_id,
        }
    }
}

/// Opaque identity unique to one inspection artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SnapshotId([u8; 16]);

impl SnapshotId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InspectionRevision(u64);

impl InspectionRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Result<Self, ModelError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ModelError::ArithmeticOverflow)
    }
}

macro_rules! snapshot_entity_id {
    ($name:ident, $physical:ty, $field:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            pub snapshot_id: SnapshotId,
            pub $field: $physical,
        }

        impl $name {
            #[must_use]
            pub const fn new(snapshot_id: SnapshotId, $field: $physical) -> Self {
                Self {
                    snapshot_id,
                    $field,
                }
            }
        }
    };
}

snapshot_entity_id!(VolumeEntityId, VolId, vol_id);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SectorEntityId {
    pub snapshot_id: SnapshotId,
    pub vol_id: VolId,
    pub sector_id: SectorId,
}

impl SectorEntityId {
    #[must_use]
    pub const fn new(snapshot_id: SnapshotId, vol_id: VolId, sector_id: SectorId) -> Self {
        Self {
            snapshot_id,
            vol_id,
            sector_id,
        }
    }
}

snapshot_entity_id!(FileEntityId, Vfid, vfid);
snapshot_entity_id!(PageEntityId, Vpid, vpid);
snapshot_entity_id!(SlotEntityId, Oid, oid);
snapshot_entity_id!(OosChainEntityId, Oid, head_oid);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EntityId {
    Snapshot(SnapshotId),
    Volume(VolumeEntityId),
    Sector(SectorEntityId),
    File(FileEntityId),
    Page(PageEntityId),
    Slot(SlotEntityId),
    OosChain(OosChainEntityId),
}

impl EntityId {
    #[must_use]
    pub const fn snapshot_id(self) -> SnapshotId {
        match self {
            Self::Snapshot(id) => id,
            Self::Volume(id) => id.snapshot_id,
            Self::Sector(id) => id.snapshot_id,
            Self::File(id) => id.snapshot_id,
            Self::Page(id) => id.snapshot_id,
            Self::Slot(id) => id.snapshot_id,
            Self::OosChain(id) => id.snapshot_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReferenceResolution {
    Resolved,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityReference {
    target: EntityId,
    resolution: ReferenceResolution,
}

impl EntityReference {
    #[must_use]
    pub const fn new(target: EntityId, resolution: ReferenceResolution) -> Self {
        Self { target, resolution }
    }

    #[must_use]
    pub const fn target(self) -> EntityId {
        self.target
    }

    #[must_use]
    pub const fn resolution(self) -> ReferenceResolution {
        self.resolution
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    Available,
    Unreadable,
    Unsupported,
    EncryptedOpaque,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coverage {
    NotRequested,
    Partial,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageAllocationClass {
    SystemMetadata,
    Unreserved,
    ReservedUnallocated,
    Allocated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TdeInspectionState {
    NotEncrypted,
    Decrypted,
    EncryptedOpaque,
    KeyError,
    DecryptedInvalid,
    InvalidFlags,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PinnedFormatProfile {
    FeatOosE1e651de,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotValidity {
    /// The input fingerprint manifest still matches what was scanned.
    Valid,
    /// The manifest changed during this reading's own scan, so its facts may
    /// mix pre-change and post-change bytes. Live follow only.
    Torn,
    /// The manifest no longer matches the input. The facts stay exactly as
    /// observed; they are simply no longer the current view. Live follow only.
    Superseded,
    /// The manifest changed and the immutable contract forbids re-reading.
    Invalidated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DatabaseSnapshotIdentity {
    pub id: SnapshotId,
    pub revision: InspectionRevision,
    pub format_profile: PinnedFormatProfile,
    pub validity: SnapshotValidity,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EvidenceId(u64);

impl EvidenceId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuleId(&'static str);

impl RuleId {
    pub fn new(value: &'static str) -> Result<Self, ModelError> {
        if value.is_empty()
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'_'
            })
        {
            return Err(ModelError::InvalidRuleId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VolumeByteRange {
    volume: VolumeEntityId,
    offset: u64,
    length: u64,
}

impl VolumeByteRange {
    pub fn new(volume: VolumeEntityId, offset: u64, length: u64) -> Result<Self, ModelError> {
        if length == 0 {
            return Err(ModelError::EmptyRange);
        }
        offset
            .checked_add(length)
            .ok_or(ModelError::ArithmeticOverflow)?;
        Ok(Self {
            volume,
            offset,
            length,
        })
    }

    #[must_use]
    pub const fn volume(self) -> VolumeEntityId {
        self.volume
    }

    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn length(self) -> u64 {
        self.length
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadOutcome {
    Read,
    Unreadable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedEvidence {
    id: EvidenceId,
    range: VolumeByteRange,
    outcome: ReadOutcome,
}

impl ObservedEvidence {
    #[must_use]
    pub const fn new(id: EvidenceId, range: VolumeByteRange, outcome: ReadOutcome) -> Self {
        Self { id, range, outcome }
    }

    #[must_use]
    pub const fn id(self) -> EvidenceId {
        self.id
    }

    #[must_use]
    pub const fn range(self) -> VolumeByteRange {
        self.range
    }

    #[must_use]
    pub const fn outcome(self) -> ReadOutcome {
        self.outcome
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterpretedEvidence {
    id: EvidenceId,
    profile: PinnedFormatProfile,
    validation_rule: RuleId,
    observed_inputs: Vec<EvidenceId>,
}

impl InterpretedEvidence {
    pub fn new(
        id: EvidenceId,
        profile: PinnedFormatProfile,
        validation_rule: RuleId,
        observed_inputs: Vec<EvidenceId>,
    ) -> Result<Self, ModelError> {
        if observed_inputs.is_empty() {
            return Err(ModelError::MissingEvidence);
        }
        Ok(Self {
            id,
            profile,
            validation_rule,
            observed_inputs,
        })
    }

    #[must_use]
    pub const fn id(&self) -> EvidenceId {
        self.id
    }

    #[must_use]
    pub fn observed_inputs(&self) -> &[EvidenceId] {
        &self.observed_inputs
    }

    #[must_use]
    pub const fn profile(&self) -> PinnedFormatProfile {
        self.profile
    }

    #[must_use]
    pub const fn validation_rule(&self) -> RuleId {
        self.validation_rule
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedEvidence {
    id: EvidenceId,
    derivation_rule: RuleId,
    evidence_inputs: Vec<EvidenceId>,
    entity_inputs: Vec<EntityReference>,
}

impl DerivedEvidence {
    pub fn new(
        id: EvidenceId,
        derivation_rule: RuleId,
        evidence_inputs: Vec<EvidenceId>,
        entity_inputs: Vec<EntityReference>,
    ) -> Result<Self, ModelError> {
        if evidence_inputs.is_empty() && entity_inputs.is_empty() {
            return Err(ModelError::MissingEvidence);
        }
        Ok(Self {
            id,
            derivation_rule,
            evidence_inputs,
            entity_inputs,
        })
    }

    #[must_use]
    pub const fn id(&self) -> EvidenceId {
        self.id
    }

    #[must_use]
    pub fn evidence_inputs(&self) -> &[EvidenceId] {
        &self.evidence_inputs
    }

    #[must_use]
    pub fn entity_inputs(&self) -> &[EntityReference] {
        &self.entity_inputs
    }

    #[must_use]
    pub const fn derivation_rule(&self) -> RuleId {
        self.derivation_rule
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Evidence {
    Observed(ObservedEvidence),
    Interpreted(InterpretedEvidence),
    Derived(DerivedEvidence),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationshipKind {
    Contains,
    AllocationClaim,
    ResolvedOwnership,
    OosReference,
    OosChunkMembership,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipClaim {
    pub kind: RelationshipKind,
    pub from: EntityReference,
    pub to: EntityReference,
    pub evidence: Vec<EvidenceId>,
}

impl RelationshipClaim {
    pub fn new(
        kind: RelationshipKind,
        from: EntityReference,
        to: EntityReference,
        evidence: Vec<EvidenceId>,
    ) -> Result<Self, ModelError> {
        if from.target().snapshot_id() != to.target().snapshot_id() {
            return Err(ModelError::SnapshotMismatch);
        }
        if evidence.is_empty() {
            return Err(ModelError::MissingEvidence);
        }
        Ok(Self {
            kind,
            from,
            to,
            evidence,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelError {
    ArithmeticOverflow,
    EmptyRange,
    InvalidRuleId,
    MissingEvidence,
    SnapshotMismatch,
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid inspection model value: {self:?}")
    }
}

impl std::error::Error for ModelError {}
