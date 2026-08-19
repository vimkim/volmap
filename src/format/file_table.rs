use crate::bytes::ByteView;
use crate::model::{FileId, PageId, SectorId, Vfid, VolId, Vpid};

use super::{DB_PAGE_SIZE, DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType};

const FILE_HEADER_SIZE: usize = 216;
const EXTDATA_HEADER_SIZE: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileType {
    Tracker,
    Heap,
    HeapReuseSlots,
    MultipageObjectHeap,
    Btree,
    BtreeOverflowKey,
    ExtensibleHash,
    HashDirectory,
    Catalog,
    DroppedFiles,
    VacuumData,
    QueryArea,
    Temporary,
    Oos,
    Unknown,
}

impl FileType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tracker => "tracker",
            Self::Heap => "heap",
            Self::HeapReuseSlots => "heap-reuse-slots",
            Self::MultipageObjectHeap => "multipage-object-heap",
            Self::Btree => "btree",
            Self::BtreeOverflowKey => "btree-overflow-key",
            Self::ExtensibleHash => "extensible-hash",
            Self::HashDirectory => "hash-directory",
            Self::Catalog => "catalog",
            Self::DroppedFiles => "dropped-files",
            Self::VacuumData => "vacuum-data",
            Self::QueryArea => "query-area",
            Self::Temporary => "temporary",
            Self::Oos => "oos",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileHeader {
    vfid: Vfid,
    file_type: FileType,
    flags: u32,
    page_total: u32,
    page_user: u32,
    page_ftab: u32,
    page_free: u32,
    page_marked_delete: u32,
    sector_total: u32,
    sector_partial: u32,
    sector_full: u32,
    sector_empty: u32,
    partial_table_offset: Option<u16>,
    full_table_offset: Option<u16>,
    user_table_offset: Option<u16>,
    sticky_first: Option<Vpid>,
    heap_header_page: Option<Vpid>,
}

impl FileHeader {
    #[must_use]
    pub const fn vfid(self) -> Vfid {
        self.vfid
    }

    #[must_use]
    pub const fn file_type(self) -> FileType {
        self.file_type
    }

    #[must_use]
    pub const fn flags(self) -> u32 {
        self.flags
    }

    #[must_use]
    pub const fn page_total(self) -> u32 {
        self.page_total
    }

    #[must_use]
    pub const fn page_user(self) -> u32 {
        self.page_user
    }

    #[must_use]
    pub const fn page_ftab(self) -> u32 {
        self.page_ftab
    }

    #[must_use]
    pub const fn page_free(self) -> u32 {
        self.page_free
    }

    #[must_use]
    pub const fn page_marked_delete(self) -> u32 {
        self.page_marked_delete
    }

    #[must_use]
    pub const fn sector_total(self) -> u32 {
        self.sector_total
    }

    #[must_use]
    pub const fn sector_partial(self) -> u32 {
        self.sector_partial
    }

    #[must_use]
    pub const fn sector_full(self) -> u32 {
        self.sector_full
    }

    #[must_use]
    pub const fn sector_empty(self) -> u32 {
        self.sector_empty
    }

    #[must_use]
    pub const fn partial_table_offset(self) -> Option<u16> {
        self.partial_table_offset
    }

    #[must_use]
    pub const fn full_table_offset(self) -> Option<u16> {
        self.full_table_offset
    }

    #[must_use]
    pub const fn user_table_offset(self) -> Option<u16> {
        self.user_table_offset
    }

    #[must_use]
    pub const fn sticky_first(self) -> Option<Vpid> {
        self.sticky_first
    }

    #[must_use]
    pub const fn heap_header_page(self) -> Option<Vpid> {
        self.heap_header_page
    }
}

pub fn decode_file_header(envelope: &DecodedPageEnvelope<'_>) -> Result<FileHeader, DecodeError> {
    if envelope.page_type() != PageType::FileTable {
        return Err(error(
            DecodeErrorKind::WrongPageType,
            "file.header.page_type",
        ));
    }
    let view = envelope.plaintext("file.header.encrypted")?;
    let file_id = read_i32(&view, 8, "file.header.self_file")?;
    let vol_id = read_i16(&view, 12, "file.header.self_volume")?;
    if file_id != envelope.id().page_id.get() || vol_id != envelope.id().vol_id.get() {
        return Err(error(
            DecodeErrorKind::IdentityMismatch,
            "file.header.self_identity",
        ));
    }
    let vfid = Vfid::new(
        VolId::new(vol_id)
            .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.header.self_volume"))?,
        FileId::new(file_id)
            .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.header.self_file"))?,
    );
    let page_total = positive(&view, 104, "file.header.page_total")?;
    let page_user = non_negative(&view, 108, "file.header.page_user")?;
    let page_ftab = positive(&view, 112, "file.header.page_ftab")?;
    let page_free = non_negative(&view, 116, "file.header.page_free")?;
    let page_marked_delete = non_negative(&view, 120, "file.header.page_marked_delete")?;
    if page_free
        .checked_add(page_user)
        .and_then(|value| value.checked_add(page_ftab))
        != Some(page_total)
        || page_marked_delete > page_user
    {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "file.header.page_accounting",
        ));
    }
    let sector_total = positive(&view, 124, "file.header.sector_total")?;
    let sector_partial = non_negative(&view, 128, "file.header.sector_partial")?;
    let sector_full = non_negative(&view, 132, "file.header.sector_full")?;
    let sector_empty = non_negative(&view, 136, "file.header.sector_empty")?;
    if sector_partial.checked_add(sector_full) != Some(sector_total)
        || sector_empty > sector_partial
    {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "file.header.sector_accounting",
        ));
    }
    let file_type = decode_file_type(read_i32(&view, 140, "file.header.type")?)?;
    let flags = read_u32(&view, 144, "file.header.flags")?;
    if flags & !0x0f != 0 || flags & 0x0c == 0x0c {
        return Err(error(DecodeErrorKind::InvalidFlags, "file.header.flags"));
    }
    let heap_header_page = if matches!(file_type, FileType::Heap | FileType::HeapReuseSlots) {
        let descriptor_file = read_i32(&view, 48, "file.header.heap_hfid")?;
        let descriptor_volume = read_i16(&view, 52, "file.header.heap_hfid")?;
        if descriptor_file != file_id || descriptor_volume != vol_id {
            return Err(error(
                DecodeErrorKind::IdentityMismatch,
                "file.header.heap_hfid",
            ));
        }
        let page_id = read_i32(&view, 56, "file.header.heap_header_page")?;
        Some(Vpid::new(
            vfid.vol_id,
            PageId::new(page_id)
                .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.header.heap_header_page"))?,
        ))
    } else {
        None
    };
    Ok(FileHeader {
        vfid,
        file_type,
        flags,
        page_total,
        page_user,
        page_ftab,
        page_free,
        page_marked_delete,
        sector_total,
        sector_partial,
        sector_full,
        sector_empty,
        partial_table_offset: table_offset(&view, 150, "file.header.partial_table")?,
        full_table_offset: table_offset(&view, 152, "file.header.full_table")?,
        user_table_offset: table_offset(&view, 154, "file.header.user_table")?,
        sticky_first: optional_vpid(&view, 156, "file.header.sticky_first")?,
        heap_header_page,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtDataHeader {
    pub next: Option<Vpid>,
    pub max_size: u16,
    pub item_size: u16,
    pub item_count: u16,
    pub items_offset: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PartialSectorFact {
    pub vol_id: VolId,
    pub sector_id: SectorId,
    pub page_bitmap: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserPageFact {
    pub vpid: Vpid,
    pub marked_deleted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackerItemFact {
    pub vfid: Vfid,
    pub file_type: FileType,
    pub heap_marked_deleted: bool,
}

pub fn decode_extdata_header(
    envelope: &DecodedPageEnvelope<'_>,
    offset: u16,
    expected_item_size: u16,
) -> Result<ExtDataHeader, DecodeError> {
    if envelope.page_type() != PageType::FileTable {
        return Err(error(
            DecodeErrorKind::WrongPageType,
            "file.extdata.page_type",
        ));
    }
    let view = envelope.plaintext("file.extdata.encrypted")?;
    let offset = usize::from(offset);
    let next = optional_vpid(&view, offset, "file.extdata.next")?;
    let max_size = non_negative_i16(
        read_i16(&view, offset + 8, "file.extdata.max_size")?,
        "file.extdata.max_size",
    )?;
    let item_size = non_negative_i16(
        read_i16(&view, offset + 10, "file.extdata.item_size")?,
        "file.extdata.item_size",
    )?;
    let item_count = non_negative_i16(
        read_i16(&view, offset + 12, "file.extdata.item_count")?,
        "file.extdata.item_count",
    )?;
    let item_bytes = usize::from(item_count)
        .checked_mul(usize::from(item_size))
        .ok_or_else(|| {
            error(
                DecodeErrorKind::ArithmeticOverflow,
                "file.extdata.used_size",
            )
        })?;
    let used = item_bytes.checked_add(EXTDATA_HEADER_SIZE).ok_or_else(|| {
        error(
            DecodeErrorKind::ArithmeticOverflow,
            "file.extdata.used_size",
        )
    })?;
    if item_size != expected_item_size
        || max_size == 0
        || usize::from(max_size) % usize::from(item_size) != 0
        || item_bytes > usize::from(max_size)
        || offset
            .checked_add(used)
            .is_none_or(|end| end > DB_PAGE_SIZE)
    {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "file.extdata.bounds",
        ));
    }
    Ok(ExtDataHeader {
        next,
        max_size,
        item_size,
        item_count,
        items_offset: u16::try_from(offset + EXTDATA_HEADER_SIZE).map_err(|_| {
            error(
                DecodeErrorKind::ArithmeticOverflow,
                "file.extdata.items_offset",
            )
        })?,
    })
}

pub fn decode_partial_sectors(
    envelope: &DecodedPageEnvelope<'_>,
    header: ExtDataHeader,
) -> Result<Vec<PartialSectorFact>, DecodeError> {
    if header.item_size != 16 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "file.partial.item_size",
        ));
    }
    let view = envelope.plaintext("file.partial.encrypted")?;
    (0..header.item_count)
        .map(|index| {
            let offset = usize::from(header.items_offset) + usize::from(index) * 16;
            let sector_id = read_i32(&view, offset, "file.partial.sector")?;
            let vol_id = read_i16(&view, offset + 4, "file.partial.volume")?;
            let page_bitmap = view
                .read_u64_le(offset + 8, "file.partial.bitmap")
                .map_err(|_| error(DecodeErrorKind::ByteAccess, "file.partial.bitmap"))?;
            Ok(PartialSectorFact {
                vol_id: VolId::new(vol_id)
                    .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.partial.volume"))?,
                sector_id: SectorId::new(sector_id)
                    .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.partial.sector"))?,
                page_bitmap,
            })
        })
        .collect()
}

pub fn decode_full_sectors(
    envelope: &DecodedPageEnvelope<'_>,
    header: ExtDataHeader,
) -> Result<Vec<(VolId, SectorId)>, DecodeError> {
    if header.item_size != 8 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "file.full.item_size",
        ));
    }
    let view = envelope.plaintext("file.full.encrypted")?;
    (0..header.item_count)
        .map(|index| {
            let offset = usize::from(header.items_offset) + usize::from(index) * 8;
            let sector_id = read_i32(&view, offset, "file.full.sector")?;
            let vol_id = read_i16(&view, offset + 4, "file.full.volume")?;
            Ok((
                VolId::new(vol_id)
                    .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.full.volume"))?,
                SectorId::new(sector_id)
                    .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.full.sector"))?,
            ))
        })
        .collect()
}

pub fn decode_user_pages(
    envelope: &DecodedPageEnvelope<'_>,
    header: ExtDataHeader,
) -> Result<Vec<UserPageFact>, DecodeError> {
    if header.item_size != 8 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "file.user.item_size",
        ));
    }
    let view = envelope.plaintext("file.user.encrypted")?;
    (0..header.item_count)
        .map(|index| {
            let offset = usize::from(header.items_offset) + usize::from(index) * 8;
            let raw_page = read_u32(&view, offset, "file.user.page")?;
            let vol_id = read_i16(&view, offset + 4, "file.user.volume")?;
            let marked_deleted = raw_page & 0x8000_0000 != 0;
            let page_id = i32::try_from(raw_page & 0x7fff_ffff)
                .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.user.page"))?;
            Ok(UserPageFact {
                vpid: Vpid::new(
                    VolId::new(vol_id)
                        .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.user.volume"))?,
                    PageId::new(page_id)
                        .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.user.page"))?,
                ),
                marked_deleted,
            })
        })
        .collect()
}

pub fn decode_tracker_items(
    envelope: &DecodedPageEnvelope<'_>,
    header: ExtDataHeader,
) -> Result<Vec<TrackerItemFact>, DecodeError> {
    if header.item_size != 16 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "file.tracker.item_size",
        ));
    }
    let view = envelope.plaintext("file.tracker.encrypted")?;
    (0..header.item_count)
        .map(|index| {
            let offset = usize::from(header.items_offset) + usize::from(index) * 16;
            let file_id = read_i32(&view, offset, "file.tracker.file")?;
            let vol_id = read_i16(&view, offset + 4, "file.tracker.volume")?;
            let file_type =
                decode_file_type(i32::from(read_i16(&view, offset + 6, "file.tracker.type")?))?;
            let metadata = view
                .read_u64_le(offset + 8, "file.tracker.metadata")
                .map_err(|_| error(DecodeErrorKind::ByteAccess, "file.tracker.metadata"))?;
            let is_heap = matches!(file_type, FileType::Heap | FileType::HeapReuseSlots);
            if is_heap {
                if metadata & !0xff != 0 || metadata & 0xff > 1 {
                    return Err(error(
                        DecodeErrorKind::InvalidFlags,
                        "file.tracker.heap_metadata",
                    ));
                }
            } else if metadata != 0 {
                return Err(error(
                    DecodeErrorKind::InvalidFlags,
                    "file.tracker.metadata",
                ));
            }
            Ok(TrackerItemFact {
                vfid: Vfid::new(
                    VolId::new(vol_id)
                        .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.tracker.volume"))?,
                    FileId::new(file_id)
                        .map_err(|_| error(DecodeErrorKind::OutOfRange, "file.tracker.file"))?,
                ),
                file_type,
                heap_marked_deleted: is_heap && metadata == 1,
            })
        })
        .collect()
}

fn decode_file_type(value: i32) -> Result<FileType, DecodeError> {
    match value {
        0 => Ok(FileType::Tracker),
        1 => Ok(FileType::Heap),
        2 => Ok(FileType::HeapReuseSlots),
        3 => Ok(FileType::MultipageObjectHeap),
        4 => Ok(FileType::Btree),
        5 => Ok(FileType::BtreeOverflowKey),
        6 => Ok(FileType::ExtensibleHash),
        7 => Ok(FileType::HashDirectory),
        8 => Ok(FileType::Catalog),
        9 => Ok(FileType::DroppedFiles),
        10 => Ok(FileType::VacuumData),
        11 => Ok(FileType::QueryArea),
        12 => Ok(FileType::Temporary),
        13 => Ok(FileType::Oos),
        14 => Ok(FileType::Unknown),
        _ => Err(error(DecodeErrorKind::UnknownEnum, "file.header.type")),
    }
}

fn table_offset(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Option<u16>, DecodeError> {
    let value = read_i16(view, offset, rule)?;
    if value == -1 {
        return Ok(None);
    }
    let value = non_negative_i16(value, rule)?;
    if usize::from(value) < FILE_HEADER_SIZE
        || usize::from(value) + EXTDATA_HEADER_SIZE > DB_PAGE_SIZE
    {
        return Err(error(DecodeErrorKind::InvalidGeometry, rule));
    }
    Ok(Some(value))
}

fn optional_vpid(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Option<Vpid>, DecodeError> {
    let page = read_i32(view, offset, rule)?;
    let volume = read_i16(view, offset + 4, rule)?;
    if page == -1 && volume == -1 {
        return Ok(None);
    }
    if page < 0 || volume < 0 {
        return Err(error(DecodeErrorKind::InvalidGeometry, rule));
    }
    Ok(Some(Vpid::new(
        VolId::new(volume).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        PageId::new(page).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
    )))
}

fn positive(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u32, DecodeError> {
    let value = non_negative(view, offset, rule)?;
    if value == 0 {
        return Err(error(DecodeErrorKind::InvalidLength, rule));
    }
    Ok(value)
}

fn non_negative(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<u32, DecodeError> {
    u32::try_from(read_i32(view, offset, rule)?)
        .map_err(|_| error(DecodeErrorKind::NegativeValue, rule))
}

fn non_negative_i16(value: i16, rule: &'static str) -> Result<u16, DecodeError> {
    u16::try_from(value).map_err(|_| error(DecodeErrorKind::NegativeValue, rule))
}

fn read_i16(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i16, DecodeError> {
    view.read_i16_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

fn read_i32(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i32, DecodeError> {
    view.read_i32_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

fn read_u32(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u32, DecodeError> {
    view.read_u32_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
