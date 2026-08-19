use crate::bytes::ByteView;
use crate::model::{FileId, Oid, PageId, SlotId, Vfid, VolId, Vpid};

use super::{DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType, RecordType, SlottedPage};

const HEAP_HEADER_SIZE: u16 = 1_160;
const HEAP_CHAIN_SIZE: u16 = 40;
const HEAP_CHAIN_ALLOWED_FLAGS: u32 = 0xc000_0003;
const OBJECT_MIN_HEADER_SIZE: u16 = 8;
const OBJECT_MVCC_FLAG_MASK: u8 = 0x07;
const OBJECT_HAS_OOS_FLAG: u8 = 0x08;
const OBJECT_ALLOWED_FLAGS: u8 = OBJECT_MVCC_FLAG_MASK | OBJECT_HAS_OOS_FLAG;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeapHeaderFact {
    pub class_oid: Option<Oid>,
    pub overflow_vfid: Option<Vfid>,
    pub next: Option<Vpid>,
    pub last: Vpid,
    pub oos_vfid: Option<Vfid>,
    pub unfill_space: u32,
    pub estimated_pages: u32,
    pub estimated_records: u64,
    pub estimated_record_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeapChainFact {
    pub class_oid: Option<Oid>,
    pub previous: Option<Vpid>,
    pub next: Option<Vpid>,
    pub max_mvccid: u64,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeapPageFact {
    Header(HeapHeaderFact),
    Chain(HeapChainFact),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeapRecordEnvelopeFact {
    pub slot_id: u16,
    pub record_type: RecordType,
    pub is_mvcc: bool,
    pub representation_id: u32,
    pub chn: i32,
    pub record_flags: u8,
    pub mvcc_flags: u8,
    pub has_bound_bits: bool,
    pub has_oos: bool,
    pub variable_offset_width: u8,
    pub header_length: u16,
    pub insert_mvccid: Option<u64>,
    pub delete_mvccid: Option<u64>,
    pub previous_version_lsa_word: Option<u64>,
    pub body_offset: u16,
    pub body_length: u16,
}

#[derive(Clone, Copy)]
struct MvccFields {
    insert_mvccid: Option<u64>,
    delete_mvccid: Option<u64>,
    previous_version_lsa_word: Option<u64>,
}

/// Decodes the value-free envelope of a caller-proven heap object record.
///
/// Whether MVCC is enabled is a class property, not an on-page discriminator.
/// The caller must derive `is_mvcc` from trusted class identity. The previous
/// version LSA is retained as an opaque word because the pinned engine stores
/// its native C++ bit-field representation directly in the record.
pub fn decode_heap_record_envelope(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    slot_id: u16,
    is_mvcc: bool,
) -> Result<HeapRecordEnvelopeFact, DecodeError> {
    if envelope.page_type() != PageType::Heap {
        return Err(error(
            DecodeErrorKind::WrongPageType,
            "heap.record.page_type",
        ));
    }
    if slot_id == 0 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "heap.record.data_slot",
        ));
    }
    let slot = slotted
        .slots()
        .get(usize::from(slot_id))
        .ok_or_else(|| error(DecodeErrorKind::OutOfRange, "heap.record.slot_exists"))?;
    if !matches!(slot.record_type(), RecordType::Home | RecordType::NewHome)
        || slot.offset() == 0
        || slot.length() < OBJECT_MIN_HEADER_SIZE
    {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "heap.record.record_shape",
        ));
    }

    let view = envelope.plaintext("heap.record.encrypted")?;
    let base = usize::from(slot.offset());
    let first_word = read_u32_be(&view, base, "heap.record.representation")?;
    let record_flags = u8::try_from((first_word >> 24) & 0x1f)
        .map_err(|_| error(DecodeErrorKind::ArithmeticOverflow, "heap.record.flags"))?;
    if record_flags & !OBJECT_ALLOWED_FLAGS != 0
        || (!is_mvcc && record_flags & OBJECT_MVCC_FLAG_MASK != 0)
    {
        return Err(error(DecodeErrorKind::InvalidFlags, "heap.record.flags"));
    }
    let mvcc_flags = record_flags & OBJECT_MVCC_FLAG_MASK;
    let header_length = if is_mvcc {
        OBJECT_MIN_HEADER_SIZE
            + 8 * u16::try_from(mvcc_flags.count_ones()).map_err(|_| {
                error(
                    DecodeErrorKind::ArithmeticOverflow,
                    "heap.record.header_length",
                )
            })?
    } else {
        OBJECT_MIN_HEADER_SIZE
    };
    if header_length > slot.length() {
        return Err(error(
            DecodeErrorKind::InvalidLength,
            "heap.record.header_length",
        ));
    }

    let mvcc = decode_mvcc_fields(&view, base, mvcc_flags)?;
    let body_offset = slot.offset().checked_add(header_length).ok_or_else(|| {
        error(
            DecodeErrorKind::ArithmeticOverflow,
            "heap.record.body_offset",
        )
    })?;

    Ok(HeapRecordEnvelopeFact {
        slot_id,
        record_type: slot.record_type(),
        is_mvcc,
        representation_id: first_word & 0x00ff_ffff,
        chn: read_i32_be(&view, base + 4, "heap.record.chn")?,
        record_flags,
        mvcc_flags,
        has_bound_bits: first_word & 0x8000_0000 != 0,
        has_oos: record_flags & OBJECT_HAS_OOS_FLAG != 0,
        variable_offset_width: match first_word & 0x6000_0000 {
            0x2000_0000 => 1,
            0x4000_0000 => 2,
            _ => 4,
        },
        header_length,
        insert_mvccid: mvcc.insert_mvccid,
        delete_mvccid: mvcc.delete_mvccid,
        previous_version_lsa_word: mvcc.previous_version_lsa_word,
        body_offset,
        body_length: slot.length() - header_length,
    })
}

fn decode_mvcc_fields(
    view: &ByteView<'_>,
    base: usize,
    flags: u8,
) -> Result<MvccFields, DecodeError> {
    let mut cursor = base + usize::from(OBJECT_MIN_HEADER_SIZE);
    let insert_mvccid = if flags & 0x01 != 0 {
        let value = read_u64_be(view, cursor, "heap.record.insert_mvccid")?;
        cursor += 8;
        Some(value)
    } else {
        None
    };
    let delete_mvccid = if flags & 0x02 != 0 {
        let value = read_u64_be(view, cursor, "heap.record.delete_mvccid")?;
        cursor += 8;
        Some(value)
    } else {
        None
    };
    let previous_version_lsa_word = if flags & 0x04 != 0 {
        Some(read_u64(view, cursor, "heap.record.previous_version_lsa")?)
    } else {
        None
    };
    Ok(MvccFields {
        insert_mvccid,
        delete_mvccid,
        previous_version_lsa_word,
    })
}

pub fn decode_relocation_target(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    slot_id: u16,
) -> Result<Oid, DecodeError> {
    if envelope.page_type() != PageType::Heap {
        return Err(error(
            DecodeErrorKind::WrongPageType,
            "heap.relocation.page_type",
        ));
    }
    let slot = slotted
        .slots()
        .get(usize::from(slot_id))
        .ok_or_else(|| error(DecodeErrorKind::OutOfRange, "heap.relocation.slot_exists"))?;
    if slot.record_type() != RecordType::Relocation || slot.offset() == 0 || slot.length() != 8 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "heap.relocation.record_shape",
        ));
    }
    let view = envelope.plaintext("heap.relocation.encrypted")?;
    optional_oid(&view, usize::from(slot.offset()), "heap.relocation.target")?.ok_or_else(|| {
        error(
            DecodeErrorKind::InvalidGeometry,
            "heap.relocation.target_required",
        )
    })
}

pub fn decode_bigone_target(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    slot_id: u16,
) -> Result<Vpid, DecodeError> {
    if envelope.page_type() != PageType::Heap {
        return Err(error(
            DecodeErrorKind::WrongPageType,
            "heap.bigone.page_type",
        ));
    }
    let slot = slotted
        .slots()
        .get(usize::from(slot_id))
        .ok_or_else(|| error(DecodeErrorKind::OutOfRange, "heap.bigone.slot_exists"))?;
    if slot.record_type() != RecordType::BigOne || slot.offset() == 0 || slot.length() != 8 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "heap.bigone.record_shape",
        ));
    }
    let view = envelope.plaintext("heap.bigone.encrypted")?;
    let offset = usize::from(slot.offset());
    let page = read_i32(&view, offset, "heap.bigone.target")?;
    let target_slot = read_i16(&view, offset + 4, "heap.bigone.target")?;
    let volume = read_i16(&view, offset + 6, "heap.bigone.target")?;
    if target_slot != -1 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "heap.bigone.null_slot",
        ));
    }
    Ok(Vpid::new(
        VolId::new(volume).map_err(|_| error(DecodeErrorKind::OutOfRange, "heap.bigone.target"))?,
        PageId::new(page).map_err(|_| error(DecodeErrorKind::OutOfRange, "heap.bigone.target"))?,
    ))
}

pub fn decode_heap_page(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    is_header: bool,
) -> Result<HeapPageFact, DecodeError> {
    if envelope.page_type() != PageType::Heap {
        return Err(error(DecodeErrorKind::WrongPageType, "heap.page.type"));
    }
    let slot = slotted
        .slots()
        .first()
        .filter(|slot| slot.slot_id() == 0 && slot.record_type() == RecordType::Home)
        .ok_or_else(|| error(DecodeErrorKind::InvalidGeometry, "heap.page.slot_zero"))?;
    let expected = if is_header {
        HEAP_HEADER_SIZE
    } else {
        HEAP_CHAIN_SIZE
    };
    if slot.length() != expected {
        return Err(error(
            DecodeErrorKind::InvalidLength,
            "heap.page.role_length",
        ));
    }
    let view = envelope.plaintext("heap.page.encrypted")?;
    let base = usize::from(slot.offset());
    if is_header {
        let last = required_vpid(&view, base + 24, "heap.header.last")?;
        let unfill_space = non_negative_i32(&view, base + 40, "heap.header.unfill_space")?;
        let estimated_pages = non_negative_i32(&view, base + 44, "heap.header.page_count")?;
        Ok(HeapPageFact::Header(HeapHeaderFact {
            class_oid: optional_oid(&view, base, "heap.header.class_oid")?,
            overflow_vfid: optional_vfid(&view, base + 8, "heap.header.overflow_vfid")?,
            next: optional_vpid(&view, base + 16, "heap.header.next")?,
            last,
            oos_vfid: optional_vfid(&view, base + 32, "heap.header.oos_vfid")?,
            unfill_space,
            estimated_pages,
            estimated_records: read_u64(&view, base + 48, "heap.header.record_count")?,
            estimated_record_bytes: read_u64(&view, base + 56, "heap.header.record_bytes")?,
        }))
    } else {
        let flags = read_u32(&view, base + 32, "heap.chain.flags")?;
        if flags & !HEAP_CHAIN_ALLOWED_FLAGS != 0 || flags & 0xc000_0000 == 0xc000_0000 {
            return Err(error(DecodeErrorKind::InvalidFlags, "heap.chain.flags"));
        }
        Ok(HeapPageFact::Chain(HeapChainFact {
            class_oid: optional_oid(&view, base, "heap.chain.class_oid")?,
            previous: optional_vpid(&view, base + 8, "heap.chain.previous")?,
            next: optional_vpid(&view, base + 16, "heap.chain.next")?,
            max_mvccid: read_u64(&view, base + 24, "heap.chain.max_mvccid")?,
            flags,
        }))
    }
}

fn optional_oid(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Option<Oid>, DecodeError> {
    let page = read_i32(view, offset, rule)?;
    let slot = read_i16(view, offset + 4, rule)?;
    let volume = read_i16(view, offset + 6, rule)?;
    if page == -1 && slot == -1 && volume == -1 {
        return Ok(None);
    }
    if page < 0 || slot < 0 || volume < 0 {
        return Err(error(DecodeErrorKind::InvalidGeometry, rule));
    }
    Ok(Some(Oid::new(
        VolId::new(volume).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        PageId::new(page).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        SlotId::new(slot).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
    )))
}

fn required_vpid(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Vpid, DecodeError> {
    optional_vpid(view, offset, rule)?.ok_or_else(|| {
        error(
            DecodeErrorKind::InvalidGeometry,
            "heap.header.last_required",
        )
    })
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

fn optional_vfid(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Option<Vfid>, DecodeError> {
    let file = read_i32(view, offset, rule)?;
    let volume = read_i16(view, offset + 4, rule)?;
    if file == -1 && volume == -1 {
        return Ok(None);
    }
    if file < 0 || volume < 0 {
        return Err(error(DecodeErrorKind::InvalidGeometry, rule));
    }
    Ok(Some(Vfid::new(
        VolId::new(volume).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        FileId::new(file).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
    )))
}

fn non_negative_i32(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<u32, DecodeError> {
    u32::try_from(read_i32(view, offset, rule)?)
        .map_err(|_| error(DecodeErrorKind::NegativeValue, rule))
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

fn read_i32_be(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i32, DecodeError> {
    view.read_i32_be(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

fn read_u32_be(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u32, DecodeError> {
    let bytes = view
        .range(offset, 4, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?;
    Ok(u32::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?,
    ))
}

fn read_u64_be(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u64, DecodeError> {
    let bytes = view
        .range(offset, 8, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?;
    Ok(u64::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?,
    ))
}

fn read_u64(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u64, DecodeError> {
    view.read_u64_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
