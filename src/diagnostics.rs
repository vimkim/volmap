//! Versioned corruption semantics owned by the inspection core.

use core::fmt;
use std::collections::BTreeMap;

use crate::model::{Coverage, EntityReference, EvidenceId, VolumeByteRange};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationBoundaryKind {
    FormatProfile,
    RequiredInput,
    SnapshotTrust,
    Volume,
    SectorBitmap,
    FileTracker,
    File,
    PageEnvelope,
    PageBody,
    SlotDirectory,
    SlotExtent,
    HeapOosReference,
    OosChain,
    Relationship,
    Operational,
    Inspector,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Requirement {
    None,
    OneOrMore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticDefinition {
    code: DiagnosticCode,
    default_severity: DiagnosticSeverity,
    anomaly: bool,
    boundary: ValidationBoundaryKind,
    required_subjects: Requirement,
    required_evidence: Requirement,
    safe_message_template: &'static str,
}

impl DiagnosticDefinition {
    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        self.code
    }

    #[must_use]
    pub const fn default_severity(self) -> DiagnosticSeverity {
        self.default_severity
    }

    #[must_use]
    pub const fn is_anomaly(self) -> bool {
        self.anomaly
    }

    #[must_use]
    pub const fn boundary(self) -> ValidationBoundaryKind {
        self.boundary
    }

    #[must_use]
    pub const fn required_subjects(self) -> Requirement {
        self.required_subjects
    }

    #[must_use]
    pub const fn required_evidence(self) -> Requirement {
        self.required_evidence
    }

    #[must_use]
    pub const fn safe_message_template(self) -> &'static str {
        self.safe_message_template
    }
}

macro_rules! diagnostic_catalog {
    ($(($constant:ident, $code:literal, $severity:ident, $anomaly:literal, $boundary:ident, $message:literal)),+ $(,)?) => {
        impl DiagnosticCode {
            $(pub const $constant: Self = Self($code);)+
        }

        const V1_DEFINITIONS: &[DiagnosticDefinition] = &[
            $(DiagnosticDefinition {
                code: DiagnosticCode::$constant,
                default_severity: DiagnosticSeverity::$severity,
                anomaly: $anomaly,
                boundary: ValidationBoundaryKind::$boundary,
                required_subjects: Requirement::OneOrMore,
                required_evidence: Requirement::OneOrMore,
                safe_message_template: $message,
            },)+
        ];
    };
}

diagnostic_catalog!(
    (
        FORMAT_UNSUPPORTED_PROFILE,
        "format.unsupported_profile",
        Fatal,
        false,
        FormatProfile,
        "The input does not match the pinned format profile."
    ),
    (
        INPUT_REQUIRED_UNREADABLE,
        "input.required_unreadable",
        Fatal,
        false,
        RequiredInput,
        "A required input could not be read."
    ),
    (
        INPUT_VOLUME_UNREADABLE,
        "input.volume_unreadable",
        Error,
        false,
        Volume,
        "A volume could not be read completely."
    ),
    (
        SNAPSHOT_MODIFIED,
        "snapshot.modified",
        Fatal,
        true,
        SnapshotTrust,
        "An input changed during inspection."
    ),
    (
        VOLUME_ENVELOPE_INVALID,
        "volume.envelope.invalid",
        Error,
        true,
        Volume,
        "The volume-header page envelope is invalid."
    ),
    (
        VOLUME_HEADER_INVALID_MAGIC,
        "volume.header.invalid_magic",
        Error,
        true,
        Volume,
        "The volume magic does not match the pinned profile."
    ),
    (
        VOLUME_HEADER_IDENTITY_MISMATCH,
        "volume.header.identity_mismatch",
        Error,
        true,
        Volume,
        "Volume identities disagree."
    ),
    (
        VOLUME_HEADER_GEOMETRY_INVALID,
        "volume.header.geometry_invalid",
        Error,
        true,
        Volume,
        "Volume geometry is invalid."
    ),
    (
        VOLUME_HEADER_STRINGS_INVALID,
        "volume.header.strings_invalid",
        Error,
        true,
        Volume,
        "Volume-header string boundaries are invalid."
    ),
    (
        VOLUME_CHAIN_CONFLICT,
        "volume.chain.conflict",
        Error,
        true,
        Relationship,
        "Volume chain claims conflict."
    ),
    (
        SECTOR_BITMAP_INVALID,
        "sector.bitmap.invalid",
        Error,
        true,
        SectorBitmap,
        "A sector bitmap boundary is invalid."
    ),
    (
        SECTOR_RESERVATION_CONFLICT,
        "sector.reservation.conflict",
        Error,
        true,
        Relationship,
        "Sector reservation claims conflict."
    ),
    (
        FILE_TRACKER_BOOTSTRAP_INVALID,
        "file.tracker.bootstrap_invalid",
        Error,
        true,
        FileTracker,
        "The file tracker cannot be bootstrapped."
    ),
    (
        FILE_TRACKER_ITEM_INVALID,
        "file.tracker.item_invalid",
        Error,
        true,
        FileTracker,
        "A file-tracker item is invalid."
    ),
    (
        FILE_HEADER_INVALID,
        "file.header.invalid",
        Error,
        true,
        File,
        "A file header is invalid."
    ),
    (
        FILE_ACCOUNTING_MISMATCH,
        "file.accounting.mismatch",
        Error,
        true,
        File,
        "File accounting claims disagree."
    ),
    (
        FILE_TABLE_LAYOUT_INVALID,
        "file.table.layout_invalid",
        Error,
        true,
        File,
        "A file table layout is invalid."
    ),
    (
        FILE_TABLE_REFERENCE_INVALID,
        "file.table.reference_invalid",
        Error,
        true,
        File,
        "A file table reference is invalid."
    ),
    (
        FILE_TABLE_CYCLE,
        "file.table.cycle",
        Error,
        true,
        File,
        "A file table chain contains a cycle."
    ),
    (
        FILE_ALLOCATION_OUT_OF_RANGE,
        "file.allocation.out_of_range",
        Error,
        true,
        File,
        "A file allocation claim is out of range."
    ),
    (
        FILE_ALLOCATION_UNRESERVED_SECTOR,
        "file.allocation.unreserved_sector",
        Error,
        true,
        Relationship,
        "A file claims an unreserved sector."
    ),
    (
        FILE_ALLOCATION_DUPLICATE_SECTOR,
        "file.allocation.duplicate_sector",
        Error,
        true,
        Relationship,
        "A file claims a sector more than once."
    ),
    (
        FILE_OWNERSHIP_CONFLICT,
        "file.ownership.conflict",
        Error,
        true,
        Relationship,
        "Page ownership claims conflict."
    ),
    (
        PAGE_ENVELOPE_IDENTITY_MISMATCH,
        "page.envelope.identity_mismatch",
        Error,
        true,
        PageEnvelope,
        "A page envelope identity does not match its location."
    ),
    (
        PAGE_ENVELOPE_LSA_MISMATCH,
        "page.envelope.lsa_mismatch",
        Error,
        true,
        PageEnvelope,
        "The page envelope LSA copies disagree."
    ),
    (
        PAGE_ENVELOPE_TYPE_UNKNOWN,
        "page.envelope.type_unknown",
        Error,
        true,
        PageEnvelope,
        "The physical page type is unknown."
    ),
    (
        PAGE_BODY_INVALID,
        "page.body.invalid",
        Error,
        true,
        PageBody,
        "A page body violates its validated format."
    ),
    (
        TDE_FLAGS_INVALID,
        "tde.flags.invalid",
        Error,
        true,
        PageEnvelope,
        "The page encryption flags are invalid."
    ),
    (
        TDE_KEY_FILE_INSECURE_PERMISSIONS,
        "tde.key_file.insecure_permissions",
        Warning,
        false,
        RequiredInput,
        "The supplied key file permissions are insecure."
    ),
    (
        TDE_KEY_ERROR,
        "tde.key_error",
        Fatal,
        false,
        RequiredInput,
        "The supplied key material cannot be used."
    ),
    (
        TDE_DECRYPTED_INVALID,
        "tde.decrypted_invalid",
        Error,
        true,
        PageBody,
        "Decrypted page structure is invalid."
    ),
    (
        SLOT_HEADER_INVALID,
        "slot.header.invalid",
        Error,
        true,
        SlotDirectory,
        "The slotted-page header is invalid."
    ),
    (
        SLOT_ENTRY_BOUNDS_INVALID,
        "slot.entry.bounds_invalid",
        Error,
        true,
        SlotExtent,
        "A slot extent is out of bounds."
    ),
    (
        SLOT_ENTRY_TYPE_INVALID,
        "slot.entry.type_invalid",
        Error,
        true,
        SlotExtent,
        "A slot record type is invalid."
    ),
    (
        SLOT_EXTENT_OVERLAP,
        "slot.extent.overlap",
        Error,
        true,
        SlotExtent,
        "Slot extents overlap."
    ),
    (
        SLOT_ACCOUNTING_MISMATCH,
        "slot.accounting.mismatch",
        Error,
        true,
        SlotDirectory,
        "Slotted-page accounting claims disagree."
    ),
    (
        HEAP_OOS_REF_DIRECTORY_INVALID,
        "heap.oos_ref.directory_invalid",
        Error,
        true,
        HeapOosReference,
        "The heap OOS-reference directory is invalid."
    ),
    (
        HEAP_OOS_REF_ENTRY_INVALID,
        "heap.oos_ref.entry_invalid",
        Error,
        true,
        HeapOosReference,
        "A heap OOS reference is invalid."
    ),
    (
        HEAP_OOS_REF_LENGTH_MISMATCH,
        "heap.oos_ref.length_mismatch",
        Error,
        true,
        HeapOosReference,
        "Heap and OOS length claims disagree."
    ),
    (
        OOS_CHUNK_HEADER_INVALID,
        "oos.chunk.header_invalid",
        Error,
        true,
        OosChain,
        "An OOS chunk header is invalid."
    ),
    (
        OOS_CHAIN_HEAD_INVALID,
        "oos.chain.head_invalid",
        Error,
        true,
        OosChain,
        "An OOS chain head is invalid."
    ),
    (
        OOS_CHAIN_TARGET_MISSING,
        "oos.chain.target_missing",
        Error,
        true,
        OosChain,
        "An OOS chain target is missing."
    ),
    (
        OOS_CHAIN_TARGET_TYPE_MISMATCH,
        "oos.chain.target_type_mismatch",
        Error,
        true,
        OosChain,
        "An OOS chain target has the wrong type."
    ),
    (
        OOS_CHAIN_INDEX_MISMATCH,
        "oos.chain.index_mismatch",
        Error,
        true,
        OosChain,
        "OOS chunk indices are not consecutive."
    ),
    (
        OOS_CHAIN_TOTAL_LENGTH_MISMATCH,
        "oos.chain.total_length_mismatch",
        Error,
        true,
        OosChain,
        "OOS total-length claims disagree."
    ),
    (
        OOS_CHAIN_PAYLOAD_LENGTH_INVALID,
        "oos.chain.payload_length_invalid",
        Error,
        true,
        OosChain,
        "An OOS chunk payload length is invalid."
    ),
    (
        OOS_CHAIN_CYCLE,
        "oos.chain.cycle",
        Error,
        true,
        OosChain,
        "An OOS chain contains a cycle."
    ),
    (
        OOS_CHAIN_UNTERMINATED,
        "oos.chain.unterminated",
        Error,
        true,
        OosChain,
        "An OOS chain does not terminate validly."
    ),
    (
        INSPECTION_RESOURCE_LIMIT,
        "inspection.resource_limit",
        Error,
        false,
        Operational,
        "An operational resource limit stopped inspection."
    ),
    (
        INSPECTION_INTERRUPTED,
        "inspection.interrupted",
        Error,
        false,
        Operational,
        "Inspection was interrupted."
    ),
    (
        INSPECTION_INTERNAL_ERROR,
        "inspection.internal_error",
        Fatal,
        false,
        Inspector,
        "An internal inspector error stopped inspection."
    ),
);

#[derive(Clone, Copy, Debug)]
pub struct DiagnosticCatalog {
    definitions: &'static [DiagnosticDefinition],
}

impl DiagnosticCatalog {
    #[must_use]
    pub const fn v1() -> Self {
        Self {
            definitions: V1_DEFINITIONS,
        }
    }

    #[must_use]
    pub const fn definitions(self) -> &'static [DiagnosticDefinition] {
        self.definitions
    }

    #[must_use]
    pub fn definition(self, code: DiagnosticCode) -> Option<DiagnosticDefinition> {
        self.definitions
            .iter()
            .copied()
            .find(|definition| definition.code == code)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // These are independent ticket-10 outcome axes.
pub struct OutcomeInputs {
    pub fatal: bool,
    pub unexpected_incomplete: bool,
    pub has_error_findings: bool,
    pub expected_limitations: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InspectionOutcome {
    Success,
    SuccessLimited,
    Findings,
    Incomplete,
    Fatal,
}

impl InspectionOutcome {
    #[must_use]
    pub const fn classify(inputs: OutcomeInputs) -> Self {
        if inputs.fatal {
            Self::Fatal
        } else if inputs.unexpected_incomplete {
            Self::Incomplete
        } else if inputs.has_error_findings {
            Self::Findings
        } else if inputs.expected_limitations {
            Self::SuccessLimited
        } else {
            Self::Success
        }
    }

    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        if self as u8 >= other as u8 {
            self
        } else {
            other
        }
    }

    #[must_use]
    pub const fn is_process_success(self) -> bool {
        matches!(self, Self::Success | Self::SuccessLimited)
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticOccurrenceId([u8; 16]);

impl DiagnosticOccurrenceId {
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
pub struct CoverageLedgerId(u64);

impl CoverageLedgerId {
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
pub enum CoverageFacet {
    VolumeTopology,
    SectorReservation,
    FileInventory,
    PageEnvelopes,
    PageBodies,
    SlotDetails,
    OosChain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedTotal {
    Known(u64),
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Remainder {
    Known(u64),
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageStopReason {
    BoundaryFailure,
    Unreadable,
    Unsupported,
    EncryptedOpaque,
    ResourceLimit,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationBoundary {
    kind: ValidationBoundaryKind,
    subject: Option<EntityReference>,
}

impl ValidationBoundary {
    #[must_use]
    pub const fn new(kind: ValidationBoundaryKind, subject: Option<EntityReference>) -> Self {
        Self { kind, subject }
    }

    #[must_use]
    pub const fn kind(self) -> ValidationBoundaryKind {
        self.kind
    }

    #[must_use]
    pub const fn subject(self) -> Option<EntityReference> {
        self.subject
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoverageStop {
    boundary: ValidationBoundary,
    reason: CoverageStopReason,
}

impl CoverageStop {
    #[must_use]
    pub const fn new(boundary: ValidationBoundary, reason: CoverageStopReason) -> Self {
        Self { boundary, reason }
    }

    #[must_use]
    pub const fn boundary(self) -> ValidationBoundary {
        self.boundary
    }

    #[must_use]
    pub const fn reason(self) -> CoverageStopReason {
        self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageLedgerInput {
    pub id: CoverageLedgerId,
    pub facet: CoverageFacet,
    pub status: Coverage,
    pub evaluated: u64,
    pub conclusive: u64,
    pub total: TrustedTotal,
    pub stopped: Option<CoverageStop>,
    pub related_diagnostics: Vec<DiagnosticOccurrenceId>,
    pub remainder: Remainder,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageLedger(CoverageLedgerInput);

impl CoverageLedger {
    pub fn new(input: CoverageLedgerInput) -> Result<Self, CoverageLedgerError> {
        if input.conclusive > input.evaluated {
            return Err(CoverageLedgerError::InvalidCounts);
        }
        if let TrustedTotal::Known(total) = input.total
            && input.evaluated > total
        {
            return Err(CoverageLedgerError::InvalidCounts);
        }

        match input.status {
            Coverage::Complete => {
                let TrustedTotal::Known(total) = input.total else {
                    return Err(CoverageLedgerError::CompleteWithoutTrustedTotal);
                };
                if input.evaluated != total
                    || input.conclusive != total
                    || input.stopped.is_some()
                    || input.remainder != Remainder::Known(0)
                {
                    return Err(CoverageLedgerError::InvalidCompleteState);
                }
            }
            Coverage::Partial => {
                if input.stopped.is_none() {
                    return Err(CoverageLedgerError::PartialWithoutStop);
                }
            }
            Coverage::NotRequested => {
                if input.evaluated != 0
                    || input.conclusive != 0
                    || input.stopped.is_some()
                    || !input.related_diagnostics.is_empty()
                {
                    return Err(CoverageLedgerError::InvalidNotRequestedState);
                }
            }
        }
        Ok(Self(input))
    }

    #[must_use]
    pub const fn id(&self) -> CoverageLedgerId {
        self.0.id
    }

    #[must_use]
    pub const fn facet(&self) -> CoverageFacet {
        self.0.facet
    }

    #[must_use]
    pub const fn status(&self) -> Coverage {
        self.0.status
    }

    #[must_use]
    pub const fn evaluated(&self) -> u64 {
        self.0.evaluated
    }

    #[must_use]
    pub const fn conclusive(&self) -> u64 {
        self.0.conclusive
    }

    #[must_use]
    pub const fn total(&self) -> TrustedTotal {
        self.0.total
    }

    #[must_use]
    pub const fn stopped(&self) -> Option<CoverageStop> {
        self.0.stopped
    }

    #[must_use]
    pub const fn remainder(&self) -> Remainder {
        self.0.remainder
    }

    #[must_use]
    pub fn related_diagnostics(&self) -> &[DiagnosticOccurrenceId] {
        &self.0.related_diagnostics
    }

    #[must_use]
    pub fn into_input(self) -> CoverageLedgerInput {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageLedgerError {
    InvalidCounts,
    CompleteWithoutTrustedTotal,
    InvalidCompleteState,
    PartialWithoutStop,
    InvalidNotRequestedState,
}

impl fmt::Display for CoverageLedgerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid coverage ledger: {self:?}")
    }
}

impl std::error::Error for CoverageLedgerError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainmentImpact {
    stopped_facets: Vec<CoverageFacet>,
    independently_usable_scopes: Vec<EntityReference>,
}

impl ContainmentImpact {
    #[must_use]
    pub const fn new(
        stopped_facets: Vec<CoverageFacet>,
        independently_usable_scopes: Vec<EntityReference>,
    ) -> Self {
        Self {
            stopped_facets,
            independently_usable_scopes,
        }
    }

    #[must_use]
    pub fn stopped_facets(&self) -> &[CoverageFacet] {
        &self.stopped_facets
    }

    #[must_use]
    pub fn independently_usable_scopes(&self) -> &[EntityReference] {
        &self.independently_usable_scopes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticValue {
    Boolean(bool),
    Signed(i64),
    Unsigned(u64),
    Code(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticOccurrenceInput {
    pub id: DiagnosticOccurrenceId,
    pub code: DiagnosticCode,
    pub affected: Vec<EntityReference>,
    pub evidence: Vec<EvidenceId>,
    pub byte_locators: Vec<VolumeByteRange>,
    pub failed_boundary: ValidationBoundary,
    pub containment: ContainmentImpact,
    pub coverage_ledgers: Vec<CoverageLedgerId>,
    pub parameters: BTreeMap<&'static str, DiagnosticValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticOccurrence {
    input: DiagnosticOccurrenceInput,
    severity: DiagnosticSeverity,
    anomaly: bool,
    message: &'static str,
}

impl DiagnosticOccurrence {
    pub fn new(
        catalog: DiagnosticCatalog,
        input: DiagnosticOccurrenceInput,
    ) -> Result<Self, DiagnosticOccurrenceError> {
        let definition = catalog
            .definition(input.code)
            .ok_or(DiagnosticOccurrenceError::UnknownCode)?;
        if definition.required_subjects() == Requirement::OneOrMore && input.affected.is_empty() {
            return Err(DiagnosticOccurrenceError::MissingSubjects);
        }
        if definition.required_evidence() == Requirement::OneOrMore && input.evidence.is_empty() {
            return Err(DiagnosticOccurrenceError::MissingEvidence);
        }
        if definition.boundary() != input.failed_boundary.kind() {
            return Err(DiagnosticOccurrenceError::BoundaryMismatch);
        }
        Ok(Self {
            input,
            severity: definition.default_severity(),
            anomaly: definition.is_anomaly(),
            message: definition.safe_message_template(),
        })
    }

    #[must_use]
    pub const fn id(&self) -> DiagnosticOccurrenceId {
        self.input.id
    }

    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.input.code
    }

    #[must_use]
    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    #[must_use]
    pub const fn is_anomaly(&self) -> bool {
        self.anomaly
    }

    #[must_use]
    pub const fn message(&self) -> &'static str {
        self.message
    }

    #[must_use]
    pub fn affected(&self) -> &[EntityReference] {
        &self.input.affected
    }

    #[must_use]
    pub fn evidence(&self) -> &[EvidenceId] {
        &self.input.evidence
    }

    #[must_use]
    pub fn byte_locators(&self) -> &[VolumeByteRange] {
        &self.input.byte_locators
    }

    #[must_use]
    pub const fn failed_boundary(&self) -> ValidationBoundary {
        self.input.failed_boundary
    }

    #[must_use]
    pub const fn containment(&self) -> &ContainmentImpact {
        &self.input.containment
    }

    #[must_use]
    pub fn coverage_ledgers(&self) -> &[CoverageLedgerId] {
        &self.input.coverage_ledgers
    }

    #[must_use]
    pub const fn parameters(&self) -> &BTreeMap<&'static str, DiagnosticValue> {
        &self.input.parameters
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticOccurrenceError {
    UnknownCode,
    MissingSubjects,
    MissingEvidence,
    BoundaryMismatch,
}

impl fmt::Display for DiagnosticOccurrenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid diagnostic occurrence: {self:?}")
    }
}

impl std::error::Error for DiagnosticOccurrenceError {}
