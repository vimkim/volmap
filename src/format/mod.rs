//! Decoders for pinned CUBRID persistent-format profiles.
//!
//! Format authorities: CUBRID `develop` commit
//! `cd593bcf2d8643b4698f1cb311c4c23af23a9d57` and `feat/oos` commit
//! `e1e651debf6cc100172bde96603b17424f9c135a`. The decoder translates explicit
//! persistent layouts; it does not reproduce a C/C++ memory layout.

mod boot;
mod btree;
mod catalog;
mod class_name;
mod classrep;
mod file_table;
mod heap;
mod object_layout;
mod overflow;
mod page;
mod record_interpretation;
mod sector;
mod slotted;
mod vacuum;
mod volume;

use core::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatProfile {
    Develop,
    FeatOos,
}

impl FormatProfile {
    #[must_use]
    pub const fn alternative(self) -> Self {
        match self {
            Self::Develop => Self::FeatOos,
            Self::FeatOos => Self::Develop,
        }
    }

    #[must_use]
    pub const fn cli_name(self) -> &'static str {
        match self {
            Self::Develop => "develop",
            Self::FeatOos => "feat-oos",
        }
    }

    #[must_use]
    pub const fn authority_id(self) -> &'static str {
        match self {
            Self::Develop => "cubrid-develop-linux-x86_64-gcc-cd593bc",
            Self::FeatOos => "cubrid-feat-oos-linux-x86_64-gcc-e1e651de",
        }
    }

    #[must_use]
    pub const fn retry_hint_message(self) -> &'static str {
        match self {
            Self::Develop => "Format mismatch: retry with --format-profile develop.",
            Self::FeatOos => "Format mismatch: retry with --format-profile feat-oos.",
        }
    }

    #[must_use]
    pub const fn supports_oos(self) -> bool {
        matches!(self, Self::FeatOos)
    }
}

pub use page::{
    DB_PAGE_SIZE, DecodedPageEnvelope, IO_PAGE_SIZE, PAGE_PREFIX_SIZE, PAGE_WATERMARK_SIZE,
    PageContent, PageEnvelopeSummary, PageType, TdeAlgorithm, decode_decrypted_page_envelope,
    decode_decrypted_page_envelope_with_profile, decode_page_envelope, decode_page_envelope_parts,
    decode_page_envelope_parts_with_profile, decode_page_envelope_with_profile,
};
pub use sector::{SectorBitmap, decode_sector_bitmap};
pub use slotted::{
    AnchorType, OosChunkFact, OosNext, RecordType, SLOTTED_HEADER_SIZE, SLOTTED_SLOT_SIZE,
    SlotFact, SlottedPage, decode_oos_chunk, decode_slotted_free_space_header, decode_slotted_page,
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
pub use boot::{BOOT_DB_PARM_SIZE, BootDbParmFact, decode_boot_db_parm};
pub(crate) use boot::{TDE_KEY_INFO_RECORD_SIZE, copy_special_heap_record};
pub use btree::{
    BtreeNodeFact, BtreeOidOverflowFact, BtreePageFact, BtreeRootFact, decode_btree_page,
};
pub use catalog::{
    CatalogClassInfoFact, CatalogDirectoryFact, CatalogPageFact, CatalogRepresentationHeaderFact,
    CatalogRepresentationItemFact, decode_catalog_class_info, decode_catalog_directory,
    decode_catalog_page, decode_catalog_representation_header,
};
pub use class_name::decode_class_record_name;
pub use classrep::{
    AttributeDomainFact, ClassAttributeFact, ClassRepresentationFact, DbType, RepresentationTarget,
    decode_class_representation,
};
pub use file_table::{
    ExtDataHeader, FILE_DESCRIPTOR_SIZE, FileClassAssociation, FileDescriptor, FileHeader,
    FileType, PartialSectorFact, TrackerItemFact, UserPageFact, decode_extdata_header,
    decode_file_descriptor, decode_file_header, decode_full_sectors, decode_partial_sectors,
    decode_tracker_items, decode_user_pages,
};
pub use heap::{
    HeapChainFact, HeapHeaderFact, HeapObjectHeaderFact, HeapPageFact, HeapRecordEnvelopeFact,
    decode_bigone_target, decode_heap_object_body, decode_heap_page, decode_heap_record_body,
    decode_heap_record_envelope, decode_relocation_target,
};
pub use overflow::{OverflowPageFact, decode_overflow_continuation, decode_overflow_head};
pub use record_interpretation::{
    AttributeExtent, AttributeInterpretation, AttributeValue, CalendarDate, ClockTime,
    InterpretedAttribute, InterpretedRecord, RecordLayoutFact, decode_record_interpretation,
};
