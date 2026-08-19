use crate::bytes::ByteView;
use crate::model::{FileId, Oid, PageId, SlotId, Vfid, VolId, Vpid};

use super::{DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType, RecordType, SlottedPage};

const HEAP_HEADER_SIZE: u16 = 1_160;
const HEAP_CHAIN_SIZE: u16 = 40;
const HEAP_CHAIN_ALLOWED_FLAGS: u32 = 0xc000_0003;

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

fn read_u64(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u64, DecodeError> {
    view.read_u64_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
