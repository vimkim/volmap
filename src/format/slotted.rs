use crate::bytes::ByteView;
use crate::model::{Oid, PageId, SlotId, VolId};

use super::{DB_PAGE_SIZE, DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType};

pub const SLOTTED_HEADER_SIZE: usize = 32;
const HEADER_SIZE: usize = SLOTTED_HEADER_SIZE;
pub const SLOTTED_SLOT_SIZE: usize = 4;
const SLOT_SIZE: usize = SLOTTED_SLOT_SIZE;

/// Decode the free-space summary available in a plaintext slotted-page header.
///
/// This deliberately validates only header geometry. It is used by the eager
/// volume scan, while [`decode_slotted_page`] remains the authoritative deep
/// decoder for slot and record geometry.
pub fn decode_slotted_free_space_header(bytes: &[u8]) -> Result<u32, DecodeError> {
    if bytes.len() != HEADER_SIZE {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidLength,
            "slotted.header.length",
        ));
    }
    let view = ByteView::new(bytes, 0);
    let num_slots = non_negative_i16(
        read_i16(&view, 0, "slotted.header.num_slots")?,
        "slotted.header.num_slots",
    )?;
    let num_records = non_negative_i16(
        read_i16(&view, 2, "slotted.header.num_records")?,
        "slotted.header.num_records",
    )?;
    if num_records > num_slots {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.header.record_count",
        ));
    }
    if !matches!(read_i16(&view, 4, "slotted.header.anchor")?, 1..=4) {
        return Err(DecodeError::new(
            DecodeErrorKind::UnknownEnum,
            "slotted.header.anchor",
        ));
    }
    let alignment = read_u16(&view, 6, "slotted.header.alignment")?;
    if !matches!(alignment, 1 | 2 | 4 | 8) {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.header.alignment",
        ));
    }
    let total_free = non_negative_i32(
        read_i32(&view, 8, "slotted.header.total_free")?,
        "slotted.header.total_free",
    )?;
    let contiguous_free = non_negative_i32(
        read_i32(&view, 12, "slotted.header.contiguous_free")?,
        "slotted.header.contiguous_free",
    )?;
    let free_area_offset = non_negative_i32(
        read_i32(&view, 16, "slotted.header.free_area_offset")?,
        "slotted.header.free_area_offset",
    )?;
    if contiguous_free > total_free {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.header.free_space_order",
        ));
    }
    let slot_bytes = usize::from(num_slots)
        .checked_mul(SLOT_SIZE)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::ArithmeticOverflow,
                "slotted.slot_array.size",
            )
        })?;
    let slot_start = DB_PAGE_SIZE.checked_sub(slot_bytes).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.slot_array.bounds",
        )
    })?;
    if slot_start < HEADER_SIZE
        || usize::try_from(free_area_offset)
            .map_or(true, |offset| offset < HEADER_SIZE || offset > slot_start)
        || total_free as usize > DB_PAGE_SIZE - HEADER_SIZE
    {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.header.free_area_bounds",
        ));
    }
    Ok(total_free)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnchorType {
    Anchored,
    AnchoredNoReuse,
    UnanchoredAnySequence,
    UnanchoredKeepSequence,
}

impl AnchorType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Anchored => "anchored",
            Self::AnchoredNoReuse => "anchored-no-reuse",
            Self::UnanchoredAnySequence => "unanchored-any-sequence",
            Self::UnanchoredKeepSequence => "unanchored-keep-sequence",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordType {
    Unknown,
    AssignAddress,
    Home,
    NewHome,
    Relocation,
    BigOne,
    MarkDeleted,
    DeletedWillReuse,
    Reserved(u8),
}

impl RecordType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::AssignAddress => "assign-address",
            Self::Home => "home",
            Self::NewHome => "new-home",
            Self::Relocation => "relocation",
            Self::BigOne => "bigone",
            Self::MarkDeleted => "marked-deleted",
            Self::DeletedWillReuse => "deleted-will-reuse",
            Self::Reserved(_) => "reserved",
        }
    }

    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::AssignAddress => 1,
            Self::Home => 2,
            Self::NewHome => 3,
            Self::Relocation => 4,
            Self::BigOne => 5,
            Self::MarkDeleted => 6,
            Self::DeletedWillReuse => 7,
            Self::Reserved(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotFact {
    slot_id: u16,
    offset: u16,
    length: u16,
    record_type: RecordType,
}

impl SlotFact {
    #[must_use]
    pub const fn slot_id(self) -> u16 {
        self.slot_id
    }

    #[must_use]
    pub const fn offset(self) -> u16 {
        self.offset
    }

    #[must_use]
    pub const fn length(self) -> u16 {
        self.length
    }

    #[must_use]
    pub const fn record_type(self) -> RecordType {
        self.record_type
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.offset == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlottedPage {
    anchor: AnchorType,
    alignment: u16,
    total_free: u32,
    contiguous_free: u32,
    free_area_offset: u32,
    flags: u32,
    is_saving: bool,
    slots: Vec<SlotFact>,
}

impl SlottedPage {
    #[must_use]
    pub const fn anchor(&self) -> AnchorType {
        self.anchor
    }

    #[must_use]
    pub const fn alignment(&self) -> u16 {
        self.alignment
    }

    #[must_use]
    pub const fn total_free(&self) -> u32 {
        self.total_free
    }

    #[must_use]
    pub const fn contiguous_free(&self) -> u32 {
        self.contiguous_free
    }

    #[must_use]
    pub const fn free_area_offset(&self) -> u32 {
        self.free_area_offset
    }

    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    #[must_use]
    pub const fn is_saving(&self) -> bool {
        self.is_saving
    }

    #[must_use]
    pub fn slots(&self) -> &[SlotFact] {
        &self.slots
    }
}

#[allow(clippy::too_many_lines)]
pub fn decode_slotted_page(envelope: &DecodedPageEnvelope<'_>) -> Result<SlottedPage, DecodeError> {
    if !matches!(
        envelope.page_type(),
        PageType::Heap
            | PageType::Oos
            | PageType::Btree
            | PageType::ExtensibleHash
            | PageType::Catalog
    ) {
        return Err(DecodeError::new(
            DecodeErrorKind::WrongPageType,
            "slotted.page.type",
        ));
    }
    let view = envelope.plaintext("slotted.page.encrypted")?;
    let num_slots = non_negative_i16(
        read_i16(&view, 0, "slotted.header.num_slots")?,
        "slotted.header.num_slots",
    )?;
    let num_records = non_negative_i16(
        read_i16(&view, 2, "slotted.header.num_records")?,
        "slotted.header.num_records",
    )?;
    if num_records > num_slots {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.header.record_count",
        ));
    }
    let anchor = match read_i16(&view, 4, "slotted.header.anchor")? {
        1 => AnchorType::Anchored,
        2 => AnchorType::AnchoredNoReuse,
        3 => AnchorType::UnanchoredAnySequence,
        4 => AnchorType::UnanchoredKeepSequence,
        _ => {
            return Err(DecodeError::new(
                DecodeErrorKind::UnknownEnum,
                "slotted.header.anchor",
            ));
        }
    };
    let alignment = read_u16(&view, 6, "slotted.header.alignment")?;
    if !matches!(alignment, 1 | 2 | 4 | 8) {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.header.alignment",
        ));
    }
    let total_free = non_negative_i32(
        read_i32(&view, 8, "slotted.header.total_free")?,
        "slotted.header.total_free",
    )?;
    let contiguous_free = non_negative_i32(
        read_i32(&view, 12, "slotted.header.contiguous_free")?,
        "slotted.header.contiguous_free",
    )?;
    let free_area_offset = non_negative_i32(
        read_i32(&view, 16, "slotted.header.free_area_offset")?,
        "slotted.header.free_area_offset",
    )?;
    if contiguous_free > total_free {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.header.free_space_order",
        ));
    }
    let slot_bytes = usize::from(num_slots)
        .checked_mul(SLOT_SIZE)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::ArithmeticOverflow,
                "slotted.slot_array.size",
            )
        })?;
    let slot_start = DB_PAGE_SIZE.checked_sub(slot_bytes).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.slot_array.bounds",
        )
    })?;
    if slot_start < HEADER_SIZE
        || usize::try_from(free_area_offset)
            .map_or(true, |offset| offset < HEADER_SIZE || offset > slot_start)
        || total_free as usize > DB_PAGE_SIZE - HEADER_SIZE
    {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.header.free_area_bounds",
        ));
    }

    let mut slots = Vec::with_capacity(usize::from(num_slots));
    let mut ranges = Vec::with_capacity(usize::from(num_records));
    for raw_slot in 0..num_slots {
        let slot_id = usize::from(raw_slot);
        let offset = DB_PAGE_SIZE
            .checked_sub((slot_id + 1) * SLOT_SIZE)
            .ok_or_else(|| {
                DecodeError::new(DecodeErrorKind::ArithmeticOverflow, "slotted.slot.offset")
            })?;
        let word = read_u32(&view, offset, "slotted.slot.word")?;
        let record_offset = u16::try_from(word & 0x3fff).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::ArithmeticOverflow,
                "slotted.slot.record_offset",
            )
        })?;
        let record_length = u16::try_from((word >> 14) & 0x3fff).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::ArithmeticOverflow,
                "slotted.slot.record_length",
            )
        })?;
        let record_ordinal = u8::try_from(word >> 28).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::ArithmeticOverflow,
                "slotted.slot.record_type",
            )
        })?;
        let record_type = record_type(record_ordinal);
        if record_offset == 0 {
            // CUBRID clears only the offset when an anchored slot is deleted;
            // the old length remains in the slot word. It is not a live byte
            // range and is valid only for the two engine-defined tombstones.
            if record_length != 0
                && !matches!(
                    record_type,
                    RecordType::MarkDeleted | RecordType::DeletedWillReuse
                )
            {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidGeometry,
                    "slotted.slot.empty_length",
                ));
            }
        } else {
            let start = usize::from(record_offset);
            let end = start
                .checked_add(usize::from(record_length))
                .ok_or_else(|| {
                    DecodeError::new(
                        DecodeErrorKind::ArithmeticOverflow,
                        "slotted.slot.record_end",
                    )
                })?;
            if start < HEADER_SIZE || end > slot_start || start % usize::from(alignment) != 0 {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidGeometry,
                    "slotted.slot.record_bounds",
                ));
            }
            ranges.push((start, end));
        }
        slots.push(SlotFact {
            slot_id: raw_slot,
            offset: record_offset,
            length: record_length,
            record_type,
        });
    }
    if ranges.len() != usize::from(num_records) {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.header.record_count_match",
        ));
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "slotted.slot.nonoverlap",
        ));
    }
    let saving_word = read_u32(&view, 28, "slotted.header.saving_word")?;
    Ok(SlottedPage {
        anchor,
        alignment,
        total_free,
        contiguous_free,
        free_area_offset,
        flags: read_u32(&view, 24, "slotted.header.flags")?,
        is_saving: saving_word & 1 != 0,
        slots,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OosNext {
    Terminal,
    Link(Oid),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OosChunkFact {
    total_data_length: u32,
    chunk_index: u32,
    next: OosNext,
    payload_offset: u16,
    payload_length: u16,
}

impl OosChunkFact {
    #[must_use]
    pub const fn total_data_length(self) -> u32 {
        self.total_data_length
    }

    #[must_use]
    pub const fn chunk_index(self) -> u32 {
        self.chunk_index
    }

    #[must_use]
    pub const fn next(self) -> OosNext {
        self.next
    }

    #[must_use]
    pub const fn payload_offset(self) -> u16 {
        self.payload_offset
    }

    #[must_use]
    pub const fn payload_length(self) -> u16 {
        self.payload_length
    }
}

pub fn decode_oos_chunk(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    slot_id: u16,
) -> Result<OosChunkFact, DecodeError> {
    if envelope.page_type() != PageType::Oos {
        return Err(DecodeError::new(
            DecodeErrorKind::WrongPageType,
            "oos.chunk.page_type",
        ));
    }
    let slot = slotted
        .slots
        .get(usize::from(slot_id))
        .ok_or_else(|| DecodeError::new(DecodeErrorKind::OutOfRange, "oos.chunk.slot_exists"))?;
    if slot.record_type != RecordType::Home || slot.length < 16 || slot.offset == 0 {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "oos.chunk.record_shape",
        ));
    }
    let view = envelope.plaintext("oos.chunk.encrypted")?;
    let offset = usize::from(slot.offset);
    let total = non_negative_i32(
        read_i32(&view, offset, "oos.chunk.total_length")?,
        "oos.chunk.total_length",
    )?;
    if total == 0 {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidLength,
            "oos.chunk.total_positive",
        ));
    }
    let chunk_index = non_negative_i32(
        read_i32(&view, offset + 4, "oos.chunk.index")?,
        "oos.chunk.index",
    )?;
    let page_id = read_i32(&view, offset + 8, "oos.chunk.next_page")?;
    let next_slot = read_i16(&view, offset + 12, "oos.chunk.next_slot")?;
    let vol_id = read_i16(&view, offset + 14, "oos.chunk.next_volume")?;
    let next = if page_id == -1 && next_slot == -1 && vol_id == -1 {
        OosNext::Terminal
    } else if page_id >= 0 && next_slot >= 0 && vol_id >= 0 {
        OosNext::Link(Oid::new(
            VolId::new(vol_id).map_err(|_| {
                DecodeError::new(DecodeErrorKind::OutOfRange, "oos.chunk.next_volume")
            })?,
            PageId::new(page_id).map_err(|_| {
                DecodeError::new(DecodeErrorKind::OutOfRange, "oos.chunk.next_page")
            })?,
            SlotId::new(next_slot).map_err(|_| {
                DecodeError::new(DecodeErrorKind::OutOfRange, "oos.chunk.next_slot")
            })?,
        ))
    } else {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "oos.chunk.next_oid",
        ));
    };
    let payload_length = slot.length - 16;
    if payload_length == 0 || u32::from(payload_length) > total {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidLength,
            "oos.chunk.payload_length",
        ));
    }
    Ok(OosChunkFact {
        total_data_length: total,
        chunk_index,
        next,
        payload_offset: slot.offset + 16,
        payload_length,
    })
}

const fn record_type(value: u8) -> RecordType {
    match value {
        0 => RecordType::Unknown,
        1 => RecordType::AssignAddress,
        2 => RecordType::Home,
        3 => RecordType::NewHome,
        4 => RecordType::Relocation,
        5 => RecordType::BigOne,
        6 => RecordType::MarkDeleted,
        7 => RecordType::DeletedWillReuse,
        other => RecordType::Reserved(other),
    }
}

fn non_negative_i16(value: i16, rule: &'static str) -> Result<u16, DecodeError> {
    u16::try_from(value).map_err(|_| DecodeError::new(DecodeErrorKind::NegativeValue, rule))
}

fn non_negative_i32(value: i32, rule: &'static str) -> Result<u32, DecodeError> {
    u32::try_from(value).map_err(|_| DecodeError::new(DecodeErrorKind::NegativeValue, rule))
}

fn read_i16(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i16, DecodeError> {
    view.read_i16_le(offset, rule)
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))
}

fn read_u16(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u16, DecodeError> {
    view.read_u16_le(offset, rule)
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))
}

fn read_i32(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i32, DecodeError> {
    view.read_i32_le(offset, rule)
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))
}

fn read_u32(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u32, DecodeError> {
    view.read_u32_le(offset, rule)
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))
}
