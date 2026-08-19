use crate::bytes::ByteView;
use crate::model::{FileId, Oid, PageId, SlotId, Vfid, VolId, Vpid};

use super::{DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType, RecordType, SlottedPage};

const NODE_HEADER_SIZE: u16 = 32;
const ROOT_FIXED_SIZE: u16 = 88;
const OVERFLOW_HEADER_SIZE: u16 = 8;
const NONLEAF_PREAMBLE_SIZE: u16 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BtreeNodeFact {
    pub previous: Option<Vpid>,
    pub next: Option<Vpid>,
    pub level: u16,
    pub max_key_length: u16,
    pub common_prefix: Option<u32>,
    pub split_pivot_bits: u32,
    pub split_index: u32,
    pub record_count: u16,
    pub record_bytes: u32,
    pub child_count: u16,
    pub overflow_key_count: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BtreeRootFact {
    pub node: BtreeNodeFact,
    pub oid_count: i64,
    pub null_count: i64,
    pub key_count: i64,
    pub top_class: Oid,
    pub constraint_flags: u32,
    pub revision_level: i16,
    pub deduplicate_key_encoded: i16,
    pub overflow_key_file: Option<Vfid>,
    pub creator_mvccid: u64,
    pub domain_offset: u16,
    pub domain_length: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BtreeOidOverflowFact {
    pub next: Option<Vpid>,
    pub record_count: u16,
    pub record_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BtreePageFact {
    Root(BtreeRootFact),
    Leaf(BtreeNodeFact),
    NonLeaf(BtreeNodeFact),
    OidOverflow(BtreeOidOverflowFact),
}

pub fn decode_btree_page(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    is_root: bool,
) -> Result<BtreePageFact, DecodeError> {
    if envelope.page_type() != PageType::Btree {
        return Err(error(DecodeErrorKind::WrongPageType, "btree.page.type"));
    }
    let header = slotted
        .slots()
        .first()
        .filter(|slot| {
            slot.slot_id() == 0 && slot.record_type() == RecordType::Home && slot.offset() != 0
        })
        .ok_or_else(|| error(DecodeErrorKind::InvalidGeometry, "btree.page.header_slot"))?;
    let view = envelope.plaintext("btree.page.encrypted")?;
    let base = usize::from(header.offset());
    if is_root {
        if header.length() <= ROOT_FIXED_SIZE {
            return Err(error(
                DecodeErrorKind::InvalidLength,
                "btree.root.header_length",
            ));
        }
        let node = decode_node(&view, base, slotted, false)?;
        let oid_count = read_i64(&view, base + 32, "btree.root.oid_count")?;
        let null_count = read_i64(&view, base + 40, "btree.root.null_count")?;
        let key_count = read_i64(&view, base + 48, "btree.root.key_count")?;
        let constraint_flags = read_u32(&view, base + 64, "btree.root.constraint_flags")?;
        if constraint_flags & !0x03 != 0
            || constraint_flags & 0x02 != 0 && constraint_flags & 0x01 == 0
        {
            return Err(error(
                DecodeErrorKind::InvalidFlags,
                "btree.root.constraint_flags",
            ));
        }
        let counts_valid = if constraint_flags & 0x01 == 0 {
            oid_count == -1 && null_count == -1 && key_count == -1
        } else {
            oid_count >= 0 && null_count >= 0 && key_count >= 0 && key_count <= oid_count
        };
        if !counts_valid {
            return Err(error(
                DecodeErrorKind::InvalidGeometry,
                "btree.root.statistics",
            ));
        }
        let packed_revision = read_u32(&view, base + 68, "btree.root.revision")?;
        let packed_revision = packed_revision.to_le_bytes();
        let revision_level = i16::from_le_bytes([packed_revision[0], packed_revision[1]]);
        let deduplicate_key_encoded = i16::from_le_bytes([packed_revision[2], packed_revision[3]]);
        if revision_level != 0 || deduplicate_key_encoded < 0 {
            return Err(error(DecodeErrorKind::UnknownEnum, "btree.root.revision"));
        }
        let domain_length = header.length() - ROOT_FIXED_SIZE;
        return Ok(BtreePageFact::Root(BtreeRootFact {
            node,
            oid_count,
            null_count,
            key_count,
            top_class: required_oid(&view, base + 56, "btree.root.top_class")?,
            constraint_flags,
            revision_level,
            deduplicate_key_encoded,
            overflow_key_file: optional_vfid(&view, base + 72, "btree.root.overflow_key_file")?,
            creator_mvccid: read_u64(&view, base + 80, "btree.root.creator_mvccid")?,
            domain_offset: header
                .offset()
                .checked_add(ROOT_FIXED_SIZE)
                .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "btree.root.domain"))?,
            domain_length,
        }));
    }
    match header.length() {
        NODE_HEADER_SIZE => {
            let node = decode_node(&view, base, slotted, true)?;
            if node.level == 1 {
                Ok(BtreePageFact::Leaf(node))
            } else {
                Ok(BtreePageFact::NonLeaf(node))
            }
        }
        OVERFLOW_HEADER_SIZE => {
            let (record_count, record_bytes) = record_extents(slotted)?;
            Ok(BtreePageFact::OidOverflow(BtreeOidOverflowFact {
                next: optional_vpid(&view, base, "btree.oid_overflow.next")?,
                record_count,
                record_bytes,
            }))
        }
        _ => Err(error(
            DecodeErrorKind::InvalidLength,
            "btree.page.role_length",
        )),
    }
}

fn decode_node(
    view: &ByteView<'_>,
    base: usize,
    slotted: &SlottedPage,
    decode_common_prefix: bool,
) -> Result<BtreeNodeFact, DecodeError> {
    let level = positive_i16(
        read_i16(view, base + 24, "btree.node.level")?,
        "btree.node.level",
    )?;
    let max_key_length = non_negative_i16(
        read_i16(view, base + 26, "btree.node.max_key_length")?,
        "btree.node.max_key_length",
    )?;
    // Root creation does not initialize this field at the pinned commit, so
    // treating its bytes as a fact would expose indeterminate stack content.
    let common_prefix = if decode_common_prefix {
        Some(non_negative_i32(
            read_i32(view, base + 28, "btree.node.common_prefix")?,
            "btree.node.common_prefix",
        )?)
    } else {
        None
    };
    let split_index = non_negative_i32(
        read_i32(view, base + 4, "btree.node.split_index")?,
        "btree.node.split_index",
    )?;
    let (record_count, record_bytes) = record_extents(slotted)?;
    let (child_count, overflow_key_count) = if level > 1 {
        nonleaf_records(view, slotted)?
    } else {
        (0, 0)
    };
    Ok(BtreeNodeFact {
        previous: optional_vpid(view, base + 8, "btree.node.previous")?,
        next: optional_vpid(view, base + 16, "btree.node.next")?,
        level,
        max_key_length,
        common_prefix,
        split_pivot_bits: read_u32(view, base, "btree.node.split_pivot")?,
        split_index,
        record_count,
        record_bytes,
        child_count,
        overflow_key_count,
    })
}

fn record_extents(slotted: &SlottedPage) -> Result<(u16, u32), DecodeError> {
    let mut count = 0_u16;
    let mut bytes = 0_u32;
    for slot in slotted
        .slots()
        .iter()
        .skip(1)
        .filter(|slot| !slot.is_empty())
    {
        if slot.record_type() != RecordType::Home {
            return Err(error(DecodeErrorKind::InvalidGeometry, "btree.record.type"));
        }
        count = count
            .checked_add(1)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "btree.record.count"))?;
        bytes = bytes
            .checked_add(u32::from(slot.length()))
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "btree.record.bytes"))?;
    }
    Ok((count, bytes))
}

fn nonleaf_records(view: &ByteView<'_>, slotted: &SlottedPage) -> Result<(u16, u16), DecodeError> {
    let mut children = 0_u16;
    let mut overflow_keys = 0_u16;
    for slot in slotted
        .slots()
        .iter()
        .skip(1)
        .filter(|slot| !slot.is_empty())
    {
        if slot.length() < NONLEAF_PREAMBLE_SIZE {
            return Err(error(
                DecodeErrorKind::InvalidLength,
                "btree.nonleaf.record_length",
            ));
        }
        let offset = usize::from(slot.offset());
        required_disk_vpid(view, offset, "btree.nonleaf.child")?;
        let key_length = read_i16_be(view, offset + 6, "btree.nonleaf.key_length")?;
        if key_length < -1
            || key_length >= 0
                && u16::try_from(key_length)
                    .ok()
                    .is_none_or(|length| length > slot.length() - NONLEAF_PREAMBLE_SIZE)
        {
            return Err(error(
                DecodeErrorKind::InvalidLength,
                "btree.nonleaf.key_length",
            ));
        }
        if key_length == -1 {
            if slot.length() < NONLEAF_PREAMBLE_SIZE + 6 {
                return Err(error(
                    DecodeErrorKind::InvalidLength,
                    "btree.nonleaf.overflow_key",
                ));
            }
            required_disk_vpid(view, offset + 8, "btree.nonleaf.overflow_key")?;
            overflow_keys = overflow_keys.checked_add(1).ok_or_else(|| {
                error(
                    DecodeErrorKind::ArithmeticOverflow,
                    "btree.nonleaf.overflow_keys",
                )
            })?;
        }
        children = children.checked_add(1).ok_or_else(|| {
            error(
                DecodeErrorKind::ArithmeticOverflow,
                "btree.nonleaf.children",
            )
        })?;
    }
    Ok((children, overflow_keys))
}

fn required_oid(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Oid, DecodeError> {
    let page = read_i32(view, offset, rule)?;
    let slot = read_i16(view, offset + 4, rule)?;
    let volume = read_i16(view, offset + 6, rule)?;
    Ok(Oid::new(
        VolId::new(volume).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        PageId::new(page).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        SlotId::new(slot).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
    ))
}

fn required_disk_vpid(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Vpid, DecodeError> {
    let page = view
        .read_i32_be(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?;
    let volume = view
        .read_i16_be(offset + 4, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?;
    Ok(Vpid::new(
        VolId::new(volume).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        PageId::new(page).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
    ))
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

fn positive_i16(value: i16, rule: &'static str) -> Result<u16, DecodeError> {
    u16::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| error(DecodeErrorKind::OutOfRange, rule))
}

fn non_negative_i16(value: i16, rule: &'static str) -> Result<u16, DecodeError> {
    u16::try_from(value).map_err(|_| error(DecodeErrorKind::NegativeValue, rule))
}

fn non_negative_i32(value: i32, rule: &'static str) -> Result<u32, DecodeError> {
    u32::try_from(value).map_err(|_| error(DecodeErrorKind::NegativeValue, rule))
}

fn read_i16(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i16, DecodeError> {
    view.read_i16_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

fn read_i16_be(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i16, DecodeError> {
    view.read_i16_be(offset, rule)
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

fn read_i64(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i64, DecodeError> {
    view.read_i64_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

fn read_u64(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u64, DecodeError> {
    view.read_u64_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
