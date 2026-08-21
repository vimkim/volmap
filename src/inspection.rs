//! Revision-pinned inspection seam over stable volume sources.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io;
use std::mem::size_of;
use std::os::unix::fs::{DirBuilderExt, FileExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::bytes::ByteView;
use crate::diagnostics::{InspectionOutcome, OutcomeInputs};
use crate::format::{
    BOOT_DB_PARM_SIZE, BtreePageFact, CatalogClassInfoFact, CatalogPageFact,
    CatalogRepresentationHeaderFact, ClassRepresentationFact, DB_PAGE_SIZE, DecodeError,
    DroppedFilesPageFact, FileHeader, FileType, HeapPageFact, InterpretedAttribute, OosNext,
    PageContent, PageType, RecordLayoutFact, RecordType, RepresentationTarget, SLOTTED_HEADER_SIZE,
    SlottedPage, TDE_KEY_INFO_RECORD_SIZE, TdeAlgorithm, TrackerItemFact, UserPageFact,
    VacuumPageFact, VolumePurpose, VolumeType, decode_bigone_target, decode_boot_db_parm,
    decode_btree_page, decode_catalog_class_info, decode_catalog_directory, decode_catalog_page,
    decode_catalog_representation_header, decode_class_representation,
    decode_decrypted_page_envelope, decode_dropped_files_page, decode_extdata_header,
    decode_file_header, decode_full_sectors, decode_heap_page, decode_heap_record_body,
    decode_heap_record_envelope, decode_oos_chunk, decode_overflow_continuation,
    decode_overflow_head, decode_page_envelope, decode_page_envelope_parts, decode_partial_sectors,
    decode_record_interpretation, decode_relocation_target, decode_sector_bitmap,
    decode_slotted_free_space_header, decode_slotted_page, decode_tracker_items, decode_user_pages,
    decode_vacuum_page, decode_volume_header,
};
use crate::model::{
    Availability, Coverage, Hfid, InspectionRevision, Oid, PageAllocationClass, PageId, SectorId,
    SlotId, SnapshotId, SnapshotValidity, TdeInspectionState, Vfid, VolId, Vpid,
};
use crate::source::{InputSpec, SourceError, SourceSet, VolumeHandle, discover};
use crate::tde::{
    PermanentDataKey, TdeError, decode_key_info_record, decrypt_page_user_region,
    load_permanent_key,
};

const FORMAT_PROFILE: &str = "cubrid-feat-oos-linux-x86_64-gcc-e1e651de";
const SECTOR_PAGES: u32 = 64;
const PACKED_PAGE_FACT_SIZE: u64 = 16;
const PACKED_PAGE_FACT_SIZE_USIZE: usize = 16;
const PACKED_FACT_LSA_PRESENT: u8 = 0x80;
const PACKED_FACT_DIAGNOSTIC_MASK: u8 = 0x07;
const PACKED_FACT_OCCUPANCY_MASK: u8 = 0x78;
const PACKED_FACT_OCCUPANCY_SHIFT: u8 = 3;
const OCCUPANCY_LEVELS: u8 = 15;
const WORKER_PAGE_BATCH: usize = 16;
const TERMINAL_DIAGNOSTIC_RESERVE: u64 = 512;
static NEXT_SPILL_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourcePolicy {
    pub memory_limit: u64,
    pub spill_limit: u64,
    pub workers: u32,
    pub max_chain_steps: u64,
    pub max_decoded_bytes: u64,
}

impl ResourcePolicy {
    pub fn new(
        memory_limit: u64,
        spill_limit: u64,
        workers: u32,
        max_chain_steps: u64,
        max_decoded_bytes: u64,
    ) -> Result<Self, ResourcePolicyError> {
        if memory_limit == 0
            || spill_limit == 0
            || workers == 0
            || max_chain_steps == 0
            || max_decoded_bytes == 0
        {
            return Err(ResourcePolicyError::ZeroValue);
        }
        Ok(Self {
            memory_limit,
            spill_limit,
            workers,
            max_chain_steps,
            max_decoded_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourcePolicyError {
    ZeroValue,
}

impl fmt::Display for ResourcePolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("resource policy values must be nonzero")
    }
}

impl std::error::Error for ResourcePolicyError {}

/// Whether a reading treats its input as immutable or follows a live one.
/// It selects the consequence of an input change, not the reading itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMode {
    /// The offline contract: a changed input invalidates the snapshot.
    Immutable,
    /// The follow contract: a changed input supersedes this generation and the
    /// caller is expected to re-read.
    Live,
}

#[derive(Clone, Debug)]
pub struct OpenRequest {
    pub input: InputSpec,
    pub tde_keys_file: Option<PathBuf>,
    /// Parent directory for the private, unlinked spill file.
    pub spill_directory: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct CancelToken(Arc<AtomicBool>);

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancelToken {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanPhase {
    Discovery,
    VolumeGeometry,
    SectorReservation,
    PageEnvelopes,
    Reconciliation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanProgress {
    pub phase: ScanPhase,
    pub completed: u64,
    pub trusted_total: Option<u64>,
}

pub trait ProgressObserver {
    fn update(&mut self, progress: ScanProgress);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionSelector {
    Latest,
    Exact(InspectionRevision),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageDetailSupport {
    Semantic,
    StructuralOnly,
    Opaque,
}

impl PageDetailSupport {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::StructuralOnly => "structural-only",
            Self::Opaque => "opaque",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticRecord {
    pub code: &'static str,
    pub severity: &'static str,
    pub message: &'static str,
    pub subject: String,
    pub rule: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoverageRecord {
    pub facet: &'static str,
    pub coverage: Coverage,
    pub evaluated: u64,
    pub conclusive: u64,
    pub trusted_total: Option<u64>,
    pub stop_reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VolumeView {
    pub vol_id: VolId,
    pub purpose: VolumePurpose,
    pub volume_type: VolumeType,
    pub total_sectors: u32,
    pub maximum_sectors: u32,
    pub system_last_page: PageId,
    pub reserved_sectors: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageView {
    pub vpid: Vpid,
    pub sector_id: SectorId,
    pub allocation: PageAllocationClass,
    pub page_type: Option<PageType>,
    pub availability: Availability,
    pub tde_state: TdeInspectionState,
    pub detail_support: Option<PageDetailSupport>,
    pub slotted_occupied_percent: Option<u8>,
    pub lsa_word: Option<u64>,
    pub diagnostic_code: Option<&'static str>,
    pub file_association: PageFileAssociation,
}

/// The stored class/table name a class OID resolved to, or the typed reason
/// it did not. A name is never manufactured; the OID stays visible either way.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassNameResolution {
    Resolved(Arc<str>),
    Unresolved(&'static str),
}

/// Class association carried by a file descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassAssociation {
    /// The file type carries no single class association (typed reason).
    None(&'static str),
    Class {
        oid: Oid,
        name: ClassNameResolution,
    },
}

/// One file's association facts, joined from the validated file inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileAssociation {
    pub vfid: Vfid,
    pub file_type: Option<FileType>,
    pub class: ClassAssociation,
}

/// How a physical page relates to the file inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PageFileAssociation {
    /// No file allocates or reserves this page.
    None,
    /// The page is an allocated page of this file.
    Allocated(FileAssociation),
    /// The page is inside a sector reserved by this file but not allocated.
    ReservedFor(FileAssociation),
    /// Multiple file tables claim this page's sector; no owner is selected.
    MixedClaims,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectorClaimKind {
    Full,
    Partial { bitmap: u64 },
}

impl SectorClaimKind {
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // a u64 popcount is at most 64
    pub const fn allocated_pages(self) -> u8 {
        match self {
            Self::Full => 64,
            Self::Partial { bitmap } => bitmap.count_ones() as u8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectorClaimView {
    pub vfid: Vfid,
    pub kind: SectorClaimKind,
}

/// How a sector relates to the file inventory's reservation claims.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SectorAttribution {
    /// No validated file table claims this sector.
    Unclaimed,
    /// Exactly one file claims the sector.
    Single {
        association: FileAssociation,
        kind: SectorClaimKind,
    },
    /// Multiple file tables claim this sector; every claim is retained.
    Mixed { claims: Vec<SectorClaimView> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectorView {
    pub vol_id: VolId,
    pub sector_id: SectorId,
    pub reserved: bool,
    pub attribution: SectorAttribution,
    pub pages: Vec<PageView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverviewView {
    pub snapshot_id: SnapshotId,
    pub revision: InspectionRevision,
    pub validity: SnapshotValidity,
    pub format_profile: &'static str,
    pub input_kind: &'static str,
    pub outcome: InspectionOutcome,
    pub volume_count: u64,
    pub sector_count: u64,
    pub reserved_sector_count: u64,
    pub physical_page_count: u64,
    pub inspected_page_envelopes: u64,
    pub page_type_counts: Vec<(PageType, u64)>,
    pub tde_opaque_pages: u64,
    pub coverage: Vec<CoverageRecord>,
    pub diagnostics: Vec<DiagnosticRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FastScanResources {
    pub memory_limit: u64,
    pub spill_limit: u64,
    pub requested_workers: u32,
    pub admitted_workers: u32,
    pub max_chain_steps: u64,
    pub max_decoded_bytes: u64,
    pub admitted_resident_bytes: u64,
    pub spill_bytes: u64,
    pub packed_fact_bytes: u64,
    pub envelope_read_attempts: u64,
    pub envelope_requested_bytes: u64,
    pub slotted_header_read_attempts: u64,
    pub slotted_header_requested_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PageFastFact {
    page_id: PageId,
    page_type: Option<PageType>,
    availability: Availability,
    tde_state: TdeInspectionState,
    slotted_occupancy_units: Option<u8>,
    lsa_word: Option<u64>,
    diagnostic_code: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PackedPageFact([u8; PACKED_PAGE_FACT_SIZE_USIZE]);

impl PackedPageFact {
    fn pack(fact: PageFastFact) -> Self {
        let mut bytes = [0_u8; PACKED_PAGE_FACT_SIZE_USIZE];
        bytes[..4].copy_from_slice(&fact.page_id.get().to_le_bytes());
        if let Some(lsa_word) = fact.lsa_word {
            bytes[4..12].copy_from_slice(&lsa_word.to_le_bytes());
            bytes[15] |= PACKED_FACT_LSA_PRESENT;
        }
        bytes[12] = fact.page_type.map_or(u8::MAX, PageType::ordinal);
        bytes[13] = match fact.availability {
            Availability::Available => 0,
            Availability::Unreadable => 1,
            Availability::Unsupported => 2,
            Availability::EncryptedOpaque => 3,
        };
        bytes[14] = match fact.tde_state {
            TdeInspectionState::NotEncrypted => 0,
            TdeInspectionState::Decrypted => 1,
            TdeInspectionState::EncryptedOpaque => 2,
            TdeInspectionState::KeyError => 3,
            TdeInspectionState::DecryptedInvalid => 4,
            TdeInspectionState::InvalidFlags => 5,
        };
        if let Some(units) = fact.slotted_occupancy_units {
            debug_assert!((1..=OCCUPANCY_LEVELS).contains(&units));
            bytes[15] |= units << PACKED_FACT_OCCUPANCY_SHIFT;
        }
        bytes[15] |= match fact.diagnostic_code {
            None => 0,
            Some("page.envelope.identity_mismatch") => 1,
            Some("page.envelope.lsa_mismatch") => 2,
            Some("page.envelope.type_unknown") => 3,
            Some("page.envelope.tde_flags_invalid") => 4,
            Some("page.envelope.invalid") => 5,
            Some("tde.decrypted_invalid") => 6,
            Some(_) => 7,
        };
        Self(bytes)
    }

    fn unpack(self) -> Result<PageFastFact, FactStoreError> {
        let page_id = PageId::new(i32::from_le_bytes(
            self.0[..4]
                .try_into()
                .map_err(|_| FactStoreError::InvalidRecord)?,
        ))
        .map_err(|_| FactStoreError::InvalidRecord)?;
        let page_type = if self.0[12] == u8::MAX {
            None
        } else {
            Some(page_type_from_ordinal(self.0[12]).ok_or(FactStoreError::InvalidRecord)?)
        };
        let availability = match self.0[13] {
            0 => Availability::Available,
            1 => Availability::Unreadable,
            2 => Availability::Unsupported,
            3 => Availability::EncryptedOpaque,
            _ => return Err(FactStoreError::InvalidRecord),
        };
        let tde_state = match self.0[14] {
            0 => TdeInspectionState::NotEncrypted,
            1 => TdeInspectionState::Decrypted,
            2 => TdeInspectionState::EncryptedOpaque,
            3 => TdeInspectionState::KeyError,
            4 => TdeInspectionState::DecryptedInvalid,
            5 => TdeInspectionState::InvalidFlags,
            _ => return Err(FactStoreError::InvalidRecord),
        };
        let metadata = self.0[15];
        let diagnostic_code = match metadata & PACKED_FACT_DIAGNOSTIC_MASK {
            0 => None,
            1 => Some("page.envelope.identity_mismatch"),
            2 => Some("page.envelope.lsa_mismatch"),
            3 => Some("page.envelope.type_unknown"),
            4 => Some("page.envelope.tde_flags_invalid"),
            5 => Some("page.envelope.invalid"),
            6 => Some("tde.decrypted_invalid"),
            _ => return Err(FactStoreError::InvalidRecord),
        };
        let occupancy_units =
            (metadata & PACKED_FACT_OCCUPANCY_MASK) >> PACKED_FACT_OCCUPANCY_SHIFT;
        let slotted_occupancy_units = (occupancy_units != 0).then_some(occupancy_units);
        let lsa_word = if metadata & PACKED_FACT_LSA_PRESENT == 0 {
            None
        } else {
            Some(u64::from_le_bytes(
                self.0[4..12]
                    .try_into()
                    .map_err(|_| FactStoreError::InvalidRecord)?,
            ))
        };
        Ok(PageFastFact {
            page_id,
            page_type,
            availability,
            tde_state,
            slotted_occupancy_units,
            lsa_word,
            diagnostic_code,
        })
    }
}

#[derive(Debug)]
struct SpillFile(File);

impl SpillFile {
    fn create(parent: Option<&Path>) -> Result<Self, io::Error> {
        let parent = parent.unwrap_or_else(|| Path::new("/tmp"));
        let metadata = std::fs::symlink_metadata(parent)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "spill parent must be a real directory",
            ));
        }
        for _ in 0..128 {
            let sequence = NEXT_SPILL_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let directory = parent.join(format!(
                ".volmap-spill-{}-{timestamp:x}-{sequence:x}",
                std::process::id()
            ));
            match DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => {
                    let path = directory.join("facts");
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(&path);
                    match file {
                        Ok(file) => {
                            std::fs::remove_file(&path)?;
                            std::fs::remove_dir(&directory)?;
                            return Ok(Self(file));
                        }
                        Err(error) => {
                            let _ = std::fs::remove_dir(&directory);
                            return Err(error);
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a private spill directory",
        ))
    }

    fn read(&self, offset: u64) -> Result<PackedPageFact, FactStoreError> {
        let mut bytes = [0_u8; PACKED_PAGE_FACT_SIZE_USIZE];
        self.0
            .read_exact_at(&mut bytes, offset)
            .map_err(|_| FactStoreError::Io)?;
        Ok(PackedPageFact(bytes))
    }

    fn write(&self, offset: u64, fact: PackedPageFact) -> Result<(), FactStoreError> {
        self.0
            .write_all_at(&fact.0, offset)
            .map_err(|_| FactStoreError::Io)
    }
}

#[derive(Debug)]
enum FactStoreError {
    Io,
    InvalidRecord,
    Arithmetic,
}

#[derive(Clone, Debug)]
enum PageFactStore {
    Memory(Vec<PackedPageFact>),
    Spilled {
        file: Arc<SpillFile>,
        offset: u64,
        len: u64,
    },
}

impl PageFactStore {
    fn memory() -> Self {
        Self::Memory(Vec::new())
    }

    fn memory_with_capacity(capacity: u64) -> Result<Self, OpenFailure> {
        Ok(Self::Memory(Vec::with_capacity(
            usize::try_from(capacity).map_err(|_| OpenFailure::Arithmetic)?,
        )))
    }

    fn spilled(file: Arc<SpillFile>, offset: u64) -> Self {
        Self::Spilled {
            file,
            offset,
            len: 0,
        }
    }

    fn len(&self) -> u64 {
        match self {
            Self::Memory(facts) => facts.len() as u64,
            Self::Spilled { len, .. } => *len,
        }
    }

    fn push(&mut self, fact: PageFastFact) -> Result<(), FactStoreError> {
        let packed = PackedPageFact::pack(fact);
        match self {
            Self::Memory(facts) => {
                facts.push(packed);
                Ok(())
            }
            Self::Spilled { file, offset, len } => {
                let relative = len
                    .checked_mul(PACKED_PAGE_FACT_SIZE)
                    .ok_or(FactStoreError::Arithmetic)?;
                let position = offset
                    .checked_add(relative)
                    .ok_or(FactStoreError::Arithmetic)?;
                file.write(position, packed)?;
                *len = len.checked_add(1).ok_or(FactStoreError::Arithmetic)?;
                Ok(())
            }
        }
    }

    fn fact_at(&self, index: u64) -> Result<Option<PageFastFact>, FactStoreError> {
        if index >= self.len() {
            return Ok(None);
        }
        let packed = match self {
            Self::Memory(facts) => *facts
                .get(usize::try_from(index).map_err(|_| FactStoreError::Arithmetic)?)
                .ok_or(FactStoreError::InvalidRecord)?,
            Self::Spilled { file, offset, .. } => {
                let relative = index
                    .checked_mul(PACKED_PAGE_FACT_SIZE)
                    .ok_or(FactStoreError::Arithmetic)?;
                file.read(
                    offset
                        .checked_add(relative)
                        .ok_or(FactStoreError::Arithmetic)?,
                )?
            }
        };
        packed.unpack().map(Some)
    }

    fn page_fact(&self, page_id: PageId) -> Result<Option<PageFastFact>, FactStoreError> {
        let mut low = 0_u64;
        let mut high = self.len();
        while low < high {
            let middle = low + (high - low) / 2;
            let fact = self.fact_at(middle)?.ok_or(FactStoreError::InvalidRecord)?;
            match fact.page_id.get().cmp(&page_id.get()) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Ok(Some(fact)),
            }
        }
        Ok(None)
    }
}

#[derive(Clone, Debug, Default)]
struct PageFactSummary {
    inspected: u64,
    page_type_counts: [u64; 15],
    tde_opaque_pages: u64,
}

#[derive(Clone, Copy, Debug)]
enum EnvelopeScanResult {
    Fact {
        fact: PageFastFact,
        diagnostic: Option<(&'static str, &'static str)>,
    },
    Unreadable(PageId),
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default)]
struct EligiblePageCursor {
    sector: u32,
    within: u32,
}

impl EligiblePageCursor {
    fn next(&mut self, volume: &VolumeRecord) -> Result<Option<PageId>, OpenFailure> {
        let system_last = u32::try_from(volume.view.system_last_page.get())
            .map_err(|_| OpenFailure::Arithmetic)?;
        while self.sector < volume.view.total_sectors {
            let first_page = self
                .sector
                .checked_mul(SECTOR_PAGES)
                .ok_or(OpenFailure::Arithmetic)?;
            if self.within == 0 && !volume.is_reserved(self.sector) && first_page > system_last {
                self.sector = self.sector.checked_add(1).ok_or(OpenFailure::Arithmetic)?;
                continue;
            }
            let raw_page = first_page
                .checked_add(self.within)
                .ok_or(OpenFailure::Arithmetic)?;
            self.within = self.within.checked_add(1).ok_or(OpenFailure::Arithmetic)?;
            if self.within == SECTOR_PAGES {
                self.within = 0;
                self.sector = self.sector.checked_add(1).ok_or(OpenFailure::Arithmetic)?;
            }
            return PageId::new(i32::try_from(raw_page).map_err(|_| OpenFailure::Arithmetic)?)
                .map(Some)
                .map_err(|_| OpenFailure::Arithmetic);
        }
        Ok(None)
    }
}

impl PageFactSummary {
    fn observe(&mut self, fact: PageFastFact) -> Result<(), OpenFailure> {
        self.inspected = self
            .inspected
            .checked_add(1)
            .ok_or(OpenFailure::Arithmetic)?;
        if let Some(page_type) = fact.page_type {
            let count = self
                .page_type_counts
                .get_mut(usize::from(page_type.ordinal()))
                .ok_or(OpenFailure::Arithmetic)?;
            *count = count.checked_add(1).ok_or(OpenFailure::Arithmetic)?;
        }
        if fact.tde_state == TdeInspectionState::EncryptedOpaque {
            self.tde_opaque_pages = self
                .tde_opaque_pages
                .checked_add(1)
                .ok_or(OpenFailure::Arithmetic)?;
        }
        Ok(())
    }
}

fn scan_page_envelope(
    source: &VolumeHandle,
    page_id: PageId,
    has_tde_key: bool,
    cancel: &CancelToken,
) -> EnvelopeScanResult {
    if cancel.is_cancelled() {
        return EnvelopeScanResult::Cancelled;
    }
    let Ok((prefix, watermark)) = source.read_envelope(page_id) else {
        return EnvelopeScanResult::Unreadable(page_id);
    };
    match decode_page_envelope_parts(&prefix, &watermark, source.vpid(page_id)) {
        Ok(summary) => {
            let page_type = summary.page_type();
            let (availability, tde_state) = match summary.content() {
                PageContent::Plaintext => {
                    (Availability::Available, TdeInspectionState::NotEncrypted)
                }
                PageContent::EncryptedOpaque { .. } => (
                    if has_tde_key {
                        Availability::Available
                    } else {
                        Availability::EncryptedOpaque
                    },
                    if has_tde_key {
                        TdeInspectionState::Decrypted
                    } else {
                        TdeInspectionState::EncryptedOpaque
                    },
                ),
                PageContent::Decrypted { .. } => {
                    unreachable!("fast envelope decoding cannot produce decrypted content")
                }
            };
            let slotted_occupancy_units = if matches!(summary.content(), PageContent::Plaintext)
                && page_uses_slotted_layout(page_type, None)
            {
                source
                    .read_page_user_prefix::<SLOTTED_HEADER_SIZE>(page_id)
                    .ok()
                    .and_then(|header| decode_slotted_free_space_header(&header).ok())
                    .and_then(slotted_occupancy_units)
            } else {
                None
            };
            EnvelopeScanResult::Fact {
                fact: PageFastFact {
                    page_id,
                    page_type: Some(page_type),
                    availability,
                    tde_state,
                    slotted_occupancy_units,
                    lsa_word: Some(summary.lsa_word()),
                    diagnostic_code: None,
                },
                diagnostic: None,
            }
        }
        Err(error) => {
            let code = page_diagnostic_code(&error);
            EnvelopeScanResult::Fact {
                fact: PageFastFact {
                    page_id,
                    page_type: None,
                    availability: Availability::Unreadable,
                    tde_state: if error.rule() == "page.envelope.tde_flags" {
                        TdeInspectionState::InvalidFlags
                    } else {
                        TdeInspectionState::NotEncrypted
                    },
                    slotted_occupancy_units: None,
                    lsa_word: None,
                    diagnostic_code: Some(code),
                },
                diagnostic: Some((code, error.rule())),
            }
        }
    }
}

fn slotted_occupancy_units(total_free: u32) -> Option<u8> {
    let page_bytes = u32::try_from(DB_PAGE_SIZE).ok()?;
    let occupied = page_bytes.checked_sub(total_free)?;
    let levels = u32::from(OCCUPANCY_LEVELS);
    let units = occupied
        .checked_mul(levels)?
        .div_ceil(page_bytes)
        .clamp(1, levels);
    u8::try_from(units).ok()
}

fn slotted_occupied_percent(units: u8) -> u8 {
    let percent = u32::from(units)
        .saturating_mul(100)
        .saturating_add(u32::from(OCCUPANCY_LEVELS / 2))
        / u32::from(OCCUPANCY_LEVELS);
    u8::try_from(percent).unwrap_or(100)
}

fn retained_diagnostic_bytes(diagnostic: &DiagnosticRecord) -> Result<u64, OpenFailure> {
    let records = u64::try_from(size_of::<DiagnosticRecord>())
        .ok()
        .and_then(|size| size.checked_mul(2))
        .ok_or(OpenFailure::Arithmetic)?;
    records
        .checked_add(u64::try_from(diagnostic.subject.len()).map_err(|_| OpenFailure::Arithmetic)?)
        .and_then(|size| size.checked_add(32))
        .ok_or(OpenFailure::Arithmetic)
}

fn admit_diagnostic(
    resident_used: &mut u64,
    memory_limit: u64,
    diagnostic: &DiagnosticRecord,
) -> Result<bool, OpenFailure> {
    let bytes = retained_diagnostic_bytes(diagnostic)?;
    let admitted = resident_used
        .checked_add(bytes)
        .is_some_and(|next| next <= memory_limit);
    if admitted {
        *resident_used = resident_used
            .checked_add(bytes)
            .ok_or(OpenFailure::Arithmetic)?;
    }
    Ok(admitted)
}

fn resource_limit_diagnostic(
    vol_id: VolId,
    page_id: PageId,
    rule: &'static str,
) -> DiagnosticRecord {
    DiagnosticRecord {
        code: "inspection.resource_limit",
        severity: "error",
        message: "The admitted fact budget stopped page-envelope inspection.",
        subject: format!("page:{}:{}", vol_id.get(), page_id.get()),
        rule,
    }
}

#[derive(Clone, Debug)]
struct VolumeRecord {
    view: VolumeView,
    reserved_masks: Vec<u64>,
    pages: PageFactStore,
}

impl VolumeRecord {
    fn is_reserved(&self, sector: u32) -> bool {
        let word = usize::try_from(sector / 64).ok();
        word.and_then(|index| self.reserved_masks.get(index))
            .is_some_and(|mask| mask & (1_u64 << (sector % 64)) != 0)
    }

    fn page_fact(&self, page_id: PageId) -> Result<Option<PageFastFact>, FactStoreError> {
        self.pages.page_fact(page_id)
    }
}

#[derive(Clone, Debug)]
struct DeepPageFact {
    slotted: Option<SlottedPage>,
    file_header: Option<FileHeader>,
    raw: Option<RawPageView>,
    diagnostic_rule: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawPageView {
    Btree(BtreePageFact),
    Catalog(CatalogPageView),
    Heap(HeapPageFact),
    Vacuum(VacuumPageFact),
    DroppedFiles(DroppedFilesPageFact),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogPageView {
    pub page: CatalogPageFact,
    pub directories: Vec<CatalogDirectoryView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogDirectoryView {
    pub slot_id: u16,
    pub class_oid: Option<Oid>,
    pub class_info: CatalogClassInfoFact,
    pub representations: Vec<CatalogRepresentationHeaderFact>,
}

#[derive(Clone, Debug)]
struct OosChainFact {
    total_data_length: Option<u32>,
    validated_payload_bytes: u64,
    complete: bool,
    chunks: Vec<OosChunkView>,
    diagnostic_rule: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OosChunkView {
    pub oid: crate::model::Oid,
    pub total_data_length: u32,
    pub chunk_index: u32,
    pub next: Option<crate::model::Oid>,
    pub payload_offset: u16,
    pub payload_length: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OosChainView {
    pub head: crate::model::Oid,
    pub revision: InspectionRevision,
    pub total_data_length: Option<u32>,
    pub validated_payload_bytes: u64,
    pub complete: bool,
    pub chunks: Vec<OosChunkView>,
    pub diagnostic_rule: Option<&'static str>,
}

#[derive(Clone, Debug)]
struct OverflowChainFact {
    head: Option<Vpid>,
    total_data_length: Option<u32>,
    validated_payload_bytes: u64,
    complete: bool,
    pages: Vec<OverflowPageView>,
    diagnostic_rule: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverflowPageView {
    pub vpid: Vpid,
    pub head: bool,
    pub next: Option<Vpid>,
    pub payload_offset: u16,
    pub payload_length: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverflowChainView {
    pub source: Oid,
    pub revision: InspectionRevision,
    pub head: Option<Vpid>,
    pub total_data_length: Option<u32>,
    pub validated_payload_bytes: u64,
    pub complete: bool,
    pub pages: Vec<OverflowPageView>,
    pub diagnostic_rule: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelocationEdgeView {
    pub source: Oid,
    pub revision: InspectionRevision,
    pub target: Option<Oid>,
    pub valid: bool,
    pub diagnostic_rule: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeepPageView {
    pub vpid: Vpid,
    pub revision: InspectionRevision,
    pub slotted: Option<SlottedPage>,
    pub file_header: Option<FileHeader>,
    pub raw: Option<RawPageView>,
    pub diagnostic_rule: Option<&'static str>,
}

#[derive(Clone, Debug)]
struct SessionData {
    sources: Arc<SourceSet>,
    source_mode: SourceMode,
    tde_key: Option<Arc<PermanentDataKey>>,
    snapshot_id: SnapshotId,
    revision: InspectionRevision,
    validity: SnapshotValidity,
    outcome: InspectionOutcome,
    volumes: Vec<VolumeRecord>,
    coverage: Vec<CoverageRecord>,
    diagnostics: Vec<DiagnosticRecord>,
    fast_summary: PageFactSummary,
    fast_scan_resources: FastScanResources,
    page_overrides: BTreeMap<Vpid, PageFastFact>,
    deep_pages: BTreeMap<Vpid, DeepPageFact>,
    file_allocations: BTreeMap<Vpid, Vfid>,
    tracked_files: BTreeMap<Vfid, FileHeader>,
    sector_claims: BTreeMap<(VolId, SectorId), Vec<SectorClaimView>>,
    class_names: BTreeMap<Oid, ClassNameResolution>,
    oos_chains: BTreeMap<crate::model::Oid, OosChainFact>,
    overflow_chains: BTreeMap<Oid, OverflowChainFact>,
    relocation_edges: BTreeMap<Oid, RelocationEdgeFact>,
    class_representations: BTreeMap<(Oid, u32), ClassRepresentationEvidence>,
    /// A heap sector belongs to exactly one file and a class has exactly one
    /// heap file, so the class a sector's pages hold is cacheable. The key must
    /// carry the volume: one class's data pages may span permanent volumes
    /// (`docs/adr/0002-classrepr-from-class-record.md`).
    sector_classes: BTreeMap<(VolId, SectorId), Oid>,
    record_interpretations: BTreeMap<Oid, RecordInterpretationEvidence>,
    /// Why one page's records were not interpreted. Kept per page because a
    /// whole-page degradation produces no per-record fact to hang a reason on,
    /// and the adapter still has to explain itself.
    interpretation_failures: BTreeMap<Vpid, &'static str>,
}

/// One representation of one class, as revision-scoped graph evidence.
#[derive(Clone, Debug)]
struct ClassRepresentationEvidence {
    representation: Option<ClassRepresentationFact>,
    diagnostic_rule: Option<&'static str>,
}

/// One record's decoded attribute values, as revision-scoped graph evidence.
#[derive(Clone, Debug)]
struct RecordInterpretationEvidence {
    class_oid: Oid,
    representation_id: u32,
    record_type: RecordType,
    /// Set when this interpretation was reached by following a relocation from
    /// another slot, so both the forward reference and the values stay visible.
    relocated_from: Option<Oid>,
    layout: Option<RecordLayoutFact>,
    attributes: Vec<InterpretedAttribute>,
    diagnostic_rule: Option<&'static str>,
}

/// One representation of one class as the graph exposes it.
#[derive(Clone, Debug)]
pub struct ClassRepresentationView {
    pub class_oid: Oid,
    pub representation_id: u32,
    pub revision: InspectionRevision,
    /// Absent when the class record could not be interpreted; `diagnostic_rule`
    /// then says why, so the failure stays visible instead of vanishing.
    pub representation: Option<ClassRepresentationFact>,
    pub diagnostic_rule: Option<&'static str>,
}

/// One record's decoded attribute values as the graph exposes them.
#[derive(Clone, Debug)]
pub struct RecordInterpretationView {
    pub record: Oid,
    pub revision: InspectionRevision,
    pub class_oid: Oid,
    pub representation_id: u32,
    pub record_type: RecordType,
    pub relocated_from: Option<Oid>,
    /// Absent when the record could not be interpreted at all.
    pub layout: Option<RecordLayoutFact>,
    pub attributes: Vec<InterpretedAttribute>,
    pub diagnostic_rule: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
struct RelocationEdgeFact {
    target: Option<Oid>,
    valid: bool,
    diagnostic_rule: Option<&'static str>,
}

#[derive(Clone, Copy)]
enum FileTableKind {
    Partial,
    Full,
    User,
    Tracker,
}

/// Everything one validated file table proves about its space: the allocated
/// pages and the per-sector reservation claims those pages came from.
struct FileAllocationFacts {
    pages: BTreeSet<Vpid>,
    claims: Vec<((VolId, SectorId), SectorClaimKind)>,
}

#[derive(Default)]
struct FileTraversal {
    table_pages: BTreeSet<Vpid>,
    partial_sectors: Vec<crate::format::PartialSectorFact>,
    full_sectors: Vec<(VolId, SectorId)>,
    user_pages: Vec<UserPageFact>,
    tracker_items: Vec<TrackerItemFact>,
    decoded_bytes: u64,
    steps: u64,
}

enum FileTraversalError {
    Decode(&'static str),
    Operation(OperationError),
}

struct OwnedInspectionPage {
    physical: Box<[u8; crate::format::IO_PAGE_SIZE]>,
    decrypted_user: Option<Zeroizing<Vec<u8>>>,
}

enum InspectionPageError {
    Source(SourceError),
    Format(&'static str),
    EncryptedOpaque,
    Decrypt,
}

impl InspectionPageError {
    fn into_operation(self) -> OperationError {
        match self {
            Self::Source(error) => OperationError::Source(error),
            Self::Format(rule) => OperationError::Structural(rule.to_owned()),
            Self::EncryptedOpaque => OperationError::Unsupported,
            Self::Decrypt => OperationError::Structural("tde.page.decrypt".to_owned()),
        }
    }
}

impl OwnedInspectionPage {
    fn read(data: &SessionData, vpid: Vpid) -> Result<Self, InspectionPageError> {
        let source = data
            .sources
            .volume(vpid.vol_id)
            .ok_or(InspectionPageError::Format("page.volume.exists"))?;
        let physical = source
            .read_page(vpid.page_id)
            .map_err(InspectionPageError::Source)?;
        let algorithm = decode_page_envelope(physical.as_slice(), vpid)
            .map_err(|error| InspectionPageError::Format(error.rule()))?
            .tde_algorithm();
        let decrypted_user = match algorithm {
            Some(algorithm) => {
                let key = data
                    .tde_key
                    .as_deref()
                    .ok_or(InspectionPageError::EncryptedOpaque)?;
                Some(
                    decrypt_page_user_region(physical.as_slice(), algorithm, key)
                        .map_err(|_| InspectionPageError::Decrypt)?,
                )
            }
            None => None,
        };
        Ok(Self {
            physical,
            decrypted_user,
        })
    }

    fn envelope(&self, vpid: Vpid) -> Result<crate::format::DecodedPageEnvelope<'_>, DecodeError> {
        if let Some(plaintext) = self.decrypted_user.as_deref() {
            decode_decrypted_page_envelope(self.physical.as_slice(), plaintext, vpid)
        } else {
            decode_page_envelope(self.physical.as_slice(), vpid)
        }
    }
}

fn read_special_heap_record<const N: usize>(
    sources: &SourceSet,
    hfid: Hfid,
    policy: ResourcePolicy,
    cancel: &CancelToken,
) -> Result<[u8; N], OpenFailure> {
    if policy.memory_limit < crate::format::IO_PAGE_SIZE as u64
        || policy.max_decoded_bytes < crate::format::IO_PAGE_SIZE as u64
    {
        return Err(OpenFailure::TdeBootstrap);
    }
    let header = Vpid::new(hfid.vfid.vol_id, hfid.header_page_id);
    let mut current = header;
    let mut previous = None;
    let mut visited = BTreeSet::new();
    let mut steps = 0_u64;
    let mut decoded_bytes = 0_u64;
    loop {
        if cancel.is_cancelled() {
            return Err(OpenFailure::Interrupted);
        }
        steps = steps.checked_add(1).ok_or(OpenFailure::Arithmetic)?;
        decoded_bytes = decoded_bytes
            .checked_add(crate::format::IO_PAGE_SIZE as u64)
            .ok_or(OpenFailure::Arithmetic)?;
        if steps > policy.max_chain_steps || decoded_bytes > policy.max_decoded_bytes {
            return Err(OpenFailure::TdeBootstrap);
        }
        if !visited.insert(current) {
            return Err(OpenFailure::TdeBootstrap);
        }
        let source = sources
            .volume(current.vol_id)
            .ok_or(OpenFailure::TdeBootstrap)?;
        let bytes = source.read_page(current.page_id)?;
        let envelope =
            decode_page_envelope(bytes.as_slice(), current).map_err(OpenFailure::Format)?;
        let slotted = decode_slotted_page(&envelope).map_err(OpenFailure::Format)?;
        let page = decode_heap_page(&envelope, &slotted, current == header)
            .map_err(OpenFailure::Format)?;
        let next = match page {
            HeapPageFact::Header(fact)
                if current == header && fact.class_oid.is_none() && previous.is_none() =>
            {
                fact.next
            }
            HeapPageFact::Chain(fact)
                if current != header && fact.class_oid.is_none() && fact.previous == previous =>
            {
                fact.next
            }
            _ => return Err(OpenFailure::TdeBootstrap),
        };
        if let Some(record) = crate::format::copy_special_heap_record::<N>(&envelope, &slotted)
            .map_err(OpenFailure::Format)?
        {
            return Ok(record);
        }
        previous = Some(current);
        current = next.ok_or(OpenFailure::TdeBootstrap)?;
    }
}

#[derive(Clone, Debug)]
pub struct Inspection {
    data: Arc<SessionData>,
}

#[derive(Clone, Debug)]
pub struct GraphView {
    data: Arc<SessionData>,
}

/// Classifies the same mid-scan source change under the two input contracts.
///
/// Keeping this decision free of I/O makes the immutable and live consequences
/// directly testable; racing a writer against a scan would only test timing.
fn classify_mid_scan_source_change(
    source_mode: SourceMode,
) -> (SnapshotValidity, DiagnosticRecord) {
    match source_mode {
        SourceMode::Immutable => (
            SnapshotValidity::Invalidated,
            DiagnosticRecord {
                code: "snapshot.modified",
                severity: "fatal",
                message: "An input changed during inspection.",
                subject: "snapshot".to_owned(),
                rule: "snapshot.file_stamp.stable",
            },
        ),
        SourceMode::Live => (
            SnapshotValidity::Torn,
            DiagnosticRecord {
                code: "snapshot.torn_read",
                severity: "warning",
                message: "An input changed while this generation was read; its facts may mix states.",
                subject: "snapshot".to_owned(),
                rule: "snapshot.file_stamp.stable",
            },
        ),
    }
}

impl Inspection {
    /// Reads the input under the offline immutable contract.
    pub fn open(
        request: &OpenRequest,
        policy: ResourcePolicy,
        cancel: &CancelToken,
        progress: Option<&mut dyn ProgressObserver>,
    ) -> Result<Self, OpenFailure> {
        Self::open_with_mode(request, SourceMode::Immutable, policy, cancel, progress)
    }

    /// Reads the input as one generation of a live follow. A change observed
    /// during the scan produces a torn generation rather than ending the
    /// session, because the caller re-reads instead of stopping.
    pub fn open_live(
        request: &OpenRequest,
        policy: ResourcePolicy,
        cancel: &CancelToken,
        progress: Option<&mut dyn ProgressObserver>,
    ) -> Result<Self, OpenFailure> {
        Self::open_with_mode(request, SourceMode::Live, policy, cancel, progress)
    }

    #[allow(clippy::too_many_lines, clippy::single_match_else)]
    fn open_with_mode(
        request: &OpenRequest,
        source_mode: SourceMode,
        policy: ResourcePolicy,
        cancel: &CancelToken,
        mut progress: Option<&mut dyn ProgressObserver>,
    ) -> Result<Self, OpenFailure> {
        report(&mut progress, ScanPhase::Discovery, 0, None);
        if cancel.is_cancelled() {
            return Err(OpenFailure::Interrupted);
        }
        let sources = Arc::new(discover(&request.input).map_err(OpenFailure::Source)?);
        report(
            &mut progress,
            ScanPhase::VolumeGeometry,
            0,
            Some(sources.volumes().len() as u64),
        );

        let mut volumes = Vec::with_capacity(sources.volumes().len());
        let mut hasher = Sha256::new();
        hasher.update(FORMAT_PROFILE.as_bytes());
        hasher.update(sources.input_kind().as_bytes());
        let mut physical_page_count = 0_u64;
        let mut primary_boot_hfid = None;

        for (index, source) in sources.volumes().iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(OpenFailure::Interrupted);
            }
            let page = source.read_page(PageId::new(0).map_err(|_| OpenFailure::Arithmetic)?)?;
            let envelope = decode_page_envelope(
                page.as_slice(),
                source.vpid(PageId::new(0).map_err(|_| OpenFailure::Arithmetic)?),
            )
            .map_err(OpenFailure::Format)?;
            let header = decode_volume_header(&envelope, source.stamp().length)
                .map_err(OpenFailure::Format)?;
            if header.vol_id().get() == 0 {
                primary_boot_hfid = header.boot_hfid();
            }
            let total_pages = u64::from(header.total_sectors())
                .checked_mul(u64::from(SECTOR_PAGES))
                .ok_or(OpenFailure::Arithmetic)?;
            physical_page_count = physical_page_count
                .checked_add(total_pages)
                .ok_or(OpenFailure::Arithmetic)?;
            hasher.update(header.vol_id().get().to_le_bytes());
            hasher.update(header.total_sectors().to_le_bytes());
            hasher.update(header.maximum_sectors().to_le_bytes());
            hasher.update(header.volume_creation().to_le_bytes());
            let stamp = source.stamp();
            hasher.update(stamp.device.to_le_bytes());
            hasher.update(stamp.inode.to_le_bytes());
            hasher.update(stamp.length.to_le_bytes());
            hasher.update(stamp.modified_seconds.to_le_bytes());
            hasher.update(stamp.modified_nanoseconds.to_le_bytes());

            let mask_words = header.total_sectors().div_ceil(64);
            let mask_len = usize::try_from(mask_words).map_err(|_| OpenFailure::Arithmetic)?;
            let mut reserved_masks = vec![0_u64; mask_len];
            for bitmap_index in 0..header.bitmap_page_count() {
                let bitmap_page_id = bitmap_index
                    .checked_add(1)
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or(OpenFailure::Arithmetic)?;
                let page_id = PageId::new(bitmap_page_id).map_err(|_| OpenFailure::Arithmetic)?;
                let bitmap_page = source.read_page(page_id)?;
                let bitmap_envelope =
                    decode_page_envelope(bitmap_page.as_slice(), source.vpid(page_id))
                        .map_err(OpenFailure::Format)?;
                let bitmap = decode_sector_bitmap(&bitmap_envelope, &header, bitmap_index)
                    .map_err(OpenFailure::Format)?;
                for relative in 0..bitmap.sector_count() {
                    let sector_value = u32::try_from(bitmap.first_sector().get())
                        .ok()
                        .and_then(|first| first.checked_add(relative))
                        .ok_or(OpenFailure::Arithmetic)?;
                    let sector = SectorId::new(
                        i32::try_from(sector_value).map_err(|_| OpenFailure::Arithmetic)?,
                    )
                    .map_err(|_| OpenFailure::Arithmetic)?;
                    if bitmap.is_reserved(sector).map_err(OpenFailure::Format)? {
                        let word = usize::try_from(sector_value / 64)
                            .map_err(|_| OpenFailure::Arithmetic)?;
                        if let Some(mask) = reserved_masks.get_mut(word) {
                            *mask |= 1_u64 << (sector_value % 64);
                        } else {
                            return Err(OpenFailure::Arithmetic);
                        }
                    }
                }
            }
            let reserved_sectors = reserved_masks.iter().map(|word| word.count_ones()).sum();
            let view = VolumeView {
                vol_id: header.vol_id(),
                purpose: header.purpose(),
                volume_type: header.volume_type(),
                total_sectors: header.total_sectors(),
                maximum_sectors: header.maximum_sectors(),
                system_last_page: header.system_last_page(),
                reserved_sectors,
            };
            volumes.push(VolumeRecord {
                view,
                reserved_masks,
                pages: PageFactStore::memory(),
            });
            report(
                &mut progress,
                ScanPhase::VolumeGeometry,
                (index + 1) as u64,
                Some(sources.volumes().len() as u64),
            );
        }

        report(
            &mut progress,
            ScanPhase::SectorReservation,
            volumes
                .iter()
                .map(|volume| u64::from(volume.view.total_sectors))
                .sum(),
            Some(
                volumes
                    .iter()
                    .map(|volume| u64::from(volume.view.total_sectors))
                    .sum(),
            ),
        );

        let (tde_key, insecure_tde_key_permissions) = if let Some(key_path) =
            request.tde_keys_file.as_deref()
        {
            let boot_hfid = primary_boot_hfid.ok_or(OpenFailure::TdeBootstrap)?;
            let boot_record =
                read_special_heap_record::<BOOT_DB_PARM_SIZE>(&sources, boot_hfid, policy, cancel)?;
            let boot = decode_boot_db_parm(&boot_record, boot_hfid).map_err(OpenFailure::Format)?;
            let key_info_record = read_special_heap_record::<TDE_KEY_INFO_RECORD_SIZE>(
                &sources,
                boot.tde_keyinfo_hfid,
                policy,
                cancel,
            )?;
            let key_info = decode_key_info_record(&key_info_record).map_err(OpenFailure::Tde)?;
            let loaded = load_permanent_key(key_path, &key_info).map_err(OpenFailure::Tde)?;
            (Some(Arc::new(loaded.key)), loaded.insecure_permissions)
        } else {
            (None, false)
        };

        let mut diagnostics = Vec::new();
        if insecure_tde_key_permissions {
            diagnostics.push(DiagnosticRecord {
                code: "tde.key_file.insecure_permissions",
                severity: "warning",
                message: "The supplied key file permissions are insecure.",
                subject: "tde-key-file".to_owned(),
                rule: "tde.key_file.owner_only_permissions",
            });
        }
        let envelope_counts = volumes
            .iter()
            .map(eligible_page_count)
            .collect::<Result<Vec<_>, _>>()?;
        let envelope_total = envelope_counts.iter().try_fold(0_u64, |total, count| {
            total.checked_add(*count).ok_or(OpenFailure::Arithmetic)
        })?;
        report(
            &mut progress,
            ScanPhase::PageEnvelopes,
            0,
            Some(envelope_total),
        );

        let mut evaluated = 0_u64;
        let mut conclusive = 0_u64;
        let mut stopped_reason = None;
        let base_memory = estimate_base_bytes(&volumes)?;
        let retained_diagnostics = diagnostics.iter().try_fold(0_u64, |total, diagnostic| {
            total
                .checked_add(retained_diagnostic_bytes(diagnostic)?)
                .ok_or(OpenFailure::Arithmetic)
        })?;
        let admitted_base = base_memory
            .checked_add(retained_diagnostics)
            .and_then(|value| value.checked_add(TERMINAL_DIAGNOSTIC_RESERVE))
            .ok_or(OpenFailure::Arithmetic)?;
        let fact_bytes = envelope_total
            .checked_mul(PACKED_PAGE_FACT_SIZE)
            .ok_or(OpenFailure::Arithmetic)?;
        let use_spill = admitted_base
            .checked_add(fact_bytes)
            .is_none_or(|required| required > policy.memory_limit);
        let spill = if use_spill && admitted_base <= policy.memory_limit {
            Some(Arc::new(
                SpillFile::create(request.spill_directory.as_deref())
                    .map_err(|_| OpenFailure::Spill)?,
            ))
        } else {
            None
        };
        if spill.is_none() && admitted_base <= policy.memory_limit {
            for (volume, count) in volumes.iter_mut().zip(&envelope_counts) {
                volume.pages = PageFactStore::memory_with_capacity(*count)?;
            }
        }
        let mut spill_used = 0_u64;
        let mut resident_used = if spill.is_some() {
            admitted_base
        } else {
            admitted_base
                .checked_add(fact_bytes)
                .ok_or(OpenFailure::Arithmetic)?
        };
        let mut fast_summary = PageFactSummary::default();
        let has_tde_key = tde_key.is_some();
        let requested_workers =
            usize::try_from(policy.workers).map_err(|_| OpenFailure::Arithmetic)?;
        let available_workers = thread::available_parallelism().map_or(1, usize::from);
        let worker_count = requested_workers.min(available_workers).max(1);
        let wave_capacity = worker_count
            .checked_mul(WORKER_PAGE_BATCH)
            .ok_or(OpenFailure::Arithmetic)?;
        let wave_capacity_u64 =
            u64::try_from(wave_capacity).map_err(|_| OpenFailure::Arithmetic)?;
        let mut envelope_read_attempts = 0_u64;
        let mut slotted_header_read_attempts = 0_u64;
        'volume_scan: for (volume_index, source) in sources.volumes().iter().enumerate() {
            let Some(volume) = volumes.get_mut(volume_index) else {
                return Err(OpenFailure::Arithmetic);
            };
            if let Some(file) = spill.as_ref() {
                volume.pages = PageFactStore::spilled(Arc::clone(file), spill_used);
            }
            let mut cursor = EligiblePageCursor::default();
            while let Some(first_page) = cursor.next(volume)? {
                let spill_slots = if spill.is_some() {
                    policy
                        .spill_limit
                        .saturating_sub(spill_used)
                        .checked_div(PACKED_PAGE_FACT_SIZE)
                        .ok_or(OpenFailure::Arithmetic)?
                } else {
                    u64::MAX
                };
                if admitted_base > policy.memory_limit || spill_slots == 0 {
                    stopped_reason = Some("resource-limit");
                    diagnostics.push(resource_limit_diagnostic(
                        volume.view.vol_id,
                        first_page,
                        if admitted_base > policy.memory_limit {
                            "inspection.resource_policy.resident"
                        } else {
                            "inspection.resource_policy.spill"
                        },
                    ));
                    break 'volume_scan;
                }
                let admitted = usize::try_from(spill_slots.min(wave_capacity_u64))
                    .map_err(|_| OpenFailure::Arithmetic)?;
                let mut page_ids = Vec::with_capacity(admitted);
                page_ids.push(first_page);
                while page_ids.len() < admitted {
                    let Some(page_id) = cursor.next(volume)? else {
                        break;
                    };
                    page_ids.push(page_id);
                }
                envelope_read_attempts = envelope_read_attempts
                    .checked_add(
                        u64::try_from(page_ids.len()).map_err(|_| OpenFailure::Arithmetic)?,
                    )
                    .ok_or(OpenFailure::Arithmetic)?;
                let scanned = thread::scope(|scope| {
                    let handles = page_ids
                        .chunks(WORKER_PAGE_BATCH)
                        .map(|chunk| {
                            scope.spawn(move || {
                                chunk
                                    .iter()
                                    .copied()
                                    .map(|page_id| {
                                        scan_page_envelope(source, page_id, has_tde_key, cancel)
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .collect::<Vec<_>>();
                    handles
                        .into_iter()
                        .map(|handle| handle.join().map_err(|_| OpenFailure::Worker))
                        .collect::<Result<Vec<_>, _>>()
                })?;
                for result in scanned.into_iter().flatten() {
                    if cancel.is_cancelled() || matches!(result, EnvelopeScanResult::Cancelled) {
                        stopped_reason = Some("interrupted");
                        break 'volume_scan;
                    }
                    let EnvelopeScanResult::Fact { fact, diagnostic } = result else {
                        let EnvelopeScanResult::Unreadable(unreadable_page) = result else {
                            unreachable!("cancelled scan results stop before interpretation")
                        };
                        let finding = DiagnosticRecord {
                            code: "input.volume_unreadable",
                            severity: "error",
                            message: "A volume could not be read completely.",
                            subject: format!("volume:{}", volume.view.vol_id.get()),
                            rule: "source.positional_read.complete",
                        };
                        if admit_diagnostic(&mut resident_used, policy.memory_limit, &finding)? {
                            evaluated = evaluated.checked_add(1).ok_or(OpenFailure::Arithmetic)?;
                            diagnostics.push(finding);
                            stopped_reason = Some("unreadable");
                        } else {
                            diagnostics.push(resource_limit_diagnostic(
                                volume.view.vol_id,
                                unreadable_page,
                                "inspection.resource_policy.diagnostics",
                            ));
                            stopped_reason = Some("resource-limit");
                        }
                        break 'volume_scan;
                    };
                    if fact.tde_state == TdeInspectionState::NotEncrypted
                        && fact
                            .page_type
                            .is_some_and(|page_type| page_uses_slotted_layout(page_type, None))
                    {
                        slotted_header_read_attempts = slotted_header_read_attempts
                            .checked_add(1)
                            .ok_or(OpenFailure::Arithmetic)?;
                    }
                    if let Some((code, rule)) = diagnostic {
                        let finding = DiagnosticRecord {
                            code,
                            severity: "error",
                            message: "The page envelope violates the pinned format.",
                            subject: format!(
                                "page:{}:{}",
                                volume.view.vol_id.get(),
                                fact.page_id.get()
                            ),
                            rule,
                        };
                        if !admit_diagnostic(&mut resident_used, policy.memory_limit, &finding)? {
                            diagnostics.push(resource_limit_diagnostic(
                                volume.view.vol_id,
                                fact.page_id,
                                "inspection.resource_policy.diagnostics",
                            ));
                            stopped_reason = Some("resource-limit");
                            break 'volume_scan;
                        }
                        diagnostics.push(finding);
                    }
                    evaluated = evaluated.checked_add(1).ok_or(OpenFailure::Arithmetic)?;
                    conclusive = conclusive.checked_add(1).ok_or(OpenFailure::Arithmetic)?;
                    if let (Some(page_type), Some(lsa_word)) = (fact.page_type, fact.lsa_word) {
                        hasher.update(volume.view.vol_id.get().to_le_bytes());
                        hasher.update(fact.page_id.get().to_le_bytes());
                        hasher.update([page_type.ordinal()]);
                        hasher.update(lsa_word.to_le_bytes());
                    }
                    volume
                        .pages
                        .push(fact)
                        .map_err(|_| OpenFailure::FactStore)?;
                    fast_summary.observe(fact)?;
                    if spill.is_some() {
                        spill_used = spill_used
                            .checked_add(PACKED_PAGE_FACT_SIZE)
                            .ok_or(OpenFailure::Arithmetic)?;
                    }
                    report(
                        &mut progress,
                        ScanPhase::PageEnvelopes,
                        evaluated,
                        Some(envelope_total),
                    );
                }
            }
        }

        report(&mut progress, ScanPhase::Reconciliation, 0, Some(1));
        let mut validity = SnapshotValidity::Valid;
        if !sources.verify_unchanged().map_err(OpenFailure::Source)? {
            // The same observation means different things under the two
            // contracts: offline it ends the session, following it only means
            // this generation is internally inconsistent and another is due.
            let (changed_validity, diagnostic) = classify_mid_scan_source_change(source_mode);
            validity = changed_validity;
            diagnostics.push(diagnostic);
        }
        let page_coverage = if stopped_reason.is_some() {
            Coverage::Partial
        } else {
            Coverage::Complete
        };
        let coverage = vec![
            CoverageRecord {
                facet: "volume-topology",
                coverage: Coverage::Complete,
                evaluated: volumes.len() as u64,
                conclusive: volumes.len() as u64,
                trusted_total: Some(volumes.len() as u64),
                stop_reason: None,
            },
            CoverageRecord {
                facet: "sector-reservation",
                coverage: Coverage::Complete,
                evaluated: volumes
                    .iter()
                    .map(|volume| u64::from(volume.view.total_sectors))
                    .sum(),
                conclusive: volumes
                    .iter()
                    .map(|volume| u64::from(volume.view.total_sectors))
                    .sum(),
                trusted_total: Some(
                    volumes
                        .iter()
                        .map(|volume| u64::from(volume.view.total_sectors))
                        .sum(),
                ),
                stop_reason: None,
            },
            CoverageRecord {
                facet: "page-envelopes",
                coverage: page_coverage,
                evaluated,
                conclusive,
                trusted_total: Some(envelope_total),
                stop_reason: stopped_reason,
            },
        ];
        let outcome = InspectionOutcome::classify(OutcomeInputs {
            fatal: validity == SnapshotValidity::Invalidated,
            unexpected_incomplete: page_coverage == Coverage::Partial,
            has_error_findings: diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == "error"),
            expected_limitations: true,
        });
        let digest = hasher.finalize();
        let mut snapshot_bytes = [0_u8; 16];
        snapshot_bytes.copy_from_slice(&digest[..16]);
        let fast_scan_resources = FastScanResources {
            memory_limit: policy.memory_limit,
            spill_limit: policy.spill_limit,
            requested_workers: policy.workers,
            admitted_workers: u32::try_from(worker_count).map_err(|_| OpenFailure::Arithmetic)?,
            max_chain_steps: policy.max_chain_steps,
            max_decoded_bytes: policy.max_decoded_bytes,
            admitted_resident_bytes: resident_used,
            spill_bytes: spill_used,
            packed_fact_bytes: fast_summary
                .inspected
                .checked_mul(PACKED_PAGE_FACT_SIZE)
                .ok_or(OpenFailure::Arithmetic)?,
            envelope_read_attempts,
            envelope_requested_bytes: envelope_read_attempts
                .checked_mul(40)
                .ok_or(OpenFailure::Arithmetic)?,
            slotted_header_read_attempts,
            slotted_header_requested_bytes: slotted_header_read_attempts
                .checked_mul(
                    u64::try_from(SLOTTED_HEADER_SIZE).map_err(|_| OpenFailure::Arithmetic)?,
                )
                .ok_or(OpenFailure::Arithmetic)?,
        };
        report(&mut progress, ScanPhase::Reconciliation, 1, Some(1));
        let inspection = Self {
            data: Arc::new(SessionData {
                sources,
                source_mode,
                tde_key,
                snapshot_id: SnapshotId::from_bytes(snapshot_bytes),
                revision: InspectionRevision::new(0),
                validity,
                outcome,
                volumes,
                coverage,
                diagnostics,
                fast_summary,
                fast_scan_resources,
                page_overrides: BTreeMap::new(),
                deep_pages: BTreeMap::new(),
                file_allocations: BTreeMap::new(),
                tracked_files: BTreeMap::new(),
                sector_claims: BTreeMap::new(),
                class_names: BTreeMap::new(),
                oos_chains: BTreeMap::new(),
                overflow_chains: BTreeMap::new(),
                relocation_edges: BTreeMap::new(),
                class_representations: BTreeMap::new(),
                sector_classes: BTreeMap::new(),
                record_interpretations: BTreeMap::new(),
                interpretation_failures: BTreeMap::new(),
            }),
        };
        inspection.bootstrap_file_inventory(policy, cancel)
    }

    #[allow(clippy::too_many_lines)]
    fn bootstrap_file_inventory(
        self,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Self, OpenFailure> {
        let page_coverage = self
            .data
            .coverage
            .iter()
            .find(|coverage| coverage.facet == "page-envelopes")
            .copied()
            .ok_or(OpenFailure::FactStore)?;
        if page_coverage.coverage != Coverage::Complete {
            return Ok(self.with_initial_inventory_result(
                Coverage::Partial,
                0,
                None,
                page_coverage.stop_reason,
                None,
            ));
        }

        let mut has_file_table = false;
        for volume in &self.data.volumes {
            for index in 0..volume.pages.len() {
                let fact = volume
                    .pages
                    .fact_at(index)
                    .map_err(|_| OpenFailure::FactStore)?
                    .ok_or(OpenFailure::FactStore)?;
                has_file_table |= fact.page_type == Some(PageType::FileTable);
            }
        }
        if !has_file_table {
            return Ok(self.with_initial_inventory_result(
                Coverage::Complete,
                0,
                Some(0),
                None,
                None,
            ));
        }

        let base = GraphView {
            data: Arc::clone(&self.data),
        };
        match base.enrich_file_inventory(policy, cancel) {
            Ok(enriched) => {
                let mut data = (*enriched.data).clone();
                data.revision = InspectionRevision::new(0);
                Ok(Self {
                    data: Arc::new(data),
                })
            }
            Err(OperationError::FactStore) => Err(OpenFailure::FactStore),
            Err(OperationError::Arithmetic) => Err(OpenFailure::Arithmetic),
            Err(OperationError::Interrupted) => Ok(self.with_initial_inventory_result(
                Coverage::Partial,
                0,
                None,
                Some("interrupted"),
                Some(DiagnosticRecord {
                    code: "inspection.interrupted",
                    severity: "warning",
                    message: "File inventory stopped after cancellation.",
                    subject: "snapshot".to_owned(),
                    rule: "inspection.cancellation.boundary",
                }),
            )),
            Err(OperationError::ResourceLimit) => Ok(self.with_initial_inventory_result(
                Coverage::Partial,
                0,
                None,
                Some("resource-limit"),
                Some(DiagnosticRecord {
                    code: "inspection.resource_limit",
                    severity: "warning",
                    message: "File inventory exceeded the configured resource policy.",
                    subject: "snapshot".to_owned(),
                    rule: "inspection.resource_policy.file_inventory",
                }),
            )),
            Err(OperationError::Unsupported) => Ok(self.with_initial_inventory_result(
                Coverage::Partial,
                0,
                None,
                Some("unsupported"),
                Some(DiagnosticRecord {
                    code: "file.inventory.unavailable",
                    severity: "warning",
                    message: "Authoritative file metadata is unavailable for structural inspection.",
                    subject: "snapshot".to_owned(),
                    rule: "file.tracker.required_metadata",
                }),
            )),
            Err(OperationError::Source(_)) => Ok(self.with_initial_inventory_result(
                Coverage::Partial,
                0,
                None,
                Some("unreadable"),
                Some(DiagnosticRecord {
                    code: "input.volume_unreadable",
                    severity: "error",
                    message: "A volume could not be read completely during file inventory.",
                    subject: "snapshot".to_owned(),
                    rule: "source.positional_read.complete",
                }),
            )),
            Err(OperationError::Structural(_)) => Ok(self.with_initial_inventory_result(
                Coverage::Partial,
                0,
                None,
                Some("structural"),
                Some(DiagnosticRecord {
                    code: "file.inventory.invalid",
                    severity: "error",
                    message: "Authoritative file metadata violates the pinned format.",
                    subject: "snapshot".to_owned(),
                    rule: "file.inventory.structural_validation",
                }),
            )),
            Err(OperationError::RevisionNotFound | OperationError::Query(_)) => {
                Err(OpenFailure::FactStore)
            }
        }
    }

    fn with_initial_inventory_result(
        &self,
        coverage: Coverage,
        evaluated: u64,
        trusted_total: Option<u64>,
        stop_reason: Option<&'static str>,
        diagnostic: Option<DiagnosticRecord>,
    ) -> Self {
        let mut data = (*self.data).clone();
        data.coverage
            .retain(|record| record.facet != "file-inventory");
        data.coverage.push(CoverageRecord {
            facet: "file-inventory",
            coverage,
            evaluated,
            conclusive: evaluated,
            trusted_total,
            stop_reason,
        });
        if let Some(diagnostic) = diagnostic {
            data.diagnostics.push(diagnostic);
        }
        data.outcome = classify_session_outcome(&data);
        Self {
            data: Arc::new(data),
        }
    }

    pub fn view(&self, selector: RevisionSelector) -> Result<GraphView, OperationError> {
        let requested = match selector {
            RevisionSelector::Latest => self.data.revision,
            RevisionSelector::Exact(revision) => revision,
        };
        if requested != self.data.revision {
            return Err(OperationError::RevisionNotFound);
        }
        Ok(GraphView {
            data: Arc::clone(&self.data),
        })
    }

    pub fn verify_snapshot(&self) -> Result<bool, OperationError> {
        self.data
            .sources
            .verify_unchanged()
            .map_err(OperationError::Source)
    }
}

impl GraphView {
    /// The contract this reading was taken under.
    #[must_use]
    pub fn source_mode(&self) -> SourceMode {
        self.data.source_mode
    }

    /// The input fingerprint manifest observed when this reading was
    /// discovered. Comparing it with a fresh `source::fingerprint` of the same
    /// input is how a follower decides the reading has been superseded.
    #[must_use]
    pub fn source_fingerprint(&self) -> crate::source::InputFingerprint {
        self.data.sources.fingerprint()
    }

    #[must_use]
    pub fn fast_scan_resources(&self) -> FastScanResources {
        self.data.fast_scan_resources
    }

    #[must_use]
    pub fn overview(&self) -> OverviewView {
        let sector_count = self
            .data
            .volumes
            .iter()
            .map(|volume| u64::from(volume.view.total_sectors))
            .sum();
        let reserved_sector_count = self
            .data
            .volumes
            .iter()
            .map(|volume| u64::from(volume.view.reserved_sectors))
            .sum();
        let physical_page_count = sector_count * u64::from(SECTOR_PAGES);
        let page_type_counts = self
            .data
            .fast_summary
            .page_type_counts
            .into_iter()
            .enumerate()
            .filter(|(_, count)| *count != 0)
            .filter_map(|(ordinal, count)| {
                u8::try_from(ordinal)
                    .ok()
                    .and_then(page_type_from_ordinal)
                    .map(|kind| (kind, count))
            })
            .collect();
        OverviewView {
            snapshot_id: self.data.snapshot_id,
            revision: self.data.revision,
            validity: self.data.validity,
            format_profile: FORMAT_PROFILE,
            input_kind: self.data.sources.input_kind(),
            outcome: self.data.outcome,
            volume_count: self.data.volumes.len() as u64,
            sector_count,
            reserved_sector_count,
            physical_page_count,
            inspected_page_envelopes: self.data.fast_summary.inspected,
            page_type_counts,
            tde_opaque_pages: self.data.fast_summary.tde_opaque_pages,
            coverage: self.data.coverage.clone(),
            diagnostics: self.data.diagnostics.clone(),
        }
    }

    #[must_use]
    pub fn volumes(&self) -> Vec<VolumeView> {
        self.data.volumes.iter().map(|volume| volume.view).collect()
    }

    pub fn volume(&self, vol_id: VolId) -> Result<VolumeView, QueryError> {
        self.volume_record(vol_id).map(|record| record.view)
    }

    pub fn sector(&self, vol_id: VolId, sector_id: SectorId) -> Result<SectorView, QueryError> {
        let volume = self.volume_record(vol_id)?;
        let sector = u32::try_from(sector_id.get()).map_err(|_| QueryError::EntityNotFound)?;
        if sector >= volume.view.total_sectors {
            return Err(QueryError::EntityNotFound);
        }
        let first_page = sector
            .checked_mul(SECTOR_PAGES)
            .ok_or(QueryError::Arithmetic)?;
        let pages = (0..SECTOR_PAGES)
            .map(|within| {
                let raw_page = first_page
                    .checked_add(within)
                    .ok_or(QueryError::Arithmetic)?;
                let page_id =
                    PageId::new(i32::try_from(raw_page).map_err(|_| QueryError::Arithmetic)?)
                        .map_err(|_| QueryError::Arithmetic)?;
                self.page_from_record(volume, page_id)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SectorView {
            vol_id,
            sector_id,
            reserved: volume.is_reserved(sector),
            attribution: self.sector_attribution(vol_id, sector_id),
            pages,
        })
    }

    pub fn page(&self, vpid: Vpid) -> Result<PageView, QueryError> {
        let volume = self.volume_record(vpid.vol_id)?;
        let raw_page = u32::try_from(vpid.page_id.get()).map_err(|_| QueryError::EntityNotFound)?;
        let total_pages = volume
            .view
            .total_sectors
            .checked_mul(SECTOR_PAGES)
            .ok_or(QueryError::Arithmetic)?;
        if raw_page >= total_pages {
            return Err(QueryError::EntityNotFound);
        }
        self.page_from_record(volume, vpid.page_id)
    }

    /// Join one file's inventory facts into a shared association view.
    /// Names are never copied per page; they resolve once per class OID.
    fn file_association(&self, vfid: Vfid) -> FileAssociation {
        let header = self.data.tracked_files.get(&vfid).copied().or_else(|| {
            PageId::new(vfid.file_id.get()).ok().and_then(|page_id| {
                self.data
                    .deep_pages
                    .get(&Vpid::new(vfid.vol_id, page_id))
                    .and_then(|fact| fact.file_header)
            })
        });
        let Some(header) = header else {
            return FileAssociation {
                vfid,
                file_type: None,
                class: ClassAssociation::None("allocating file header is unavailable"),
            };
        };
        let class = header.class_oid().map_or_else(
            || ClassAssociation::None(class_absence_reason(header.file_type())),
            |oid| ClassAssociation::Class {
                oid,
                name: self.data.class_names.get(&oid).cloned().unwrap_or(
                    ClassNameResolution::Unresolved("class name was not resolved"),
                ),
            },
        );
        FileAssociation {
            vfid,
            file_type: Some(header.file_type()),
            class,
        }
    }

    fn page_file_association(&self, vpid: Vpid, sector_id: SectorId) -> PageFileAssociation {
        if let Some(owner) = self.data.file_allocations.get(&vpid) {
            return PageFileAssociation::Allocated(self.file_association(*owner));
        }
        match self
            .data
            .sector_claims
            .get(&(vpid.vol_id, sector_id))
            .map(Vec::as_slice)
        {
            None | Some([]) => PageFileAssociation::None,
            Some([claim]) => PageFileAssociation::ReservedFor(self.file_association(claim.vfid)),
            Some(_) => PageFileAssociation::MixedClaims,
        }
    }

    fn sector_attribution(&self, vol_id: VolId, sector_id: SectorId) -> SectorAttribution {
        match self
            .data
            .sector_claims
            .get(&(vol_id, sector_id))
            .map(Vec::as_slice)
        {
            None | Some([]) => SectorAttribution::Unclaimed,
            Some([claim]) => SectorAttribution::Single {
                association: self.file_association(claim.vfid),
                kind: claim.kind,
            },
            Some(claims) => SectorAttribution::Mixed {
                claims: claims.to_vec(),
            },
        }
    }

    pub fn file_pages(&self, vfid: Vfid) -> Result<Vec<PageView>, QueryError> {
        let pages = self
            .data
            .file_allocations
            .iter()
            .filter_map(|(vpid, owner)| (*owner == vfid).then_some(*vpid))
            .map(|vpid| self.page(vpid))
            .collect::<Result<Vec<_>, _>>()?;
        if pages.is_empty() {
            return Err(QueryError::EntityNotFound);
        }
        Ok(pages)
    }

    /// Resolve every distinct descriptor class OID among `headers` exactly
    /// once. Failures stay typed and per-OID; they never abort the inventory.
    fn resolve_tracked_class_names(
        &self,
        headers: &BTreeMap<Vfid, FileHeader>,
        cancel: &CancelToken,
    ) -> Result<BTreeMap<Oid, ClassNameResolution>, OperationError> {
        let charset = database_charset(&self.data);
        let mut class_names = BTreeMap::new();
        for header in headers.values() {
            let Some(oid) = header.class_oid() else {
                continue;
            };
            if class_names.contains_key(&oid) {
                continue;
            }
            if cancel.is_cancelled() {
                return Err(OperationError::Interrupted);
            }
            let resolution = match charset {
                Ok(charset) => match self.resolve_class_name(oid, charset) {
                    Ok(name) => ClassNameResolution::Resolved(name),
                    Err(reason) => ClassNameResolution::Unresolved(reason),
                },
                Err(reason) => ClassNameResolution::Unresolved(reason),
            };
            class_names.insert(oid, resolution);
        }
        Ok(class_names)
    }

    /// Read the class record at `class_oid` and decode its stored name.
    /// Bounded and fail-closed: relocations are followed a fixed number of
    /// steps; multipage (`REC_BIGONE`) class records and compressed names
    /// stay typed-unresolved rather than guessed.
    fn resolve_class_name(&self, class_oid: Oid, charset: u8) -> Result<Arc<str>, &'static str> {
        let mut current = class_oid;
        let mut visited = BTreeSet::new();
        for _ in 0..8 {
            if !visited.insert(current) {
                return Err("class record relocation cycle");
            }
            let vpid = Vpid::new(current.vol_id, current.page_id);
            let owned = OwnedInspectionPage::read(&self.data, vpid)
                .map_err(|_| "class record page could not be read")?;
            let envelope = owned
                .envelope(vpid)
                .map_err(|_| "class record page envelope is invalid")?;
            let slotted = decode_slotted_page(&envelope)
                .map_err(|_| "class record page is not a valid heap page")?;
            let slot_id = u16::try_from(current.slot_id.get())
                .map_err(|_| "class record slot identifier is invalid")?;
            let slot = slotted
                .slots()
                .get(usize::from(slot_id))
                .ok_or("class record slot does not exist")?;
            match slot.record_type() {
                RecordType::Home | RecordType::NewHome => {
                    return decode_class_name(&envelope, &slotted, slot_id, charset);
                }
                RecordType::Relocation => {
                    current = decode_relocation_target(&envelope, &slotted, slot_id)
                        .map_err(|_| "class record relocation target is invalid")?;
                }
                RecordType::BigOne => {
                    return Err("multipage class records are not yet decoded");
                }
                RecordType::Unknown
                | RecordType::AssignAddress
                | RecordType::MarkDeleted
                | RecordType::DeletedWillReuse
                | RecordType::Reserved(_) => return Err("class record slot is not live"),
            }
        }
        Err("class record relocation limit reached")
    }

    /// Decode one page body into a new immutable revision. The prior view is
    /// retained unchanged and remains queryable by its caller.
    #[allow(clippy::too_many_lines)]
    pub fn enrich_page(
        &self,
        vpid: Vpid,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Self, OperationError> {
        if cancel.is_cancelled() {
            return Err(OperationError::Interrupted);
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        let page_view = self.page(vpid).map_err(OperationError::Query)?;
        if page_view.availability != Availability::Available {
            return Err(OperationError::Unsupported);
        }
        let required_bytes = crate::format::IO_PAGE_SIZE as u64
            + if page_view.tde_state == TdeInspectionState::Decrypted {
                crate::format::DB_PAGE_SIZE as u64
            } else {
                0
            };
        if policy.max_decoded_bytes < required_bytes || policy.memory_limit < required_bytes {
            return Err(OperationError::ResourceLimit);
        }
        let owned = match OwnedInspectionPage::read(&self.data, vpid) {
            Ok(page) => page,
            Err(InspectionPageError::Source(error)) => return Err(OperationError::Source(error)),
            Err(InspectionPageError::Format(rule)) => {
                return self.page_decode_failure(vpid, rule);
            }
            Err(InspectionPageError::EncryptedOpaque) => {
                return Err(OperationError::Unsupported);
            }
            Err(InspectionPageError::Decrypt) => {
                return self.page_decode_failure(vpid, "tde.page.decrypt");
            }
        };
        if cancel.is_cancelled() {
            return Err(OperationError::Interrupted);
        }
        let decoded = owned.envelope(vpid);
        let fact = match decoded {
            Ok(envelope) => {
                let owner_file_type = self
                    .data
                    .file_allocations
                    .get(&vpid)
                    .and_then(|owner| self.data.tracked_files.get(owner))
                    .map(|header| header.file_type());
                let slotted = if page_uses_slotted_layout(envelope.page_type(), owner_file_type) {
                    match decode_slotted_page(&envelope) {
                        Ok(value) => Some(value),
                        Err(error) => {
                            return self.page_decode_failure(vpid, error.rule());
                        }
                    }
                } else {
                    None
                };
                let raw = match envelope.page_type() {
                    PageType::Heap => {
                        let role = self
                            .data
                            .file_allocations
                            .get(&vpid)
                            .and_then(|owner| self.data.tracked_files.get(owner))
                            .and_then(|header| {
                                header
                                    .heap_header_page()
                                    .map(|heap_header| (heap_header == vpid, header.file_type()))
                            });
                        match (role, slotted.as_ref()) {
                            (
                                Some((
                                    is_header,
                                    crate::format::FileType::Heap
                                    | crate::format::FileType::HeapReuseSlots,
                                )),
                                Some(slotted),
                            ) => match decode_heap_page(&envelope, slotted, is_header) {
                                Ok(value) => Some(RawPageView::Heap(value)),
                                Err(error) => {
                                    return self.page_decode_failure(vpid, error.rule());
                                }
                            },
                            _ => None,
                        }
                    }
                    PageType::Btree => {
                        let role = self
                            .data
                            .file_allocations
                            .get(&vpid)
                            .and_then(|owner| self.data.tracked_files.get(owner))
                            .and_then(|header| {
                                (header.file_type() == crate::format::FileType::Btree)
                                    .then_some(header.sticky_first() == Some(vpid))
                            });
                        match (role, slotted.as_ref()) {
                            (Some(is_root), Some(slotted)) => {
                                match decode_btree_page(&envelope, slotted, is_root) {
                                    Ok(value) => Some(RawPageView::Btree(value)),
                                    Err(error) => {
                                        return self.page_decode_failure(vpid, error.rule());
                                    }
                                }
                            }
                            _ => None,
                        }
                    }
                    PageType::Catalog => {
                        let owned_by_catalog = self
                            .data
                            .file_allocations
                            .get(&vpid)
                            .and_then(|owner| self.data.tracked_files.get(owner))
                            .is_some_and(|header| {
                                header.file_type() == crate::format::FileType::Catalog
                            });
                        match (owned_by_catalog, slotted.as_ref()) {
                            (true, Some(slotted)) => {
                                match decode_catalog_page(&envelope, slotted) {
                                    Ok(page) => match self.catalog_directories(
                                        vpid, &envelope, slotted, page, policy, cancel,
                                    ) {
                                        Ok(directories) => {
                                            Some(RawPageView::Catalog(CatalogPageView {
                                                page,
                                                directories,
                                            }))
                                        }
                                        Err("resource-limit") => {
                                            return Err(OperationError::ResourceLimit);
                                        }
                                        Err("interrupted") => {
                                            return Err(OperationError::Interrupted);
                                        }
                                        Err(rule) => {
                                            return self.page_decode_failure(vpid, rule);
                                        }
                                    },
                                    Err(error) => {
                                        return self.page_decode_failure(vpid, error.rule());
                                    }
                                }
                            }
                            _ => None,
                        }
                    }
                    PageType::VacuumData => match decode_vacuum_page(&envelope) {
                        Ok(value) => Some(RawPageView::Vacuum(value)),
                        Err(error) => {
                            return self.page_decode_failure(vpid, error.rule());
                        }
                    },
                    PageType::DroppedFiles => match decode_dropped_files_page(&envelope) {
                        Ok(value) => Some(RawPageView::DroppedFiles(value)),
                        Err(error) => {
                            return self.page_decode_failure(vpid, error.rule());
                        }
                    },
                    _ => None,
                };
                DeepPageFact {
                    slotted,
                    // PAGE_FTAB is role-ambiguous without its owning VFID: only
                    // an explicit file selector may interpret one as a header.
                    file_header: None,
                    raw,
                    diagnostic_rule: None,
                }
            }
            Err(error) => return self.page_decode_failure(vpid, error.rule()),
        };
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        self.publish_deep_page(vpid, fact)
    }

    /// Decode the header page selected by a VFID. Generic `PAGE_FTAB` page
    /// enrichment intentionally remains envelope-only because continuation
    /// pages share the same page type and do not contain `FILE_HEADER`.
    pub fn enrich_file(
        &self,
        vfid: Vfid,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Self, OperationError> {
        let header_page = Vpid::new(
            vfid.vol_id,
            PageId::new(vfid.file_id.get()).map_err(|_| OperationError::Arithmetic)?,
        );
        if self
            .data
            .deep_pages
            .get(&header_page)
            .is_some_and(|fact| fact.file_header.is_some())
        {
            return Ok(self.clone());
        }
        if cancel.is_cancelled() {
            return Err(OperationError::Interrupted);
        }
        if policy.max_decoded_bytes < crate::format::IO_PAGE_SIZE as u64
            || policy.memory_limit < crate::format::IO_PAGE_SIZE as u64
        {
            return Err(OperationError::ResourceLimit);
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        let page = self.page(header_page).map_err(OperationError::Query)?;
        if page.availability != Availability::Available
            || page.page_type != Some(PageType::FileTable)
        {
            return Err(OperationError::Unsupported);
        }
        let owned = match OwnedInspectionPage::read(&self.data, header_page) {
            Ok(page) => page,
            Err(InspectionPageError::Source(error)) => return Err(OperationError::Source(error)),
            Err(InspectionPageError::Format(rule)) => {
                return self.page_decode_failure(header_page, rule);
            }
            Err(InspectionPageError::EncryptedOpaque | InspectionPageError::Decrypt) => {
                return self.page_decode_failure(header_page, "tde.file_header.invalid_state");
            }
        };
        let envelope = match owned.envelope(header_page) {
            Ok(value) => value,
            Err(error) => return self.page_decode_failure(header_page, error.rule()),
        };
        let file_header = match decode_file_header(&envelope) {
            Ok(value) if value.vfid() == vfid => value,
            Ok(_) => return self.page_decode_failure(header_page, "file.header.self_identity"),
            Err(error) => return self.page_decode_failure(header_page, error.rule()),
        };
        let facts = match self.collect_file_allocations(file_header, policy, cancel) {
            Ok(value) => value,
            Err(FileTraversalError::Decode(rule)) => {
                return self.page_decode_failure(header_page, rule);
            }
            Err(FileTraversalError::Operation(error)) => return Err(error),
        };
        if cancel.is_cancelled() {
            return Err(OperationError::Interrupted);
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        self.publish_file(header_page, file_header, facts, cancel)
    }

    /// Validate the permanent-file tracker and every referenced file header.
    /// Tracker items are the authority for assigning file roles; arbitrary
    /// `PAGE_FTAB` pages are never promoted by self-looking bytes alone.
    #[allow(clippy::too_many_lines)]
    pub fn enrich_file_inventory(
        &self,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Self, OperationError> {
        if !self.data.tracked_files.is_empty() {
            return Ok(self.clone());
        }
        let mut tracker = None;
        let mut decoded_bytes = 0_u64;
        for volume in &self.data.volumes {
            for index in 0..volume.pages.len() {
                let fact = volume
                    .pages
                    .fact_at(index)
                    .map_err(|_| OperationError::FactStore)?
                    .ok_or(OperationError::FactStore)?;
                if fact.page_type != Some(PageType::FileTable) {
                    continue;
                }
                if cancel.is_cancelled() {
                    return Err(OperationError::Interrupted);
                }
                let vpid = Vpid::new(volume.view.vol_id, fact.page_id);
                decoded_bytes = decoded_bytes
                    .checked_add(crate::format::IO_PAGE_SIZE as u64)
                    .ok_or(OperationError::Arithmetic)?;
                if decoded_bytes > policy.max_decoded_bytes {
                    return Err(OperationError::ResourceLimit);
                }
                let owned = match OwnedInspectionPage::read(&self.data, vpid) {
                    Ok(page) => page,
                    Err(InspectionPageError::Source(error)) => {
                        return Err(OperationError::Source(error));
                    }
                    Err(
                        InspectionPageError::Format(_)
                        | InspectionPageError::EncryptedOpaque
                        | InspectionPageError::Decrypt,
                    ) => continue,
                };
                let Ok(envelope) = owned.envelope(vpid) else {
                    continue;
                };
                let Ok(header) = decode_file_header(&envelope) else {
                    continue;
                };
                if header.file_type() == crate::format::FileType::Tracker
                    && tracker.replace(header).is_some()
                {
                    return self.page_decode_failure(vpid, "file.tracker.unique_header");
                }
            }
        }
        let tracker = tracker
            .ok_or_else(|| OperationError::Structural("file.tracker.header_missing".to_owned()))?;
        let tracker_page = tracker.sticky_first().ok_or_else(|| {
            OperationError::Structural("file.tracker.first_page_missing".to_owned())
        })?;
        let mut traversal = FileTraversal::default();
        traversal.table_pages.insert(tracker_page);
        self.walk_file_table(
            tracker_page,
            0,
            FileTableKind::Tracker,
            policy,
            cancel,
            &mut traversal,
        )
        .map_err(|error| match error {
            FileTraversalError::Decode(rule) => OperationError::Structural(rule.to_owned()),
            FileTraversalError::Operation(error) => error,
        })?;
        let mut headers = BTreeMap::new();
        headers.insert(tracker.vfid(), tracker);
        for item in traversal.tracker_items {
            if headers.contains_key(&item.vfid) {
                return Err(OperationError::Structural(
                    "file.tracker.item_unique".to_owned(),
                ));
            }
            let vpid = Vpid::new(
                item.vfid.vol_id,
                PageId::new(item.vfid.file_id.get()).map_err(|_| OperationError::Arithmetic)?,
            );
            let owned = OwnedInspectionPage::read(&self.data, vpid)
                .map_err(InspectionPageError::into_operation)?;
            let envelope = owned
                .envelope(vpid)
                .map_err(|error| OperationError::Structural(error.rule().to_owned()))?;
            let header = decode_file_header(&envelope)
                .map_err(|error| OperationError::Structural(error.rule().to_owned()))?;
            if header.vfid() != item.vfid || header.file_type() != item.file_type {
                return Err(OperationError::Structural(
                    "file.tracker.header_match".to_owned(),
                ));
            }
            headers.insert(item.vfid, header);
        }
        let mut allocations = BTreeMap::new();
        let mut sector_claims: BTreeMap<(VolId, SectorId), Vec<SectorClaimView>> = BTreeMap::new();
        for header in headers.values().copied() {
            let facts = self
                .collect_file_allocations(header, policy, cancel)
                .map_err(|error| match error {
                    FileTraversalError::Decode(rule) => OperationError::Structural(format!(
                        "{rule} for file:{}:{}",
                        header.vfid().vol_id.get(),
                        header.vfid().file_id.get()
                    )),
                    FileTraversalError::Operation(error) => error,
                })?;
            for vpid in facts.pages {
                if allocations.insert(vpid, header.vfid()).is_some() {
                    return Err(OperationError::Structural(format!(
                        "file.table.owner_unique at page:{}:{}",
                        vpid.vol_id.get(),
                        vpid.page_id.get()
                    )));
                }
            }
            for (key, kind) in facts.claims {
                sector_claims.entry(key).or_default().push(SectorClaimView {
                    vfid: header.vfid(),
                    kind,
                });
            }
        }
        let claim_conflicts: Vec<DiagnosticRecord> = sector_claims
            .iter()
            .filter(|(_, claims)| claims.len() > 1)
            .map(|((vol_id, sector_id), _)| DiagnosticRecord {
                code: "file.sector.claim_conflict",
                severity: "error",
                message: "Multiple validated file tables claim the same sector.",
                subject: format!("sector:{}:{}", vol_id.get(), sector_id.get()),
                rule: "file.table.sector_owner_unique",
            })
            .collect();
        let class_names = self.resolve_tracked_class_names(&headers, cancel)?;
        let retained = (allocations.len() as u64)
            .checked_mul(size_of::<(Vpid, Vfid)>() as u64)
            .ok_or(OperationError::Arithmetic)?;
        if retained > policy.memory_limit {
            return Err(OperationError::ResourceLimit);
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        let mut next = (*self.data).clone();
        next.revision = next
            .revision
            .next()
            .map_err(|_| OperationError::Arithmetic)?;
        next.tracked_files = headers;
        next.file_allocations = allocations;
        next.sector_claims = sector_claims;
        next.class_names = class_names;
        next.diagnostics.extend(claim_conflicts);
        for header in next.tracked_files.values().copied() {
            let header_page = Vpid::new(
                header.vfid().vol_id,
                PageId::new(header.vfid().file_id.get()).map_err(|_| OperationError::Arithmetic)?,
            );
            next.deep_pages.insert(
                header_page,
                DeepPageFact {
                    slotted: None,
                    file_header: Some(header),
                    raw: None,
                    diagnostic_rule: None,
                },
            );
        }
        refresh_deep_coverage(&mut next, None);
        let total = next.tracked_files.len() as u64;
        next.coverage
            .retain(|coverage| coverage.facet != "file-inventory");
        next.coverage.push(CoverageRecord {
            facet: "file-inventory",
            coverage: Coverage::Complete,
            evaluated: total,
            conclusive: total,
            trusted_total: Some(total),
            stop_reason: None,
        });
        next.outcome = classify_session_outcome(&next);
        Ok(Self {
            data: Arc::new(next),
        })
    }

    #[allow(clippy::too_many_lines)]
    fn collect_file_allocations(
        &self,
        header: FileHeader,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<FileAllocationFacts, FileTraversalError> {
        let temporary = header.flags() & 0x2 != 0;
        let numerable = header.flags() & 0x1 != 0;
        let partial_offset = header
            .partial_table_offset()
            .ok_or(FileTraversalError::Decode(
                "file.header.partial_table_required",
            ))?;
        if header.full_table_offset().is_some() == temporary {
            return Err(FileTraversalError::Decode(
                "file.header.full_table_presence",
            ));
        }
        if header.user_table_offset().is_some() != numerable {
            return Err(FileTraversalError::Decode(
                "file.header.user_table_presence",
            ));
        }
        let start = Vpid::new(
            header.vfid().vol_id,
            PageId::new(header.vfid().file_id.get())
                .map_err(|_| FileTraversalError::Operation(OperationError::Arithmetic))?,
        );
        let mut traversal = FileTraversal::default();
        traversal.table_pages.insert(start);
        self.walk_file_table(
            start,
            partial_offset,
            FileTableKind::Partial,
            policy,
            cancel,
            &mut traversal,
        )?;
        if let Some(offset) = header.full_table_offset() {
            self.walk_file_table(
                start,
                offset,
                FileTableKind::Full,
                policy,
                cancel,
                &mut traversal,
            )?;
        }
        if let Some(offset) = header.user_table_offset() {
            self.walk_file_table(
                start,
                offset,
                FileTableKind::User,
                policy,
                cancel,
                &mut traversal,
            )?;
        }
        let expected_partial = if temporary {
            header.sector_total()
        } else {
            header.sector_partial()
        };
        if traversal.partial_sectors.len() as u64 != u64::from(expected_partial)
            || (!temporary
                && traversal.full_sectors.len() as u64 != u64::from(header.sector_full()))
            || traversal.table_pages.len() as u64 != u64::from(header.page_ftab())
            || (numerable && traversal.user_pages.len() as u64 != u64::from(header.page_user()))
            || traversal
                .user_pages
                .iter()
                .filter(|page| page.marked_deleted)
                .count() as u64
                != u64::from(header.page_marked_delete())
        {
            return Err(FileTraversalError::Decode(
                "file.table.count_reconciliation",
            ));
        }
        let mut sectors = BTreeSet::new();
        let mut allocations = BTreeSet::new();
        let mut claims = Vec::new();
        for partial in traversal.partial_sectors {
            if !sectors.insert((partial.vol_id, partial.sector_id)) {
                return Err(FileTraversalError::Decode("file.table.sector_unique"));
            }
            claims.push((
                (partial.vol_id, partial.sector_id),
                SectorClaimKind::Partial {
                    bitmap: partial.page_bitmap,
                },
            ));
            let first = i64::from(partial.sector_id.get()) * i64::from(SECTOR_PAGES);
            for bit in 0..SECTOR_PAGES {
                if partial.page_bitmap & (1_u64 << bit) != 0 {
                    let page = first + i64::from(bit);
                    let page_id = i32::try_from(page)
                        .ok()
                        .and_then(|value| PageId::new(value).ok())
                        .ok_or(FileTraversalError::Decode("file.table.page_range"))?;
                    let vpid = Vpid::new(partial.vol_id, page_id);
                    self.page(vpid)
                        .map_err(|_| FileTraversalError::Decode("file.table.page_range"))?;
                    allocations.insert(vpid);
                }
            }
        }
        for (vol_id, sector_id) in traversal.full_sectors {
            if !sectors.insert((vol_id, sector_id)) {
                return Err(FileTraversalError::Decode("file.table.sector_unique"));
            }
            claims.push(((vol_id, sector_id), SectorClaimKind::Full));
            let first = i64::from(sector_id.get()) * i64::from(SECTOR_PAGES);
            for bit in 0..SECTOR_PAGES {
                let page = first + i64::from(bit);
                let page_id = i32::try_from(page)
                    .ok()
                    .and_then(|value| PageId::new(value).ok())
                    .ok_or(FileTraversalError::Decode("file.table.page_range"))?;
                let vpid = Vpid::new(vol_id, page_id);
                self.page(vpid)
                    .map_err(|_| FileTraversalError::Decode("file.table.page_range"))?;
                allocations.insert(vpid);
            }
        }
        if allocations.len() as u64
            != u64::from(header.page_total().saturating_sub(header.page_free()))
            || !traversal
                .table_pages
                .iter()
                .all(|page| allocations.contains(page))
            || !traversal
                .user_pages
                .iter()
                .all(|page| allocations.contains(&page.vpid))
        {
            return Err(FileTraversalError::Decode("file.table.page_reconciliation"));
        }
        let retained = (allocations.len() as u64)
            .checked_mul(size_of::<(Vpid, Vfid)>() as u64)
            .and_then(|bytes| {
                bytes.checked_add(
                    (claims.len() as u64)
                        .checked_mul(size_of::<((VolId, SectorId), SectorClaimKind)>() as u64)?,
                )
            })
            .ok_or(FileTraversalError::Operation(OperationError::Arithmetic))?;
        if retained > policy.memory_limit {
            return Err(FileTraversalError::Operation(OperationError::ResourceLimit));
        }
        Ok(FileAllocationFacts {
            pages: allocations,
            claims,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_file_table(
        &self,
        start: Vpid,
        initial_offset: u16,
        kind: FileTableKind,
        policy: ResourcePolicy,
        cancel: &CancelToken,
        traversal: &mut FileTraversal,
    ) -> Result<(), FileTraversalError> {
        let mut current = start;
        let mut offset = initial_offset;
        let mut visited = BTreeSet::new();
        loop {
            if cancel.is_cancelled() {
                return Err(FileTraversalError::Operation(OperationError::Interrupted));
            }
            traversal.steps = traversal
                .steps
                .checked_add(1)
                .ok_or(FileTraversalError::Operation(OperationError::Arithmetic))?;
            traversal.decoded_bytes = traversal
                .decoded_bytes
                .checked_add(crate::format::IO_PAGE_SIZE as u64)
                .ok_or(FileTraversalError::Operation(OperationError::Arithmetic))?;
            if traversal.steps > policy.max_chain_steps
                || traversal.decoded_bytes > policy.max_decoded_bytes
            {
                return Err(FileTraversalError::Operation(OperationError::ResourceLimit));
            }
            if !visited.insert(current) {
                return Err(FileTraversalError::Decode("file.extdata.cycle"));
            }
            if current != start && !traversal.table_pages.insert(current) {
                return Err(FileTraversalError::Decode("file.extdata.page_shared"));
            }
            let owned =
                OwnedInspectionPage::read(&self.data, current).map_err(|error| match error {
                    InspectionPageError::Source(error) => {
                        FileTraversalError::Operation(OperationError::Source(error))
                    }
                    InspectionPageError::Format(rule) => FileTraversalError::Decode(rule),
                    InspectionPageError::EncryptedOpaque => {
                        FileTraversalError::Decode("file.extdata.encrypted")
                    }
                    InspectionPageError::Decrypt => FileTraversalError::Decode("tde.page.decrypt"),
                })?;
            let envelope = owned
                .envelope(current)
                .map_err(|error| FileTraversalError::Decode(error.rule()))?;
            if envelope.page_type() != PageType::FileTable {
                return Err(FileTraversalError::Decode("file.extdata.page_type"));
            }
            let item_size = match kind {
                FileTableKind::Partial | FileTableKind::Tracker => 16,
                FileTableKind::Full | FileTableKind::User => 8,
            };
            let component = decode_extdata_header(&envelope, offset, item_size)
                .map_err(|error| FileTraversalError::Decode(error.rule()))?;
            match kind {
                FileTableKind::Partial => traversal.partial_sectors.extend(
                    decode_partial_sectors(&envelope, component)
                        .map_err(|error| FileTraversalError::Decode(error.rule()))?,
                ),
                FileTableKind::Full => traversal.full_sectors.extend(
                    decode_full_sectors(&envelope, component)
                        .map_err(|error| FileTraversalError::Decode(error.rule()))?,
                ),
                FileTableKind::User => traversal.user_pages.extend(
                    decode_user_pages(&envelope, component)
                        .map_err(|error| FileTraversalError::Decode(error.rule()))?,
                ),
                FileTableKind::Tracker => traversal.tracker_items.extend(
                    decode_tracker_items(&envelope, component)
                        .map_err(|error| FileTraversalError::Decode(error.rule()))?,
                ),
            }
            let Some(next) = component.next else {
                return Ok(());
            };
            current = next;
            offset = 0;
        }
    }

    fn catalog_directories(
        &self,
        vpid: Vpid,
        envelope: &crate::format::DecodedPageEnvelope<'_>,
        slotted: &SlottedPage,
        page: CatalogPageFact,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Vec<CatalogDirectoryView>, &'static str> {
        if page.is_overflow {
            return Ok(Vec::new());
        }
        let catalog_owner = self
            .data
            .file_allocations
            .get(&vpid)
            .copied()
            .ok_or("catalog.directory.owner")?;
        let mut decoded_bytes = crate::format::IO_PAGE_SIZE as u64;
        let mut directories = Vec::new();
        for slot in slotted.slots().iter().skip(1).filter(|slot| {
            !slot.is_empty()
                && slot.record_type() == crate::format::RecordType::Home
                && slot.length() == 32
        }) {
            let Ok(directory) = decode_catalog_directory(envelope, slotted, slot.slot_id()) else {
                continue;
            };
            let mut class_items = directory
                .items
                .iter()
                .filter(|item| item.representation_id == -1);
            let Some(class_item) = class_items.next() else {
                continue;
            };
            if class_items.next().is_some()
                || directory
                    .items
                    .iter()
                    .any(|item| item.representation_id < -1)
            {
                continue;
            }
            let class_info = self.decode_catalog_target(
                class_item.target,
                catalog_owner,
                &mut decoded_bytes,
                policy,
                cancel,
                decode_catalog_class_info,
            )?;
            let source = Oid::new(
                vpid.vol_id,
                vpid.page_id,
                SlotId::new(i16::try_from(slot.slot_id()).map_err(|_| "catalog.directory.slot")?)
                    .map_err(|_| "catalog.directory.slot")?,
            );
            if class_info.representation_directory != source {
                continue;
            }
            let heap_header = match (class_info.heap_file, class_info.heap_header) {
                (Some(heap_file), Some(heap_page)) => {
                    let Some(header) = self.data.tracked_files.get(&heap_file) else {
                        continue;
                    };
                    if !matches!(
                        header.file_type(),
                        crate::format::FileType::Heap | crate::format::FileType::HeapReuseSlots
                    ) || header.heap_header_page() != Some(heap_page)
                    {
                        continue;
                    }
                    Some(header)
                }
                (None, None) => None,
                _ => return Err("catalog.class_info.heap_pair"),
            };
            let mut representations = Vec::new();
            for item in directory
                .items
                .iter()
                .filter(|item| item.representation_id >= 0)
            {
                let representation = self.decode_catalog_target(
                    item.target,
                    catalog_owner,
                    &mut decoded_bytes,
                    policy,
                    cancel,
                    decode_catalog_representation_header,
                )?;
                if representation.representation_id != i32::from(item.representation_id) {
                    return Err("catalog.representation.id_match");
                }
                representations.push(representation);
            }
            directories.push(CatalogDirectoryView {
                slot_id: slot.slot_id(),
                class_oid: heap_header.and_then(|header| header.class_oid()),
                class_info,
                representations,
            });
        }
        if directories.len() != usize::try_from(page.directory_count).unwrap_or(usize::MAX) {
            return Err("catalog.page.directory_roles");
        }
        Ok(directories)
    }

    fn decode_catalog_target<T, F>(
        &self,
        oid: Oid,
        catalog_owner: Vfid,
        decoded_bytes: &mut u64,
        policy: ResourcePolicy,
        cancel: &CancelToken,
        decoder: F,
    ) -> Result<T, &'static str>
    where
        F: FnOnce(
            &crate::format::DecodedPageEnvelope<'_>,
            &SlottedPage,
            u16,
        ) -> Result<T, DecodeError>,
    {
        if cancel.is_cancelled() {
            return Err("interrupted");
        }
        *decoded_bytes = decoded_bytes
            .checked_add(crate::format::IO_PAGE_SIZE as u64)
            .filter(|bytes| *bytes <= policy.max_decoded_bytes)
            .ok_or("resource-limit")?;
        let vpid = Vpid::new(oid.vol_id, oid.page_id);
        if self.data.file_allocations.get(&vpid) != Some(&catalog_owner) {
            return Err("catalog.record.owner");
        }
        let owned = OwnedInspectionPage::read(&self.data, vpid).map_err(|error| match error {
            InspectionPageError::Source(_) => "catalog.record.unreadable",
            InspectionPageError::Format(rule) => rule,
            InspectionPageError::EncryptedOpaque => "catalog.record.encrypted",
            InspectionPageError::Decrypt => "tde.page.decrypt",
        })?;
        if owned.decrypted_user.is_some() {
            *decoded_bytes = decoded_bytes
                .checked_add(crate::format::DB_PAGE_SIZE as u64)
                .filter(|bytes| *bytes <= policy.max_decoded_bytes)
                .ok_or("resource-limit")?;
        }
        let envelope = owned.envelope(vpid).map_err(|error| error.rule())?;
        if envelope.page_type() != PageType::Catalog {
            return Err("catalog.record.page_type");
        }
        let slotted = decode_slotted_page(&envelope).map_err(|error| error.rule())?;
        let slot_id = u16::try_from(oid.slot_id.get()).map_err(|_| "catalog.record.slot")?;
        decoder(&envelope, &slotted, slot_id).map_err(|error| error.rule())
    }

    #[must_use]
    pub fn deep_page(&self, vpid: Vpid) -> Option<DeepPageView> {
        self.data.deep_pages.get(&vpid).map(|fact| DeepPageView {
            vpid,
            revision: self.data.revision,
            slotted: fact.slotted.clone(),
            file_header: fact.file_header,
            raw: fact.raw.clone(),
            diagnostic_rule: fact.diagnostic_rule,
        })
    }

    #[must_use]
    pub fn deep_pages(&self) -> Vec<DeepPageView> {
        self.data
            .deep_pages
            .iter()
            .map(|(vpid, fact)| DeepPageView {
                vpid: *vpid,
                revision: self.data.revision,
                slotted: fact.slotted.clone(),
                file_header: fact.file_header,
                raw: fact.raw.clone(),
                diagnostic_rule: fact.diagnostic_rule,
            })
            .collect()
    }

    /// Validate the typed edge carried by one explicitly selected
    /// `REC_RELOCATION`. Both endpoints must belong to the same tracked heap,
    /// and the destination must be a live `REC_NEWHOME` slot.
    #[allow(clippy::too_many_lines)]
    pub fn enrich_relocation(
        &self,
        source: Oid,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Self, OperationError> {
        if self.data.relocation_edges.contains_key(&source) {
            return Ok(self.clone());
        }
        if cancel.is_cancelled() {
            return self.publish_relocation_edge(source, None, Some("interrupted"));
        }
        if policy.max_decoded_bytes < crate::format::IO_PAGE_SIZE as u64
            || policy.memory_limit < crate::format::IO_PAGE_SIZE as u64
        {
            return self.publish_relocation_edge(source, None, Some("resource-limit"));
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        let source_vpid = Vpid::new(source.vol_id, source.page_id);
        let Some(heap_owner) = self.data.file_allocations.get(&source_vpid).copied() else {
            return self.publish_relocation_edge(source, None, Some("heap.relocation.heap_owner"));
        };
        let valid_heap_owner = self
            .data
            .tracked_files
            .get(&heap_owner)
            .is_some_and(|header| {
                matches!(
                    header.file_type(),
                    crate::format::FileType::Heap | crate::format::FileType::HeapReuseSlots
                )
            });
        if !valid_heap_owner {
            return self.publish_relocation_edge(source, None, Some("heap.relocation.heap_owner"));
        }
        let source_owned = match OwnedInspectionPage::read(&self.data, source_vpid) {
            Ok(page) => page,
            Err(InspectionPageError::Source(error)) => return Err(OperationError::Source(error)),
            Err(InspectionPageError::Format(rule)) => {
                return self.publish_relocation_edge(source, None, Some(rule));
            }
            Err(InspectionPageError::EncryptedOpaque) => {
                return self.publish_relocation_edge(
                    source,
                    None,
                    Some("heap.relocation.encrypted"),
                );
            }
            Err(InspectionPageError::Decrypt) => {
                return self.publish_relocation_edge(source, None, Some("tde.page.decrypt"));
            }
        };
        let source_envelope = match source_owned.envelope(source_vpid) {
            Ok(value) if value.page_type() == PageType::Heap => value,
            Ok(_) => {
                return self.publish_relocation_edge(
                    source,
                    None,
                    Some("heap.relocation.source_page_role"),
                );
            }
            Err(error) => {
                return self.publish_relocation_edge(source, None, Some(error.rule()));
            }
        };
        let source_slotted = match decode_slotted_page(&source_envelope) {
            Ok(value) => value,
            Err(error) => {
                return self.publish_relocation_edge(source, None, Some(error.rule()));
            }
        };
        let source_slot =
            u16::try_from(source.slot_id.get()).map_err(|_| OperationError::Arithmetic)?;
        let target = match decode_relocation_target(&source_envelope, &source_slotted, source_slot)
        {
            Ok(value) => value,
            Err(error) => {
                return self.publish_relocation_edge(source, None, Some(error.rule()));
            }
        };
        drop(source_slotted);
        drop(source_owned);
        let target_vpid = Vpid::new(target.vol_id, target.page_id);
        if policy.max_decoded_bytes < 2 * crate::format::IO_PAGE_SIZE as u64 {
            return self.publish_relocation_edge(source, Some(target), Some("resource-limit"));
        }
        let target_role_valid = self.data.file_allocations.get(&target_vpid) == Some(&heap_owner)
            && self.page(target_vpid).is_ok_and(|page| {
                page.page_type == Some(PageType::Heap)
                    && page.availability == Availability::Available
            });
        if !target_role_valid {
            return self.publish_relocation_edge(
                source,
                Some(target),
                Some("heap.relocation.target_page_role"),
            );
        }
        let target_owned = match OwnedInspectionPage::read(&self.data, target_vpid) {
            Ok(page) => page,
            Err(InspectionPageError::Source(error)) => return Err(OperationError::Source(error)),
            Err(InspectionPageError::Format(rule)) => {
                return self.publish_relocation_edge(source, Some(target), Some(rule));
            }
            Err(InspectionPageError::EncryptedOpaque) => {
                return self.publish_relocation_edge(
                    source,
                    Some(target),
                    Some("heap.relocation.encrypted"),
                );
            }
            Err(InspectionPageError::Decrypt) => {
                return self.publish_relocation_edge(
                    source,
                    Some(target),
                    Some("tde.page.decrypt"),
                );
            }
        };
        let target_envelope = match target_owned.envelope(target_vpid) {
            Ok(value) if value.page_type() == PageType::Heap => value,
            Ok(_) => {
                return self.publish_relocation_edge(
                    source,
                    Some(target),
                    Some("heap.relocation.target_page_role"),
                );
            }
            Err(error) => {
                return self.publish_relocation_edge(source, Some(target), Some(error.rule()));
            }
        };
        let target_slotted = match decode_slotted_page(&target_envelope) {
            Ok(value) => value,
            Err(error) => {
                return self.publish_relocation_edge(source, Some(target), Some(error.rule()));
            }
        };
        let target_slot = usize::try_from(target.slot_id.get())
            .ok()
            .and_then(|slot| target_slotted.slots().get(slot));
        if !target_slot.is_some_and(|slot| {
            slot.record_type() == RecordType::NewHome && slot.offset() != 0 && slot.length() >= 8
        }) {
            return self.publish_relocation_edge(
                source,
                Some(target),
                Some("heap.relocation.target_slot_role"),
            );
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        self.publish_relocation_edge(source, Some(target), None)
    }

    #[must_use]
    pub fn relocation_edge(&self, source: Oid) -> Option<RelocationEdgeView> {
        self.data
            .relocation_edges
            .get(&source)
            .map(|fact| RelocationEdgeView {
                source,
                revision: self.data.revision,
                target: fact.target,
                valid: fact.valid,
                diagnostic_rule: fact.diagnostic_rule,
            })
    }

    #[must_use]
    pub fn relocation_edges(&self) -> Vec<RelocationEdgeView> {
        self.data
            .relocation_edges
            .keys()
            .filter_map(|source| self.relocation_edge(*source))
            .collect()
    }

    /// The sector a page belongs to. Sector `s` is exactly pages `64s` through
    /// `64s + 63`, with no sub-sector aliasing.
    fn sector_of(page_id: PageId) -> Option<SectorId> {
        let raw = u32::try_from(page_id.get()).ok()? / SECTOR_PAGES;
        SectorId::new(i32::try_from(raw).ok()?).ok()
    }

    /// The class OID whose heap holds `vpid`, from the sector cache when a page
    /// of that sector has already been resolved, otherwise from slot 0.
    ///
    /// A heap sector belongs to exactly one file for the file's whole lifetime
    /// and a class has exactly one heap file, so one resolution serves every
    /// page of the sector (research §5).
    fn class_oid_for_heap_page(&self, vpid: Vpid) -> Result<Oid, &'static str> {
        if let Some(oid) = Self::sector_of(vpid.page_id)
            .and_then(|sector| self.data.sector_classes.get(&(vpid.vol_id, sector)))
        {
            return Ok(*oid);
        }
        let owned = OwnedInspectionPage::read(&self.data, vpid)
            .map_err(|_| "heap page could not be read")?;
        let envelope = owned
            .envelope(vpid)
            .map_err(|_| "heap page envelope is invalid")?;
        if envelope.page_type() != PageType::Heap {
            return Err(
                "only heap pages hold class instances, so this page's records carry no attribute values",
            );
        }
        let slotted =
            decode_slotted_page(&envelope).map_err(|_| "heap page is not a valid slotted page")?;
        // Slot 0 is HEAP_HDR_STATS on a heap header page and HEAP_CHAIN on a
        // data page; the class OID is field 0 of both, so try each role.
        let fact = decode_heap_page(&envelope, &slotted, true)
            .or_else(|_| decode_heap_page(&envelope, &slotted, false))
            .map_err(|_| "heap page slot 0 is not a recognized heap record")?;
        let class_oid = match fact {
            HeapPageFact::Header(header) => header.class_oid,
            HeapPageFact::Chain(chain) => chain.class_oid,
        };
        // A NULL class OID is the root and boot heaps. Interpreting those needs
        // boot_dbparm, which version one does not read.
        class_oid.ok_or("root and system heap records are not interpreted")
    }

    /// Parses one representation out of a class object's own heap record,
    /// following `REC_RELOCATION` with a bounded, cycle-checked walk.
    fn resolve_class_representation(
        &self,
        class_oid: Oid,
        target: RepresentationTarget,
    ) -> Result<ClassRepresentationFact, &'static str> {
        let mut current = class_oid;
        let mut visited = BTreeSet::new();
        for _ in 0..8 {
            if !visited.insert(current) {
                return Err("class record relocation cycle");
            }
            let vpid = Vpid::new(current.vol_id, current.page_id);
            let owned = OwnedInspectionPage::read(&self.data, vpid)
                .map_err(|_| "class record page could not be read")?;
            let envelope = owned
                .envelope(vpid)
                .map_err(|_| "class record page envelope is invalid")?;
            let slotted = decode_slotted_page(&envelope)
                .map_err(|_| "class record page is not a valid heap page")?;
            let slot_id = u16::try_from(current.slot_id.get())
                .map_err(|_| "class record slot identifier is invalid")?;
            let slot = slotted
                .slots()
                .get(usize::from(slot_id))
                .ok_or("class record slot does not exist")?;
            match slot.record_type() {
                RecordType::Home | RecordType::NewHome => {
                    let (header, body) =
                        decode_heap_record_body(&envelope, &slotted, slot_id, true)
                            .map_err(|_| "class record could not be read")?;
                    return decode_class_representation(
                        body,
                        header.variable_offset_width,
                        header.representation_id,
                        target,
                    )
                    .map_err(|_| match target {
                        RepresentationTarget::Current => {
                            "the class record could not be interpreted"
                        }
                        RepresentationTarget::Id(_) => {
                            "the record's representation is not described by its class"
                        }
                    });
                }
                RecordType::Relocation => {
                    current = decode_relocation_target(&envelope, &slotted, slot_id)
                        .map_err(|_| "class record relocation target is invalid")?;
                }
                RecordType::BigOne => {
                    return Err("multipage class records are not yet decoded");
                }
                RecordType::Unknown
                | RecordType::AssignAddress
                | RecordType::MarkDeleted
                | RecordType::DeletedWillReuse
                | RecordType::Reserved(_) => return Err("class record slot is not live"),
            }
        }
        Err("class record relocation limit reached")
    }

    /// Resolve and publish the current representation of the class whose heap
    /// holds `vpid`. Schema evidence is useful on its own — the representation
    /// is an independently renderable entity — and it seeds the sector cache
    /// that record interpretation then reuses.
    pub fn enrich_class_representation(
        &self,
        vpid: Vpid,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Self, OperationError> {
        if cancel.is_cancelled() {
            return Err(OperationError::Interrupted);
        }
        if policy.max_decoded_bytes < crate::format::IO_PAGE_SIZE as u64
            || policy.memory_limit < crate::format::IO_PAGE_SIZE as u64
        {
            return Err(OperationError::Unsupported);
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        let class_oid = match self.class_oid_for_heap_page(vpid) {
            Ok(oid) => oid,
            Err(reason) => {
                return self.publish_interpretation(
                    vpid,
                    Vec::new(),
                    Vec::new(),
                    None,
                    Some(reason),
                );
            }
        };
        let representation =
            match self.resolve_class_representation(class_oid, RepresentationTarget::Current) {
                Ok(representation) => representation,
                Err(reason) => {
                    return self.publish_interpretation(
                        vpid,
                        Vec::new(),
                        Vec::new(),
                        Some((vpid, class_oid)),
                        Some(reason),
                    );
                }
            };
        let key = (class_oid, representation.representation_id);
        if self
            .data
            .class_representations
            .get(&key)
            .is_some_and(|existing| existing.representation.is_some())
            && Self::sector_of(vpid.page_id).is_some_and(|sector| {
                self.data
                    .sector_classes
                    .contains_key(&(vpid.vol_id, sector))
            })
        {
            // Already committed to this revision; re-requesting it is a no-op
            // rather than a new revision carrying identical facts.
            return Ok(self.clone());
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        self.publish_interpretation(
            vpid,
            vec![(key, representation)],
            Vec::new(),
            Some((vpid, class_oid)),
            None,
        )
    }

    /// Interpret every home record of one heap page as a single enrichment.
    ///
    /// Page granularity is deliberate: resolving the page's class record once
    /// amortizes over every record on it, so one operator action produces one
    /// revision (`docs/adr/0002-classrepr-from-class-record.md`).
    #[allow(clippy::too_many_lines)]
    pub fn enrich_record_page(
        &self,
        vpid: Vpid,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Self, OperationError> {
        if cancel.is_cancelled() {
            return Err(OperationError::Interrupted);
        }
        if policy.max_decoded_bytes < crate::format::IO_PAGE_SIZE as u64
            || policy.memory_limit < crate::format::IO_PAGE_SIZE as u64
        {
            return Err(OperationError::Unsupported);
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        let class_oid = match self.class_oid_for_heap_page(vpid) {
            Ok(oid) => oid,
            Err(reason) => {
                return self.publish_interpretation(
                    vpid,
                    Vec::new(),
                    Vec::new(),
                    None,
                    Some(reason),
                );
            }
        };

        let owned = match OwnedInspectionPage::read(&self.data, vpid) {
            Ok(page) => page,
            Err(InspectionPageError::Source(error)) => return Err(OperationError::Source(error)),
            Err(InspectionPageError::Format(rule)) => {
                return self.publish_interpretation(vpid, Vec::new(), Vec::new(), None, Some(rule));
            }
            Err(InspectionPageError::EncryptedOpaque) => {
                return self.publish_interpretation(
                    vpid,
                    Vec::new(),
                    Vec::new(),
                    None,
                    Some("the page's user region is encrypted and was not decrypted"),
                );
            }
            Err(InspectionPageError::Decrypt) => {
                return self.publish_interpretation(
                    vpid,
                    Vec::new(),
                    Vec::new(),
                    None,
                    Some("tde.page.decrypt"),
                );
            }
        };
        let envelope = match owned.envelope(vpid) {
            Ok(value) if value.page_type() == PageType::Heap => value,
            Ok(_) => {
                return self.publish_interpretation(
                    vpid,
                    Vec::new(),
                    Vec::new(),
                    None,
                    Some(
                        "only heap pages hold class instances, so this page's records carry no attribute values",
                    ),
                );
            }
            Err(error) => {
                return self.publish_interpretation(
                    vpid,
                    Vec::new(),
                    Vec::new(),
                    None,
                    Some(error.rule()),
                );
            }
        };
        let slotted = match decode_slotted_page(&envelope) {
            Ok(value) => value,
            Err(error) => {
                return self.publish_interpretation(
                    vpid,
                    Vec::new(),
                    Vec::new(),
                    None,
                    Some(error.rule()),
                );
            }
        };

        let mut representations: BTreeMap<(Oid, u32), ClassRepresentationFact> = BTreeMap::new();
        let mut records: Vec<(Oid, RecordInterpretationEvidence)> = Vec::new();
        // Slot 0 carries the page's own heap metadata, never a user record.
        for slot in slotted.slots().iter().skip(1) {
            if cancel.is_cancelled() {
                return Err(OperationError::Interrupted);
            }
            if slot.is_empty() {
                continue;
            }
            let slot_id = slot.slot_id();
            let Ok(record_oid) = i16::try_from(slot_id)
                .map_err(|_| ())
                .and_then(|raw| SlotId::new(raw).map_err(|_| ()))
                .map(|slot| Oid::new(vpid.vol_id, vpid.page_id, slot))
            else {
                continue;
            };
            match slot.record_type() {
                RecordType::Home | RecordType::NewHome => {
                    let evidence = self.interpret_one_record(
                        &envelope,
                        &slotted,
                        slot_id,
                        class_oid,
                        None,
                        &mut representations,
                    );
                    records.push((record_oid, evidence));
                }
                RecordType::Relocation => {
                    // Both facts matter: the forward reference is the record's
                    // own content, and the target carries the values.
                    if let Ok(target) = decode_relocation_target(&envelope, &slotted, slot_id)
                        && let Some(evidence) = self.interpret_relocated_record(
                            target,
                            record_oid,
                            &mut representations,
                        )
                    {
                        records.push((target, evidence));
                    }
                }
                // A REC_BIGONE record's values live in an overflow file; only
                // its forward reference is a fact of this page (backlog B3).
                RecordType::BigOne
                | RecordType::Unknown
                | RecordType::AssignAddress
                | RecordType::MarkDeleted
                | RecordType::DeletedWillReuse
                | RecordType::Reserved(_) => {}
            }
        }

        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        let already_published = !records.is_empty()
            && records
                .iter()
                .all(|(oid, _)| self.data.record_interpretations.contains_key(oid));
        if already_published {
            return Ok(self.clone());
        }
        self.publish_interpretation(
            vpid,
            representations.into_iter().collect(),
            records,
            Some((vpid, class_oid)),
            None,
        )
    }

    /// Interprets one home record, resolving whichever representation its own
    /// header names and caching it for the rest of the page.
    fn interpret_one_record(
        &self,
        envelope: &crate::format::DecodedPageEnvelope<'_>,
        slotted: &SlottedPage,
        slot_id: u16,
        class_oid: Oid,
        relocated_from: Option<Oid>,
        representations: &mut BTreeMap<(Oid, u32), ClassRepresentationFact>,
    ) -> RecordInterpretationEvidence {
        let record_type = slotted
            .slots()
            .get(usize::from(slot_id))
            .map_or(RecordType::Unknown, |slot| slot.record_type());
        let failed = |reason: &'static str, representation_id: u32| RecordInterpretationEvidence {
            class_oid,
            representation_id,
            record_type,
            relocated_from,
            layout: None,
            attributes: Vec::new(),
            diagnostic_rule: Some(reason),
        };
        let Ok((header, body)) = decode_heap_record_body(envelope, slotted, slot_id, true) else {
            return failed("the record could not be read", 0);
        };
        let key = (class_oid, header.representation_id);
        if let std::collections::btree_map::Entry::Vacant(slot) = representations.entry(key) {
            // Prefer a representation already committed to the graph; resolving
            // it again would re-read the class record for every page.
            let resolved = self
                .data
                .class_representations
                .get(&key)
                .and_then(|evidence| evidence.representation.clone())
                .map_or_else(
                    || {
                        self.resolve_class_representation(
                            class_oid,
                            RepresentationTarget::Id(header.representation_id),
                        )
                    },
                    Ok,
                );
            match resolved {
                Ok(representation) => {
                    slot.insert(representation);
                }
                Err(reason) => return failed(reason, header.representation_id),
            }
        }
        let representation = &representations[&key];
        match decode_record_interpretation(body, &header, representation) {
            Ok(record) => RecordInterpretationEvidence {
                class_oid,
                representation_id: header.representation_id,
                record_type,
                relocated_from,
                layout: Some(record.layout),
                attributes: record.attributes,
                diagnostic_rule: None,
            },
            Err(_) => failed(
                "the record's layout does not match its representation",
                header.representation_id,
            ),
        }
    }

    /// Interprets the one-hop target of a relocation, which may live on another
    /// page and therefore under another class.
    fn interpret_relocated_record(
        &self,
        target: Oid,
        source: Oid,
        representations: &mut BTreeMap<(Oid, u32), ClassRepresentationFact>,
    ) -> Option<RecordInterpretationEvidence> {
        let vpid = Vpid::new(target.vol_id, target.page_id);
        let class_oid = self.class_oid_for_heap_page(vpid).ok()?;
        let owned = OwnedInspectionPage::read(&self.data, vpid).ok()?;
        let envelope = owned.envelope(vpid).ok()?;
        if envelope.page_type() != PageType::Heap {
            return None;
        }
        let slotted = decode_slotted_page(&envelope).ok()?;
        let slot_id = u16::try_from(target.slot_id.get()).ok()?;
        Some(self.interpret_one_record(
            &envelope,
            &slotted,
            slot_id,
            class_oid,
            Some(source),
            representations,
        ))
    }

    #[must_use]
    pub fn class_representation(
        &self,
        class_oid: Oid,
        representation_id: u32,
    ) -> Option<ClassRepresentationView> {
        self.data
            .class_representations
            .get(&(class_oid, representation_id))
            .map(|evidence| ClassRepresentationView {
                class_oid,
                representation_id,
                revision: self.data.revision,
                representation: evidence.representation.clone(),
                diagnostic_rule: evidence.diagnostic_rule,
            })
    }

    #[must_use]
    pub fn class_representations(&self) -> Vec<ClassRepresentationView> {
        self.data
            .class_representations
            .keys()
            .filter_map(|(class_oid, representation_id)| {
                self.class_representation(*class_oid, *representation_id)
            })
            .collect()
    }

    #[must_use]
    pub fn record_interpretation(&self, record: Oid) -> Option<RecordInterpretationView> {
        self.data
            .record_interpretations
            .get(&record)
            .map(|evidence| RecordInterpretationView {
                record,
                revision: self.data.revision,
                class_oid: evidence.class_oid,
                representation_id: evidence.representation_id,
                record_type: evidence.record_type,
                relocated_from: evidence.relocated_from,
                layout: evidence.layout,
                attributes: evidence.attributes.clone(),
                diagnostic_rule: evidence.diagnostic_rule,
            })
    }

    /// The interpretation that speaks for `slot`, following a published
    /// relocation edge to its target.
    ///
    /// Deciding this is graph navigation, not presentation, so every adapter
    /// asks rather than each re-deriving it.
    #[must_use]
    pub fn slot_interpretation(&self, slot: Oid) -> Option<RecordInterpretationView> {
        let target = self
            .relocation_edge(slot)
            .and_then(|edge| edge.target)
            .unwrap_or(slot);
        self.record_interpretation(target)
    }

    /// Why the records of `vpid` were not interpreted, when a requested
    /// interpretation degraded the whole page.
    #[must_use]
    pub fn record_page_interpretation_failure(&self, vpid: Vpid) -> Option<&'static str> {
        self.data.interpretation_failures.get(&vpid).copied()
    }

    #[must_use]
    pub fn record_interpretations(&self) -> Vec<RecordInterpretationView> {
        self.data
            .record_interpretations
            .keys()
            .filter_map(|record| self.record_interpretation(*record))
            .collect()
    }

    /// Commits one interpretation enrichment: any representations it resolved,
    /// any records it interpreted, and the sector-to-class association, as a
    /// single revision.
    fn publish_interpretation(
        &self,
        vpid: Vpid,
        representations: Vec<((Oid, u32), ClassRepresentationFact)>,
        records: Vec<(Oid, RecordInterpretationEvidence)>,
        sector_class: Option<(Vpid, Oid)>,
        failure: Option<&'static str>,
    ) -> Result<Self, OperationError> {
        let mut next = (*self.data).clone();
        next.revision = next
            .revision
            .next()
            .map_err(|_| OperationError::Arithmetic)?;
        for (key, representation) in representations {
            next.class_representations.insert(
                key,
                ClassRepresentationEvidence {
                    representation: Some(representation),
                    diagnostic_rule: None,
                },
            );
        }
        let interpreted_any = !records.is_empty();
        for (oid, evidence) in records {
            // A record already attributed to the relocation that reached it
            // keeps that attribution: re-interpreting the target page on its own
            // would otherwise erase where the values were requested from.
            if evidence.relocated_from.is_none()
                && next
                    .record_interpretations
                    .get(&oid)
                    .is_some_and(|existing| existing.relocated_from.is_some())
            {
                continue;
            }
            next.record_interpretations.insert(oid, evidence);
        }
        if interpreted_any {
            // A later successful pass supersedes an earlier reason.
            next.interpretation_failures.remove(&vpid);
        }
        if let Some((page, class_oid)) = sector_class
            && let Some(sector) = Self::sector_of(page.page_id)
        {
            next.sector_classes.insert((page.vol_id, sector), class_oid);
        }
        if let Some(reason) = failure {
            // The whole page degrades to its structural view, and the reason is
            // durable evidence rather than a dropped request.
            next.interpretation_failures.insert(vpid, reason);
            next.diagnostics.push(DiagnosticRecord {
                code: "record.interpretation.unavailable",
                severity: "warning",
                message: "The selected page's records were not interpreted.",
                subject: format!("page:{}:{}", vpid.vol_id.get(), vpid.page_id.get()),
                rule: reason,
            });
            if let Ok(class_oid) = self.class_oid_for_heap_page(vpid) {
                next.class_representations.entry((class_oid, 0)).or_insert(
                    ClassRepresentationEvidence {
                        representation: None,
                        diagnostic_rule: Some(reason),
                    },
                );
            }
        }
        refresh_interpretation_coverage(&mut next, failure);
        next.outcome = classify_session_outcome(&next);
        Ok(Self {
            data: Arc::new(next),
        })
    }

    /// Validate the overflow chain referenced by one explicitly selected
    /// `REC_BIGONE`. The record payload and overflow payload bytes are never
    /// retained; only typed links and byte extents are published.
    #[allow(clippy::too_many_lines)]
    pub fn enrich_bigone(
        &self,
        source: Oid,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Self, OperationError> {
        if self.data.overflow_chains.contains_key(&source) {
            return Ok(self.clone());
        }
        if cancel.is_cancelled() {
            return self.publish_overflow_chain(
                source,
                None,
                None,
                0,
                Vec::new(),
                Some("interrupted"),
            );
        }
        if policy.max_decoded_bytes < crate::format::IO_PAGE_SIZE as u64
            || policy.memory_limit < crate::format::IO_PAGE_SIZE as u64
        {
            return self.publish_overflow_chain(
                source,
                None,
                None,
                0,
                Vec::new(),
                Some("resource-limit"),
            );
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        let home = Vpid::new(source.vol_id, source.page_id);
        let Some(heap_owner) = self.data.file_allocations.get(&home).copied() else {
            return self.publish_overflow_chain(
                source,
                None,
                None,
                0,
                Vec::new(),
                Some("overflow.chain.heap_owner"),
            );
        };
        let Some(heap_header) = self.data.tracked_files.get(&heap_owner).copied() else {
            return self.publish_overflow_chain(
                source,
                None,
                None,
                0,
                Vec::new(),
                Some("overflow.chain.heap_owner"),
            );
        };
        if !matches!(
            heap_header.file_type(),
            crate::format::FileType::Heap | crate::format::FileType::HeapReuseSlots
        ) {
            return self.publish_overflow_chain(
                source,
                None,
                None,
                0,
                Vec::new(),
                Some("overflow.chain.heap_owner"),
            );
        }
        let source_owned = match OwnedInspectionPage::read(&self.data, home) {
            Ok(page) => page,
            Err(InspectionPageError::Source(error)) => return Err(OperationError::Source(error)),
            Err(InspectionPageError::Format(rule)) => {
                return self.publish_overflow_chain(source, None, None, 0, Vec::new(), Some(rule));
            }
            Err(InspectionPageError::EncryptedOpaque) => {
                return self.publish_overflow_chain(
                    source,
                    None,
                    None,
                    0,
                    Vec::new(),
                    Some("overflow.chain.encrypted"),
                );
            }
            Err(InspectionPageError::Decrypt) => {
                return self.publish_overflow_chain(
                    source,
                    None,
                    None,
                    0,
                    Vec::new(),
                    Some("tde.page.decrypt"),
                );
            }
        };
        let source_envelope = match source_owned.envelope(home) {
            Ok(value) if value.page_type() == PageType::Heap => value,
            Ok(_) => {
                return self.publish_overflow_chain(
                    source,
                    None,
                    None,
                    0,
                    Vec::new(),
                    Some("overflow.chain.heap_page_role"),
                );
            }
            Err(error) => {
                return self.publish_overflow_chain(
                    source,
                    None,
                    None,
                    0,
                    Vec::new(),
                    Some(error.rule()),
                );
            }
        };
        let source_slotted = match decode_slotted_page(&source_envelope) {
            Ok(value) => value,
            Err(error) => {
                return self.publish_overflow_chain(
                    source,
                    None,
                    None,
                    0,
                    Vec::new(),
                    Some(error.rule()),
                );
            }
        };
        let source_slot =
            u16::try_from(source.slot_id.get()).map_err(|_| OperationError::Arithmetic)?;
        let head = match decode_bigone_target(&source_envelope, &source_slotted, source_slot) {
            Ok(value) => value,
            Err(error) => {
                return self.publish_overflow_chain(
                    source,
                    None,
                    None,
                    0,
                    Vec::new(),
                    Some(error.rule()),
                );
            }
        };
        drop(source_slotted);
        drop(source_owned);
        let Some(overflow_owner) = self.data.file_allocations.get(&head).copied() else {
            return self.publish_overflow_chain(
                source,
                Some(head),
                None,
                0,
                Vec::new(),
                Some("overflow.chain.file_owner"),
            );
        };
        let valid_owner = self
            .data
            .tracked_files
            .get(&overflow_owner)
            .is_some_and(|header| {
                header.file_type() == crate::format::FileType::MultipageObjectHeap
                    && header.related_heap()
                        == heap_header
                            .heap_header_page()
                            .map(|heap_header_page| (heap_owner, heap_header_page))
            });
        if !valid_owner {
            return self.publish_overflow_chain(
                source,
                Some(head),
                None,
                0,
                Vec::new(),
                Some("overflow.chain.file_owner"),
            );
        }

        let mut current = head;
        let mut visited = BTreeSet::new();
        let mut pages = Vec::new();
        let mut total = None;
        let mut remaining = 0_u32;
        let mut validated_payload_bytes = 0_u64;
        let mut decoded_bytes = crate::format::IO_PAGE_SIZE as u64;
        loop {
            if cancel.is_cancelled() {
                return self.publish_overflow_chain(
                    source,
                    Some(head),
                    total,
                    validated_payload_bytes,
                    pages,
                    Some("interrupted"),
                );
            }
            if u64::try_from(pages.len()).unwrap_or(u64::MAX) >= policy.max_chain_steps {
                return self.publish_overflow_chain(
                    source,
                    Some(head),
                    total,
                    validated_payload_bytes,
                    pages,
                    Some("resource-limit"),
                );
            }
            if !visited.insert(current) {
                return self.publish_overflow_chain(
                    source,
                    Some(head),
                    total,
                    validated_payload_bytes,
                    pages,
                    Some("overflow.chain.acyclic"),
                );
            }
            decoded_bytes = match decoded_bytes.checked_add(crate::format::IO_PAGE_SIZE as u64) {
                Some(value)
                    if value <= policy.max_decoded_bytes
                        && value <= policy.memory_limit.saturating_mul(2) =>
                {
                    value
                }
                _ => {
                    return self.publish_overflow_chain(
                        source,
                        Some(head),
                        total,
                        validated_payload_bytes,
                        pages,
                        Some("resource-limit"),
                    );
                }
            };
            let page_role_ok = self.page(current).is_ok_and(|page| {
                page.page_type == Some(PageType::Overflow)
                    && page.availability == Availability::Available
            }) && self.data.file_allocations.get(&current)
                == Some(&overflow_owner);
            if !page_role_ok {
                return self.publish_overflow_chain(
                    source,
                    Some(head),
                    total,
                    validated_payload_bytes,
                    pages,
                    Some("overflow.chain.page_role"),
                );
            }
            let owned = match OwnedInspectionPage::read(&self.data, current) {
                Ok(page) => page,
                Err(InspectionPageError::Source(error)) => {
                    return Err(OperationError::Source(error));
                }
                Err(InspectionPageError::Format(rule)) => {
                    return self.publish_overflow_chain(
                        source,
                        Some(head),
                        total,
                        validated_payload_bytes,
                        pages,
                        Some(rule),
                    );
                }
                Err(InspectionPageError::EncryptedOpaque) => {
                    return self.publish_overflow_chain(
                        source,
                        Some(head),
                        total,
                        validated_payload_bytes,
                        pages,
                        Some("overflow.chain.encrypted"),
                    );
                }
                Err(InspectionPageError::Decrypt) => {
                    return self.publish_overflow_chain(
                        source,
                        Some(head),
                        total,
                        validated_payload_bytes,
                        pages,
                        Some("tde.page.decrypt"),
                    );
                }
            };
            if owned.decrypted_user.is_some() {
                decoded_bytes = match decoded_bytes.checked_add(crate::format::DB_PAGE_SIZE as u64)
                {
                    Some(value) if value <= policy.max_decoded_bytes => value,
                    _ => {
                        return self.publish_overflow_chain(
                            source,
                            Some(head),
                            total,
                            validated_payload_bytes,
                            pages,
                            Some("resource-limit"),
                        );
                    }
                };
            }
            let envelope = match owned.envelope(current) {
                Ok(value) => value,
                Err(error) => {
                    return self.publish_overflow_chain(
                        source,
                        Some(head),
                        total,
                        validated_payload_bytes,
                        pages,
                        Some(error.rule()),
                    );
                }
            };
            let is_head = pages.is_empty();
            let page = if is_head {
                match decode_overflow_head(&envelope) {
                    Ok(value) => value,
                    Err(error) => {
                        return self.publish_overflow_chain(
                            source,
                            Some(head),
                            total,
                            validated_payload_bytes,
                            pages,
                            Some(error.rule()),
                        );
                    }
                }
            } else {
                match decode_overflow_continuation(&envelope, remaining) {
                    Ok(value) => value,
                    Err(error) => {
                        return self.publish_overflow_chain(
                            source,
                            Some(head),
                            total,
                            validated_payload_bytes,
                            pages,
                            Some(error.rule()),
                        );
                    }
                }
            };
            if is_head {
                let Some(head_total) = page.total_length() else {
                    return self.publish_overflow_chain(
                        source,
                        Some(head),
                        total,
                        validated_payload_bytes,
                        pages,
                        Some("overflow.head.total_required"),
                    );
                };
                total = Some(head_total);
                remaining = head_total;
            }
            remaining = remaining
                .checked_sub(u32::from(page.payload_length()))
                .ok_or(OperationError::Arithmetic)?;
            validated_payload_bytes = validated_payload_bytes
                .checked_add(u64::from(page.payload_length()))
                .ok_or(OperationError::Arithmetic)?;
            let next = page.next();
            pages.push(OverflowPageView {
                vpid: current,
                head: is_head,
                next,
                payload_offset: page.payload_offset(),
                payload_length: page.payload_length(),
            });
            match next {
                Some(next) => current = next,
                None if remaining == 0 => break,
                None => {
                    return self.publish_overflow_chain(
                        source,
                        Some(head),
                        total,
                        validated_payload_bytes,
                        pages,
                        Some("overflow.chain.complete_length"),
                    );
                }
            }
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        self.publish_overflow_chain(
            source,
            Some(head),
            total,
            validated_payload_bytes,
            pages,
            None,
        )
    }

    #[must_use]
    pub fn overflow_chain(&self, source: Oid) -> Option<OverflowChainView> {
        self.data
            .overflow_chains
            .get(&source)
            .map(|fact| OverflowChainView {
                source,
                revision: self.data.revision,
                head: fact.head,
                total_data_length: fact.total_data_length,
                validated_payload_bytes: fact.validated_payload_bytes,
                complete: fact.complete,
                pages: fact.pages.clone(),
                diagnostic_rule: fact.diagnostic_rule,
            })
    }

    #[must_use]
    pub fn overflow_chains(&self) -> Vec<OverflowChainView> {
        self.data
            .overflow_chains
            .keys()
            .filter_map(|source| self.overflow_chain(*source))
            .collect()
    }

    /// Validate one explicitly selected OOS chain and publish its structural
    /// prefix as a new immutable revision. Payload bytes are never retained.
    #[allow(clippy::too_many_lines)]
    pub fn enrich_oos(
        &self,
        head: Oid,
        policy: ResourcePolicy,
        cancel: &CancelToken,
    ) -> Result<Self, OperationError> {
        if self.data.oos_chains.contains_key(&head) {
            return Ok(self.clone());
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        let mut visited = BTreeSet::new();
        let mut chunks = Vec::new();
        let mut current = head;
        let mut expected_total = None;
        let mut expected_index = 0_u32;
        let mut validated_payload_bytes = 0_u64;
        let mut decoded_bytes = 0_u64;
        loop {
            if cancel.is_cancelled() {
                return self.publish_oos_chain(
                    head,
                    expected_total,
                    validated_payload_bytes,
                    chunks,
                    Some("interrupted"),
                );
            }
            if u64::try_from(chunks.len()).unwrap_or(u64::MAX) >= policy.max_chain_steps {
                return self.publish_oos_chain(
                    head,
                    expected_total,
                    validated_payload_bytes,
                    chunks,
                    Some("resource-limit"),
                );
            }
            if !visited.insert(current) {
                return self.publish_oos_chain(
                    head,
                    expected_total,
                    validated_payload_bytes,
                    chunks,
                    Some("oos.chain.acyclic"),
                );
            }
            decoded_bytes = match decoded_bytes.checked_add(crate::format::IO_PAGE_SIZE as u64) {
                Some(value)
                    if value <= policy.max_decoded_bytes
                        && value <= policy.memory_limit.saturating_mul(2) =>
                {
                    value
                }
                _ => {
                    return self.publish_oos_chain(
                        head,
                        expected_total,
                        validated_payload_bytes,
                        chunks,
                        Some("resource-limit"),
                    );
                }
            };
            let Ok(page_view) = self.page(Vpid::new(current.vol_id, current.page_id)) else {
                return self.publish_oos_chain(
                    head,
                    expected_total,
                    validated_payload_bytes,
                    chunks,
                    Some("oos.chain.page_exists"),
                );
            };
            if page_view.page_type != Some(PageType::Oos)
                || page_view.availability != Availability::Available
            {
                return self.publish_oos_chain(
                    head,
                    expected_total,
                    validated_payload_bytes,
                    chunks,
                    Some("oos.chain.page_role"),
                );
            }
            let current_vpid = Vpid::new(current.vol_id, current.page_id);
            let owned = match OwnedInspectionPage::read(&self.data, current_vpid) {
                Ok(page) => page,
                Err(InspectionPageError::Source(error)) => {
                    return Err(OperationError::Source(error));
                }
                Err(InspectionPageError::Format(rule)) => {
                    return self.publish_oos_chain(
                        head,
                        expected_total,
                        validated_payload_bytes,
                        chunks,
                        Some(rule),
                    );
                }
                Err(InspectionPageError::EncryptedOpaque) => {
                    return self.publish_oos_chain(
                        head,
                        expected_total,
                        validated_payload_bytes,
                        chunks,
                        Some("oos.chain.encrypted"),
                    );
                }
                Err(InspectionPageError::Decrypt) => {
                    return self.publish_oos_chain(
                        head,
                        expected_total,
                        validated_payload_bytes,
                        chunks,
                        Some("tde.page.decrypt"),
                    );
                }
            };
            if owned.decrypted_user.is_some() {
                decoded_bytes = match decoded_bytes.checked_add(crate::format::DB_PAGE_SIZE as u64)
                {
                    Some(value) if value <= policy.max_decoded_bytes => value,
                    _ => {
                        return self.publish_oos_chain(
                            head,
                            expected_total,
                            validated_payload_bytes,
                            chunks,
                            Some("resource-limit"),
                        );
                    }
                };
            }
            let envelope = match owned.envelope(current_vpid) {
                Ok(value) => value,
                Err(error) => {
                    return self.publish_oos_chain(
                        head,
                        expected_total,
                        validated_payload_bytes,
                        chunks,
                        Some(error.rule()),
                    );
                }
            };
            let slotted = match decode_slotted_page(&envelope) {
                Ok(value) => value,
                Err(error) => {
                    return self.publish_oos_chain(
                        head,
                        expected_total,
                        validated_payload_bytes,
                        chunks,
                        Some(error.rule()),
                    );
                }
            };
            let slot_id =
                u16::try_from(current.slot_id.get()).map_err(|_| OperationError::Arithmetic)?;
            let chunk = match decode_oos_chunk(&envelope, &slotted, slot_id) {
                Ok(value) => value,
                Err(error) => {
                    return self.publish_oos_chain(
                        head,
                        expected_total,
                        validated_payload_bytes,
                        chunks,
                        Some(error.rule()),
                    );
                }
            };
            if chunk.chunk_index() != expected_index
                || expected_total.is_some_and(|total| total != chunk.total_data_length())
            {
                return self.publish_oos_chain(
                    head,
                    expected_total,
                    validated_payload_bytes,
                    chunks,
                    Some("oos.chain.sequence"),
                );
            }
            expected_total.get_or_insert(chunk.total_data_length());
            validated_payload_bytes =
                match validated_payload_bytes.checked_add(u64::from(chunk.payload_length())) {
                    Some(value)
                        if value <= u64::from(chunk.total_data_length())
                            && value <= policy.max_decoded_bytes =>
                    {
                        value
                    }
                    _ => {
                        return self.publish_oos_chain(
                            head,
                            expected_total,
                            validated_payload_bytes,
                            chunks,
                            Some("oos.chain.accumulated_length"),
                        );
                    }
                };
            let next = match chunk.next() {
                OosNext::Terminal => None,
                OosNext::Link(oid) => Some(oid),
            };
            chunks.push(OosChunkView {
                oid: current,
                total_data_length: chunk.total_data_length(),
                chunk_index: chunk.chunk_index(),
                next,
                payload_offset: chunk.payload_offset(),
                payload_length: chunk.payload_length(),
            });
            match next {
                None if validated_payload_bytes == u64::from(chunk.total_data_length()) => break,
                None => {
                    return self.publish_oos_chain(
                        head,
                        expected_total,
                        validated_payload_bytes,
                        chunks,
                        Some("oos.chain.complete_length"),
                    );
                }
                Some(oid) => current = oid,
            }
            expected_index = expected_index
                .checked_add(1)
                .ok_or(OperationError::Arithmetic)?;
        }
        if let Some(stopped) = self.source_stability_stop()? {
            return Ok(stopped);
        }
        self.publish_oos_chain(head, expected_total, validated_payload_bytes, chunks, None)
    }

    #[must_use]
    pub fn oos_chain(&self, head: Oid) -> Option<OosChainView> {
        self.data.oos_chains.get(&head).map(|fact| OosChainView {
            head,
            revision: self.data.revision,
            total_data_length: fact.total_data_length,
            validated_payload_bytes: fact.validated_payload_bytes,
            complete: fact.complete,
            chunks: fact.chunks.clone(),
            diagnostic_rule: fact.diagnostic_rule,
        })
    }

    #[must_use]
    pub fn oos_chains(&self) -> Vec<OosChainView> {
        self.data
            .oos_chains
            .keys()
            .filter_map(|head| self.oos_chain(*head))
            .collect()
    }

    fn publish_oos_chain(
        &self,
        head: Oid,
        total_data_length: Option<u32>,
        validated_payload_bytes: u64,
        chunks: Vec<OosChunkView>,
        failure: Option<&'static str>,
    ) -> Result<Self, OperationError> {
        let mut next = (*self.data).clone();
        next.revision = next
            .revision
            .next()
            .map_err(|_| OperationError::Arithmetic)?;
        next.oos_chains.insert(
            head,
            OosChainFact {
                total_data_length,
                validated_payload_bytes,
                complete: failure.is_none(),
                chunks,
                diagnostic_rule: failure,
            },
        );
        if let Some(rule) = failure {
            let resource = matches!(rule, "resource-limit" | "interrupted");
            next.diagnostics.push(DiagnosticRecord {
                code: if resource {
                    "inspection.resource_limit"
                } else {
                    "oos.chain.invalid"
                },
                severity: "error",
                message: if resource {
                    "The OOS traversal stopped at its admitted resource boundary."
                } else {
                    "The selected OOS chain violates its pinned structural format."
                },
                subject: format!(
                    "oos:{}:{}:{}",
                    head.vol_id.get(),
                    head.page_id.get(),
                    head.slot_id.get()
                ),
                rule,
            });
        }
        refresh_oos_coverage(&mut next, failure);
        next.outcome = classify_session_outcome(&next);
        Ok(Self {
            data: Arc::new(next),
        })
    }

    fn publish_overflow_chain(
        &self,
        source: Oid,
        head: Option<Vpid>,
        total_data_length: Option<u32>,
        validated_payload_bytes: u64,
        pages: Vec<OverflowPageView>,
        failure: Option<&'static str>,
    ) -> Result<Self, OperationError> {
        let mut next = (*self.data).clone();
        next.revision = next
            .revision
            .next()
            .map_err(|_| OperationError::Arithmetic)?;
        next.overflow_chains.insert(
            source,
            OverflowChainFact {
                head,
                total_data_length,
                validated_payload_bytes,
                complete: failure.is_none(),
                pages,
                diagnostic_rule: failure,
            },
        );
        if let Some(rule) = failure {
            let resource = matches!(rule, "resource-limit" | "interrupted");
            next.diagnostics.push(DiagnosticRecord {
                code: if resource {
                    "inspection.resource_limit"
                } else {
                    "overflow.chain.invalid"
                },
                severity: "error",
                message: if resource {
                    "The overflow traversal stopped at its admitted resource boundary."
                } else {
                    "The selected REC_BIGONE overflow chain violates its pinned structural format."
                },
                subject: format!(
                    "slot:{}:{}:{}",
                    source.vol_id.get(),
                    source.page_id.get(),
                    source.slot_id.get()
                ),
                rule,
            });
        }
        refresh_overflow_coverage(&mut next, failure);
        next.outcome = classify_session_outcome(&next);
        Ok(Self {
            data: Arc::new(next),
        })
    }

    fn publish_relocation_edge(
        &self,
        source: Oid,
        target: Option<Oid>,
        failure: Option<&'static str>,
    ) -> Result<Self, OperationError> {
        let mut next = (*self.data).clone();
        next.revision = next
            .revision
            .next()
            .map_err(|_| OperationError::Arithmetic)?;
        next.relocation_edges.insert(
            source,
            RelocationEdgeFact {
                target,
                valid: failure.is_none(),
                diagnostic_rule: failure,
            },
        );
        if let Some(rule) = failure {
            let resource = matches!(rule, "resource-limit" | "interrupted");
            next.diagnostics.push(DiagnosticRecord {
                code: if resource {
                    "inspection.resource_limit"
                } else {
                    "heap.relocation.invalid"
                },
                severity: "error",
                message: if resource {
                    "The relocation validation stopped at its admitted resource boundary."
                } else {
                    "The selected REC_RELOCATION edge violates its pinned structural format."
                },
                subject: format!(
                    "slot:{}:{}:{}",
                    source.vol_id.get(),
                    source.page_id.get(),
                    source.slot_id.get()
                ),
                rule,
            });
        }
        refresh_relocation_coverage(&mut next, failure);
        next.outcome = classify_session_outcome(&next);
        Ok(Self {
            data: Arc::new(next),
        })
    }

    fn page_decode_failure(&self, vpid: Vpid, rule: &'static str) -> Result<Self, OperationError> {
        let mut next = (*self.data).clone();
        next.revision = next
            .revision
            .next()
            .map_err(|_| OperationError::Arithmetic)?;
        next.deep_pages.insert(
            vpid,
            DeepPageFact {
                slotted: None,
                file_header: None,
                raw: None,
                diagnostic_rule: Some(rule),
            },
        );
        let original = next
            .volumes
            .iter()
            .find(|volume| volume.view.vol_id == vpid.vol_id)
            .ok_or(OperationError::FactStore)?
            .page_fact(vpid.page_id)
            .map_err(|_| OperationError::FactStore)?
            .ok_or(OperationError::FactStore)?;
        let decrypted = original.tde_state == TdeInspectionState::Decrypted;
        if decrypted {
            next.page_overrides.insert(
                vpid,
                PageFastFact {
                    tde_state: TdeInspectionState::DecryptedInvalid,
                    diagnostic_code: Some("tde.decrypted_invalid"),
                    ..original
                },
            );
        }
        next.diagnostics.push(DiagnosticRecord {
            code: if decrypted {
                "tde.decrypted_invalid"
            } else {
                "page.body.invalid"
            },
            severity: "error",
            message: if decrypted {
                "Decrypted page structure is invalid."
            } else {
                "The requested page body violates its pinned structural format."
            },
            subject: format!("page:{}:{}", vpid.vol_id.get(), vpid.page_id.get()),
            rule,
        });
        refresh_deep_coverage(&mut next, Some("corrupt-structure"));
        next.outcome = classify_session_outcome(&next);
        Ok(Self {
            data: Arc::new(next),
        })
    }

    fn publish_deep_page(&self, vpid: Vpid, fact: DeepPageFact) -> Result<Self, OperationError> {
        if let Some(existing) = self.data.deep_pages.get(&vpid) {
            // An explicit file selector may refine an earlier envelope-only
            // PAGE_FTAB result. All other repeated enrichments are idempotent.
            if fact.file_header.is_none() || existing.file_header.is_some() {
                return Ok(self.clone());
            }
        }
        let mut next = (*self.data).clone();
        next.revision = next
            .revision
            .next()
            .map_err(|_| OperationError::Arithmetic)?;
        next.deep_pages.insert(vpid, fact);
        refresh_deep_coverage(&mut next, None);
        next.outcome = classify_session_outcome(&next);
        Ok(Self {
            data: Arc::new(next),
        })
    }

    fn publish_file(
        &self,
        header_page: Vpid,
        header: FileHeader,
        facts: FileAllocationFacts,
        cancel: &CancelToken,
    ) -> Result<Self, OperationError> {
        if facts.pages.iter().any(|vpid| {
            self.data
                .file_allocations
                .get(vpid)
                .is_some_and(|owner| *owner != header.vfid())
        }) {
            return self.page_decode_failure(header_page, "file.table.owner_unique");
        }
        let mut single = BTreeMap::new();
        single.insert(header.vfid(), header);
        let class_names = self.resolve_tracked_class_names(&single, cancel)?;
        let mut next = (*self.data).clone();
        next.revision = next
            .revision
            .next()
            .map_err(|_| OperationError::Arithmetic)?;
        next.deep_pages.insert(
            header_page,
            DeepPageFact {
                slotted: None,
                file_header: Some(header),
                raw: None,
                diagnostic_rule: None,
            },
        );
        for vpid in facts.pages {
            next.file_allocations.insert(vpid, header.vfid());
        }
        for claims in next.sector_claims.values_mut() {
            claims.retain(|claim| claim.vfid != header.vfid());
        }
        next.sector_claims.retain(|_, claims| !claims.is_empty());
        for (key, kind) in facts.claims {
            next.sector_claims
                .entry(key)
                .or_default()
                .push(SectorClaimView {
                    vfid: header.vfid(),
                    kind,
                });
        }
        next.class_names.extend(class_names);
        refresh_deep_coverage(&mut next, None);
        let inspected_files = next
            .deep_pages
            .values()
            .filter(|fact| fact.file_header.is_some())
            .count() as u64;
        next.coverage
            .retain(|coverage| coverage.facet != "file-inventory");
        next.coverage.push(CoverageRecord {
            facet: "file-inventory",
            coverage: Coverage::Partial,
            evaluated: inspected_files,
            conclusive: inspected_files,
            trusted_total: None,
            stop_reason: Some("selective-enrichment"),
        });
        next.outcome = classify_session_outcome(&next);
        Ok(Self {
            data: Arc::new(next),
        })
    }

    /// Checks source stability before an enrichment.
    ///
    /// Under the immutable contract a changed input ends the session at an
    /// invalidated revision, returned here for the caller to publish. Under
    /// live follow the read proceeds: the watcher re-reads the input, and the
    /// live session labels the generation superseded from the outside, so a
    /// deep read is never refused just because the database is in use.
    fn source_stability_stop(&self) -> Result<Option<Self>, OperationError> {
        if self
            .data
            .sources
            .verify_unchanged()
            .map_err(OperationError::Source)?
        {
            return Ok(None);
        }
        match self.data.source_mode {
            SourceMode::Immutable => self.invalidated_revision().map(Some),
            SourceMode::Live => Ok(None),
        }
    }

    fn invalidated_revision(&self) -> Result<Self, OperationError> {
        if self.data.validity == SnapshotValidity::Invalidated {
            return Ok(self.clone());
        }
        let mut next = (*self.data).clone();
        next.revision = next
            .revision
            .next()
            .map_err(|_| OperationError::Arithmetic)?;
        next.validity = SnapshotValidity::Invalidated;
        next.diagnostics.push(DiagnosticRecord {
            code: "snapshot.modified",
            severity: "fatal",
            message: "An input changed after this snapshot was published.",
            subject: "snapshot".to_owned(),
            rule: "snapshot.file_stamp.stable",
        });
        next.outcome = InspectionOutcome::Fatal;
        Ok(Self {
            data: Arc::new(next),
        })
    }

    fn volume_record(&self, vol_id: VolId) -> Result<&VolumeRecord, QueryError> {
        self.data
            .volumes
            .binary_search_by_key(&vol_id.get(), |volume| volume.view.vol_id.get())
            .ok()
            .and_then(|index| self.data.volumes.get(index))
            .ok_or(QueryError::EntityNotFound)
    }

    fn page_from_record(
        &self,
        volume: &VolumeRecord,
        page_id: PageId,
    ) -> Result<PageView, QueryError> {
        let raw_page = u32::try_from(page_id.get()).map_err(|_| QueryError::EntityNotFound)?;
        let raw_sector = raw_page / SECTOR_PAGES;
        let sector_id =
            SectorId::new(i32::try_from(raw_sector).map_err(|_| QueryError::Arithmetic)?)
                .map_err(|_| QueryError::Arithmetic)?;
        let vpid = Vpid::new(volume.view.vol_id, page_id);
        let allocation = if page_id.get() <= volume.view.system_last_page.get() {
            PageAllocationClass::SystemMetadata
        } else if self.data.file_allocations.contains_key(&vpid) {
            PageAllocationClass::Allocated
        } else if volume.is_reserved(raw_sector) {
            PageAllocationClass::ReservedUnallocated
        } else {
            PageAllocationClass::Unreserved
        };
        let fact = if let Some(override_fact) = self.data.page_overrides.get(&vpid) {
            Some(*override_fact)
        } else {
            volume
                .page_fact(page_id)
                .map_err(|_| QueryError::FactStore)?
        };
        Ok(PageView {
            vpid,
            sector_id,
            allocation,
            page_type: fact.and_then(|value| value.page_type),
            availability: fact.map_or(Availability::Unsupported, |value| value.availability),
            tde_state: fact.map_or(TdeInspectionState::NotEncrypted, |value| value.tde_state),
            detail_support: fact.and_then(|value| value.page_type.map(page_detail_support)),
            slotted_occupied_percent: fact
                .and_then(|value| value.slotted_occupancy_units)
                .map(slotted_occupied_percent),
            lsa_word: fact.and_then(|value| value.lsa_word),
            diagnostic_code: fact.and_then(|value| value.diagnostic_code),
            file_association: self.page_file_association(vpid, sector_id),
        })
    }
}

/// Typed reason a file type carries no single class association.
const fn class_absence_reason(file_type: FileType) -> &'static str {
    match file_type {
        FileType::Oos => "OOS class attribution is intentionally deferred",
        FileType::Heap
        | FileType::HeapReuseSlots
        | FileType::MultipageObjectHeap
        | FileType::Btree
        | FileType::BtreeOverflowKey
        | FileType::ExtensibleHash
        | FileType::HashDirectory => "file descriptor has a null class OID",
        FileType::Tracker
        | FileType::Catalog
        | FileType::DroppedFiles
        | FileType::VacuumData
        | FileType::QueryArea
        | FileType::Temporary
        | FileType::Unknown => "file type has no single class association",
    }
}

fn database_charset(data: &SessionData) -> Result<u8, &'static str> {
    let source = data
        .sources
        .volumes()
        .iter()
        .find(|volume| volume.declared_id().get() == 0)
        .ok_or("primary volume is unavailable")?;
    let page_id = PageId::new(0).map_err(|_| "primary volume header page is invalid")?;
    let bytes = source
        .read_page(page_id)
        .map_err(|_| "primary volume header could not be read")?;
    let envelope = decode_page_envelope(bytes.as_slice(), Vpid::new(source.declared_id(), page_id))
        .map_err(|_| "primary volume header envelope is invalid")?;
    let header = decode_volume_header(&envelope, source.stamp().length)
        .map_err(|_| "primary volume header is invalid")?;
    Ok(header.database_charset())
}

fn decode_class_name(
    envelope: &crate::format::DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    slot_id: u16,
    charset: u8,
) -> Result<Arc<str>, &'static str> {
    let slot = slotted
        .slots()
        .get(usize::from(slot_id))
        .ok_or("class record slot does not exist")?;
    let record = decode_heap_record_envelope(envelope, slotted, slot_id, false)
        .map_err(|_| "class object header is invalid")?;
    let view = envelope
        .plaintext("class.record.encrypted")
        .map_err(|_| "class record is encrypted and unavailable")?;
    let record_start = usize::from(slot.offset());
    let header_length = usize::from(record.header_length);
    let offset_width = usize::from(record.variable_offset_width);
    let table_start = record_start
        .checked_add(header_length)
        .ok_or("class variable table offset overflow")?;
    let first_raw = read_class_var_offset(&view, table_start, offset_width)?;
    let next_entry = table_start
        .checked_add(offset_width)
        .ok_or("class variable table offset overflow")?;
    let second_raw = read_class_var_offset(&view, next_entry, offset_width)?;
    if first_raw & 1 != 0 {
        return Err("out-of-row class names are outside this POC");
    }
    let first =
        usize::try_from(first_raw & !3).map_err(|_| "class name offset does not fit in memory")?;
    let second = usize::try_from(second_raw & !3)
        .map_err(|_| "class name end offset does not fit in memory")?;
    if first >= second {
        return Err("class name variable attribute is empty or inverted");
    }
    let variable_start = record_start
        .checked_add(header_length)
        .and_then(|base| base.checked_add(first))
        .ok_or("class name offset overflow")?;
    let variable_length = second
        .checked_sub(first)
        .ok_or("class name length underflow")?;
    let record_end = record_start
        .checked_add(usize::from(slot.length()))
        .ok_or("class record length overflow")?;
    let variable_end = variable_start
        .checked_add(variable_length)
        .ok_or("class name length overflow")?;
    if variable_end > record_end {
        return Err("class name exceeds its record slot");
    }
    let variable = view
        .range(variable_start, variable_length, "class.record.name")
        .map_err(|_| "class name bytes are out of bounds")?;
    let (name_start, name_length) = class_varchar_shape(variable)?;
    let name_end = name_start
        .checked_add(name_length)
        .ok_or("class name length overflow")?;
    if name_length > 255 || name_end >= variable.len() || variable[name_end] != 0 {
        return Err("class name length or terminator is invalid");
    }
    decode_class_identifier(&variable[name_start..name_end], charset)
}

fn read_class_var_offset(
    view: &ByteView<'_>,
    offset: usize,
    width: usize,
) -> Result<u32, &'static str> {
    let bytes = view
        .range(offset, width, "class.record.var_offset")
        .map_err(|_| "class variable offset is out of bounds")?;
    match bytes {
        [value] => Ok(u32::from(*value)),
        [first, second] => Ok(u32::from(u16::from_be_bytes([*first, *second]))),
        [first, second, third, fourth] => {
            Ok(u32::from_be_bytes([*first, *second, *third, *fourth]))
        }
        _ => Err("class variable offset width is unsupported"),
    }
}

fn class_varchar_shape(variable: &[u8]) -> Result<(usize, usize), &'static str> {
    let prefix = *variable.first().ok_or("class name VARCHAR is empty")?;
    if prefix != 0xff {
        return Ok((1, usize::from(prefix)));
    }
    let compressed = variable
        .get(1..5)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_be_bytes)
        .ok_or("class name compression header is truncated")?;
    let decompressed = variable
        .get(5..9)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_be_bytes)
        .ok_or("class name compression header is truncated")?;
    if compressed != 0 {
        return Err("compressed class names are not yet decoded");
    }
    Ok((
        9,
        usize::try_from(decompressed).map_err(|_| "class name length does not fit in memory")?,
    ))
}

fn decode_class_identifier(bytes: &[u8], charset: u8) -> Result<Arc<str>, &'static str> {
    if bytes.is_ascii() {
        return std::str::from_utf8(bytes)
            .map(Arc::from)
            .map_err(|_| "ASCII class name is invalid");
    }
    match charset {
        3 => Ok(bytes
            .iter()
            .copied()
            .map(char::from)
            .collect::<String>()
            .into()),
        5 => std::str::from_utf8(bytes)
            .map(Arc::from)
            .map_err(|_| "UTF-8 class name is invalid"),
        4 => Err("EUC-KR class names are not yet decoded"),
        0 => Err("ASCII database contains a non-ASCII class name"),
        _ => Err("database codeset is unsupported"),
    }
}

#[derive(Debug)]
pub enum OpenFailure {
    Source(SourceError),
    Format(DecodeError),
    Tde(TdeError),
    Spill,
    FactStore,
    Worker,
    TdeBootstrap,
    Interrupted,
    Arithmetic,
}

impl From<SourceError> for OpenFailure {
    fn from(value: SourceError) -> Self {
        Self::Source(value)
    }
}

impl fmt::Display for OpenFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "{error}"),
            Self::Format(error) => write!(formatter, "{error}"),
            Self::Tde(error) => write!(formatter, "{error}"),
            Self::Spill => formatter.write_str("private spill storage could not be created"),
            Self::FactStore => formatter.write_str("packed page facts could not be stored"),
            Self::Worker => formatter.write_str("page-envelope worker failed"),
            Self::TdeBootstrap => formatter.write_str("TDE key bootstrap failed"),
            Self::Interrupted => formatter.write_str("inspection interrupted before publication"),
            Self::Arithmetic => formatter.write_str("inspection arithmetic overflow"),
        }
    }
}

impl std::error::Error for OpenFailure {}

#[derive(Debug)]
pub enum OperationError {
    RevisionNotFound,
    Source(SourceError),
    Query(QueryError),
    Interrupted,
    Unsupported,
    Structural(String),
    ResourceLimit,
    FactStore,
    Arithmetic,
}

impl fmt::Display for OperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RevisionNotFound => formatter.write_str("inspection revision not found"),
            Self::Source(error) => write!(formatter, "{error}"),
            Self::Query(error) => write!(formatter, "{error}"),
            Self::Interrupted => formatter.write_str("inspection enrichment interrupted"),
            Self::Unsupported => formatter.write_str("page body is unavailable for enrichment"),
            Self::Structural(rule) => write!(formatter, "structural validation failed: {rule}"),
            Self::ResourceLimit => formatter.write_str("page enrichment exceeds resource policy"),
            Self::FactStore => formatter.write_str("packed page fact storage is unavailable"),
            Self::Arithmetic => formatter.write_str("inspection enrichment arithmetic overflow"),
        }
    }
}

impl std::error::Error for OperationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryError {
    EntityNotFound,
    FactStore,
    Arithmetic,
}

impl fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "query failed: {self:?}")
    }
}

impl std::error::Error for QueryError {}

fn refresh_deep_coverage(data: &mut SessionData, failure: Option<&'static str>) {
    data.coverage
        .retain(|coverage| coverage.facet != "deep-pages");
    let conclusive = data
        .deep_pages
        .values()
        .filter(|fact| fact.diagnostic_rule.is_none())
        .count() as u64;
    let total = data
        .volumes
        .iter()
        .map(|volume| u64::from(volume.view.total_sectors) * u64::from(SECTOR_PAGES))
        .sum();
    let complete = data.deep_pages.len() as u64 == total;
    data.coverage.push(CoverageRecord {
        facet: "deep-pages",
        coverage: if complete {
            Coverage::Complete
        } else {
            Coverage::Partial
        },
        evaluated: data.deep_pages.len() as u64,
        conclusive,
        trusted_total: Some(total),
        stop_reason: if complete {
            failure
        } else {
            failure.or(Some("selective-enrichment"))
        },
    });
}

fn refresh_oos_coverage(data: &mut SessionData, failure: Option<&'static str>) {
    data.coverage
        .retain(|coverage| coverage.facet != "oos-chains");
    let total = data.oos_chains.len() as u64;
    let conclusive = data
        .oos_chains
        .values()
        .filter(|chain| chain.complete)
        .count() as u64;
    data.coverage.push(CoverageRecord {
        facet: "oos-chains",
        coverage: if conclusive == total {
            Coverage::Complete
        } else {
            Coverage::Partial
        },
        evaluated: total,
        conclusive,
        trusted_total: Some(total),
        stop_reason: failure,
    });
}

fn refresh_overflow_coverage(data: &mut SessionData, failure: Option<&'static str>) {
    data.coverage
        .retain(|coverage| coverage.facet != "overflow-chains");
    let total = data.overflow_chains.len() as u64;
    let conclusive = data
        .overflow_chains
        .values()
        .filter(|chain| chain.complete)
        .count() as u64;
    data.coverage.push(CoverageRecord {
        facet: "overflow-chains",
        coverage: if conclusive == total {
            Coverage::Complete
        } else {
            Coverage::Partial
        },
        evaluated: total,
        conclusive,
        trusted_total: Some(total),
        stop_reason: failure,
    });
}

fn refresh_relocation_coverage(data: &mut SessionData, failure: Option<&'static str>) {
    data.coverage
        .retain(|coverage| coverage.facet != "relocation-edges");
    let total = data.relocation_edges.len() as u64;
    let conclusive = data
        .relocation_edges
        .values()
        .filter(|edge| edge.valid)
        .count() as u64;
    data.coverage.push(CoverageRecord {
        facet: "relocation-edges",
        coverage: if conclusive == total {
            Coverage::Complete
        } else {
            Coverage::Partial
        },
        evaluated: total,
        conclusive,
        trusted_total: Some(total),
        stop_reason: failure,
    });
}

fn refresh_interpretation_coverage(data: &mut SessionData, failure: Option<&'static str>) {
    data.coverage
        .retain(|coverage| coverage.facet != "record-interpretations");
    let total = data.record_interpretations.len() as u64;
    let conclusive = data
        .record_interpretations
        .values()
        .filter(|evidence| evidence.diagnostic_rule.is_none())
        .count() as u64;
    data.coverage.push(CoverageRecord {
        facet: "record-interpretations",
        coverage: if conclusive == total {
            Coverage::Complete
        } else {
            Coverage::Partial
        },
        evaluated: total,
        conclusive,
        trusted_total: Some(total),
        stop_reason: failure,
    });
    data.coverage
        .retain(|coverage| coverage.facet != "class-representations");
    let classes = data.class_representations.len() as u64;
    let resolved = data
        .class_representations
        .values()
        .filter(|evidence| evidence.representation.is_some())
        .count() as u64;
    data.coverage.push(CoverageRecord {
        facet: "class-representations",
        coverage: if resolved == classes {
            Coverage::Complete
        } else {
            Coverage::Partial
        },
        evaluated: classes,
        conclusive: resolved,
        trusted_total: Some(classes),
        stop_reason: failure,
    });
}

fn classify_session_outcome(data: &SessionData) -> InspectionOutcome {
    let unexpectedly_incomplete = data.coverage.iter().any(|coverage| {
        coverage.coverage == Coverage::Partial
            && matches!(
                coverage.stop_reason,
                Some("resource-limit" | "interrupted" | "unreadable")
            )
    });
    InspectionOutcome::classify(OutcomeInputs {
        fatal: data.validity == SnapshotValidity::Invalidated,
        unexpected_incomplete: unexpectedly_incomplete,
        has_error_findings: data
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error"),
        expected_limitations: true,
    })
}

fn report(
    observer: &mut Option<&mut dyn ProgressObserver>,
    phase: ScanPhase,
    completed: u64,
    trusted_total: Option<u64>,
) {
    if let Some(observer) = observer.as_deref_mut() {
        observer.update(ScanProgress {
            phase,
            completed,
            trusted_total,
        });
    }
}

fn estimate_base_bytes(volumes: &[VolumeRecord]) -> Result<u64, OpenFailure> {
    volumes.iter().try_fold(0_u64, |total, volume| {
        let mask_bytes = u64::try_from(volume.reserved_masks.len())
            .ok()
            .and_then(|count| count.checked_mul(size_of::<u64>() as u64))
            .ok_or(OpenFailure::Arithmetic)?;
        total
            .checked_add(mask_bytes)
            .and_then(|value| value.checked_add(size_of::<VolumeRecord>() as u64))
            .ok_or(OpenFailure::Arithmetic)
    })
}

fn eligible_page_count(volume: &VolumeRecord) -> Result<u64, OpenFailure> {
    let system_last =
        u32::try_from(volume.view.system_last_page.get()).map_err(|_| OpenFailure::Arithmetic)?;
    (0..volume.view.total_sectors).try_fold(0_u64, |count, sector| {
        let first_page = sector
            .checked_mul(SECTOR_PAGES)
            .ok_or(OpenFailure::Arithmetic)?;
        if volume.is_reserved(sector) || first_page <= system_last {
            count
                .checked_add(u64::from(SECTOR_PAGES))
                .ok_or(OpenFailure::Arithmetic)
        } else {
            Ok(count)
        }
    })
}

fn page_detail_support(page_type: PageType) -> PageDetailSupport {
    match page_type {
        PageType::ExtensibleHash | PageType::Btree | PageType::Catalog => {
            PageDetailSupport::StructuralOnly
        }
        PageType::Unknown | PageType::QueryResult | PageType::Area | PageType::Log => {
            PageDetailSupport::Opaque
        }
        PageType::FileTable
        | PageType::Heap
        | PageType::VolumeHeader
        | PageType::VolumeBitmap
        | PageType::Overflow
        | PageType::Oos
        | PageType::DroppedFiles
        | PageType::VacuumData => PageDetailSupport::Semantic,
    }
}

fn page_uses_slotted_layout(
    page_type: PageType,
    owner_file_type: Option<crate::format::FileType>,
) -> bool {
    match page_type {
        // PAGE_EHASH is shared by raw directory pages and slotted bucket
        // pages. Only tracker-proven bucket-file ownership resolves the role.
        PageType::ExtensibleHash => {
            owner_file_type == Some(crate::format::FileType::ExtensibleHash)
        }
        PageType::Heap | PageType::Oos | PageType::Btree | PageType::Catalog => true,
        _ => false,
    }
}

fn page_diagnostic_code(error: &DecodeError) -> &'static str {
    match error.rule() {
        "page.envelope.identity_match" => "page.envelope.identity_mismatch",
        "page.envelope.lsa_match" => "page.envelope.lsa_mismatch",
        "page.envelope.type_known" => "page.envelope.type_unknown",
        "page.envelope.tde_flags" => "page.envelope.tde_flags_invalid",
        _ => "page.envelope.invalid",
    }
}

fn page_type_from_ordinal(ordinal: u8) -> Option<PageType> {
    match ordinal {
        0 => Some(PageType::Unknown),
        1 => Some(PageType::FileTable),
        2 => Some(PageType::Heap),
        3 => Some(PageType::VolumeHeader),
        4 => Some(PageType::VolumeBitmap),
        5 => Some(PageType::QueryResult),
        6 => Some(PageType::ExtensibleHash),
        7 => Some(PageType::Overflow),
        8 => Some(PageType::Oos),
        9 => Some(PageType::Area),
        10 => Some(PageType::Catalog),
        11 => Some(PageType::Btree),
        12 => Some(PageType::Log),
        13 => Some(PageType::DroppedFiles),
        14 => Some(PageType::VacuumData),
        _ => None,
    }
}

#[must_use]
pub const fn tde_algorithm_name(algorithm: TdeAlgorithm) -> &'static str {
    match algorithm {
        TdeAlgorithm::Aes => "aes",
        TdeAlgorithm::Aria => "aria",
    }
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::{
        PACKED_PAGE_FACT_SIZE_USIZE, PackedPageFact, PageFastFact, SourceMode,
        classify_mid_scan_source_change, page_uses_slotted_layout,
    };
    use crate::format::{FileType, PageType};
    use crate::model::{Availability, PageId, SnapshotValidity, TdeInspectionState};

    #[test]
    fn a_mid_scan_change_is_fatal_offline_but_torn_when_following() {
        let (validity, diagnostic) = classify_mid_scan_source_change(SourceMode::Immutable);
        assert_eq!(validity, SnapshotValidity::Invalidated);
        assert_eq!(diagnostic.code, "snapshot.modified");
        assert_eq!(diagnostic.severity, "fatal");

        let (validity, diagnostic) = classify_mid_scan_source_change(SourceMode::Live);
        assert_eq!(validity, SnapshotValidity::Torn);
        assert_eq!(diagnostic.code, "snapshot.torn_read");
        assert_eq!(diagnostic.severity, "warning");
    }

    #[test]
    fn packed_page_fact_is_exact_and_round_trips_canonical_fields() {
        assert_eq!(size_of::<PackedPageFact>(), PACKED_PAGE_FACT_SIZE_USIZE);
        let fact = PageFastFact {
            page_id: PageId::new(42).unwrap(),
            page_type: Some(PageType::Heap),
            availability: Availability::Available,
            tde_state: TdeInspectionState::Decrypted,
            slotted_occupancy_units: Some(8),
            lsa_word: Some(0x0102_0304_0506_0708),
            diagnostic_code: Some("page.envelope.lsa_mismatch"),
        };

        assert_eq!(PackedPageFact::pack(fact).unpack().unwrap(), fact);
    }

    #[test]
    fn ehash_slotted_layout_requires_bucket_file_ownership() {
        assert!(page_uses_slotted_layout(
            PageType::ExtensibleHash,
            Some(FileType::ExtensibleHash)
        ));
        assert!(!page_uses_slotted_layout(
            PageType::ExtensibleHash,
            Some(FileType::HashDirectory)
        ));
        assert!(!page_uses_slotted_layout(PageType::ExtensibleHash, None));
        assert!(page_uses_slotted_layout(PageType::Heap, None));
    }
}
