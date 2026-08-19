//! Decoders for the pinned CUBRID `feat/oos` profile.
//!
//! Format authority: CUBRID commit
//! `e1e651debf6cc100172bde96603b17424f9c135a`. The decoder translates explicit
//! offsets from `src/storage/file_io.h`, `storage_common.h`, and
//! `disk_manager.c`; it does not reproduce a C/C++ memory layout.

mod file_table;
mod heap;
mod page;
mod sector;
mod slotted;
mod vacuum;
mod volume;

use core::fmt;

pub use page::{
    DB_PAGE_SIZE, DecodedPageEnvelope, IO_PAGE_SIZE, PAGE_PREFIX_SIZE, PAGE_WATERMARK_SIZE,
    PageContent, PageEnvelopeSummary, PageType, TdeAlgorithm, decode_page_envelope,
    decode_page_envelope_parts,
};
pub use sector::{SectorBitmap, decode_sector_bitmap};
pub use slotted::{
    AnchorType, OosChunkFact, OosNext, RecordType, SlotFact, SlottedPage, decode_oos_chunk,
    decode_slotted_page,
};
pub use vacuum::{
    DroppedFileFact, DroppedFilesPageFact, VacuumEntryFact, VacuumPageFact,
    decode_dropped_files_page, decode_vacuum_page,
};
pub use volume::{ValidatedCString, VolumeHeader, VolumePurpose, VolumeType, decode_volume_header};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeErrorKind {
    ByteAccess,
    InvalidLength,
    NegativeValue,
    ArithmeticOverflow,
    IdentityMismatch,
    LsaMismatch,
    UnknownEnum,
    InvalidFlags,
    WrongPageType,
    InvalidMagic,
    InvalidGeometry,
    InvalidStringTable,
    FileLengthInvalid,
    EncryptedOpaque,
    OutOfRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodeError {
    kind: DecodeErrorKind,
    rule: &'static str,
    offset: Option<u64>,
}

impl DecodeError {
    pub(crate) const fn new(kind: DecodeErrorKind, rule: &'static str) -> Self {
        Self {
            kind,
            rule,
            offset: None,
        }
    }

    pub(crate) const fn at(kind: DecodeErrorKind, rule: &'static str, offset: u64) -> Self {
        Self {
            kind,
            rule,
            offset: Some(offset),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> DecodeErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn rule(&self) -> &'static str {
        self.rule
    }

    #[must_use]
    pub const fn offset(&self) -> Option<u64> {
        self.offset
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {:?}", self.rule, self.kind)
    }
}

impl std::error::Error for DecodeError {}
pub use file_table::{
    ExtDataHeader, FileHeader, FileType, PartialSectorFact, TrackerItemFact, UserPageFact,
    decode_extdata_header, decode_file_header, decode_full_sectors, decode_partial_sectors,
    decode_tracker_items, decode_user_pages,
};
pub use heap::{HeapChainFact, HeapHeaderFact, HeapPageFact, decode_heap_page};
