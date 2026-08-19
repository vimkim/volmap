use crate::bytes::ByteView;
use crate::model::{FileId, Oid, PageId, SlotId, Vfid, VolId, Vpid};

use super::{DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType, RecordType, SlottedPage};

const PAGE_HEADER_SIZE: u16 = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogPageFact {
    pub next_overflow: Option<Vpid>,
    pub directory_count: u32,
    pub is_overflow: bool,
    pub record_count: u16,
    pub record_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogRepresentationItemFact {
    pub target: Oid,
    pub representation_id: i16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogDirectoryFact {
    pub items: Vec<CatalogRepresentationItemFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogRepresentationHeaderFact {
    pub representation_id: i32,
    pub fixed_count: u32,
    pub fixed_length: u32,
    pub variable_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogClassInfoFact {
    pub heap_file: Option<Vfid>,
    pub heap_header: Option<Vpid>,
    pub total_pages: u32,
    pub total_objects: u32,
    pub representation_directory: Oid,
}

pub fn decode_catalog_class_info(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    slot_id: u16,
) -> Result<CatalogClassInfoFact, DecodeError> {
    let (view, base, length) = catalog_record(envelope, slotted, slot_id, "catalog.class_info")?;
    if length != 56 {
        return Err(error(
            DecodeErrorKind::InvalidLength,
            "catalog.class_info.length",
        ));
    }
    let heap_page = read_i32_be(&view, base, "catalog.class_info.heap")?;
    let heap_file = read_i32_be(&view, base + 4, "catalog.class_info.heap")?;
    let heap_volume = read_i32_be(&view, base + 8, "catalog.class_info.heap")?;
    let heap = if heap_file == -1 {
        if heap_page != -1 {
            return Err(error(
                DecodeErrorKind::InvalidGeometry,
                "catalog.class_info.heap",
            ));
        }
        // HFID_IS_NULL is defined by NULL_FILEID. CUBRID's null setter also
        // clears hpgid but deliberately leaves the unused volume word alone.
        None
    } else {
        if heap_page < 0 || heap_file < 0 || heap_volume < 0 {
            return Err(error(
                DecodeErrorKind::InvalidGeometry,
                "catalog.class_info.heap",
            ));
        }
        let heap_volume = i16::try_from(heap_volume)
            .map_err(|_| error(DecodeErrorKind::OutOfRange, "catalog.class_info.heap"))?;
        let heap_volume = VolId::new(heap_volume)
            .map_err(|_| error(DecodeErrorKind::OutOfRange, "catalog.class_info.heap"))?;
        let heap_file = FileId::new(heap_file)
            .map_err(|_| error(DecodeErrorKind::OutOfRange, "catalog.class_info.heap"))?;
        let heap_page = PageId::new(heap_page)
            .map_err(|_| error(DecodeErrorKind::OutOfRange, "catalog.class_info.heap"))?;
        Some((
            Vfid::new(heap_volume, heap_file),
            Vpid::new(heap_volume, heap_page),
        ))
    };
    Ok(CatalogClassInfoFact {
        heap_file: heap.map(|value| value.0),
        heap_header: heap.map(|value| value.1),
        total_pages: non_negative_i32(
            read_i32_be(&view, base + 12, "catalog.class_info.total_pages")?,
            "catalog.class_info.total_pages",
        )?,
        total_objects: non_negative_i32(
            read_i32_be(&view, base + 16, "catalog.class_info.total_objects")?,
            "catalog.class_info.total_objects",
        )?,
        representation_directory: required_oid(
            read_i32_be(&view, base + 24, "catalog.class_info.directory")?,
            read_i16_be(&view, base + 28, "catalog.class_info.directory")?,
            read_i16_be(&view, base + 30, "catalog.class_info.directory")?,
            "catalog.class_info.directory",
        )?,
    })
}

pub fn decode_catalog_directory(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    slot_id: u16,
) -> Result<CatalogDirectoryFact, DecodeError> {
    let (view, base, length) = catalog_record(envelope, slotted, slot_id, "catalog.directory")?;
    if length != 32 {
        return Err(error(
            DecodeErrorKind::InvalidLength,
            "catalog.directory.length",
        ));
    }
    let count = view
        .read_u8(base + 12, "catalog.directory.count")
        .map_err(|_| error(DecodeErrorKind::ByteAccess, "catalog.directory.count"))?;
    if !matches!(count, 1 | 2) {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "catalog.directory.count",
        ));
    }
    let items = (0..count)
        .map(|index| {
            let offset = base + usize::from(index) * 16;
            let page = read_i32_be(&view, offset, "catalog.directory.target")?;
            let volume = read_i16_be(&view, offset + 4, "catalog.directory.target")?;
            let representation_id =
                read_i16_be(&view, offset + 8, "catalog.directory.representation_id")?;
            let slot = read_i16_be(&view, offset + 10, "catalog.directory.target")?;
            Ok(CatalogRepresentationItemFact {
                target: required_oid(page, slot, volume, "catalog.directory.target")?,
                representation_id,
            })
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;
    Ok(CatalogDirectoryFact { items })
}

pub fn decode_catalog_representation_header(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
    slot_id: u16,
) -> Result<CatalogRepresentationHeaderFact, DecodeError> {
    let (view, base, length) =
        catalog_record(envelope, slotted, slot_id, "catalog.representation")?;
    if length < 56 {
        return Err(error(
            DecodeErrorKind::InvalidLength,
            "catalog.representation.length",
        ));
    }
    let representation_id = read_i32_be(&view, base, "catalog.representation.representation_id")?;
    if representation_id < 0 {
        return Err(error(
            DecodeErrorKind::NegativeValue,
            "catalog.representation.representation_id",
        ));
    }
    Ok(CatalogRepresentationHeaderFact {
        representation_id,
        fixed_count: non_negative_i32(
            read_i32_be(&view, base + 4, "catalog.representation.fixed_count")?,
            "catalog.representation.fixed_count",
        )?,
        fixed_length: non_negative_i32(
            read_i32_be(&view, base + 8, "catalog.representation.fixed_length")?,
            "catalog.representation.fixed_length",
        )?,
        variable_count: non_negative_i32(
            read_i32_be(&view, base + 12, "catalog.representation.variable_count")?,
            "catalog.representation.variable_count",
        )?,
    })
}

pub fn decode_catalog_page(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
) -> Result<CatalogPageFact, DecodeError> {
    if envelope.page_type() != PageType::Catalog {
        return Err(error(DecodeErrorKind::WrongPageType, "catalog.page.type"));
    }
    let header = slotted
        .slots()
        .first()
        .filter(|slot| {
            slot.slot_id() == 0
                && slot.record_type() == RecordType::Home
                && slot.offset() != 0
                && slot.length() == PAGE_HEADER_SIZE
        })
        .ok_or_else(|| error(DecodeErrorKind::InvalidGeometry, "catalog.page.header_slot"))?;
    let view = envelope.plaintext("catalog.page.encrypted")?;
    let base = usize::from(header.offset());
    let directory_count = non_negative_i32(
        read_i32_be(&view, base + 8, "catalog.page.directory_count")?,
        "catalog.page.directory_count",
    )?;
    let is_overflow = match read_i32_be(&view, base + 12, "catalog.page.overflow_flag")? {
        0 => false,
        1 => true,
        _ => {
            return Err(error(
                DecodeErrorKind::InvalidFlags,
                "catalog.page.overflow_flag",
            ));
        }
    };
    let mut record_count = 0_u16;
    let mut record_bytes = 0_u32;
    for slot in slotted
        .slots()
        .iter()
        .skip(1)
        .filter(|slot| !slot.is_empty())
    {
        if slot.record_type() != RecordType::Home {
            return Err(error(
                DecodeErrorKind::InvalidGeometry,
                "catalog.page.record_type",
            ));
        }
        record_count = record_count
            .checked_add(1)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "catalog.page.records"))?;
        record_bytes = record_bytes
            .checked_add(u32::from(slot.length()))
            .ok_or_else(|| {
                error(
                    DecodeErrorKind::ArithmeticOverflow,
                    "catalog.page.record_bytes",
                )
            })?;
    }
    if directory_count > u32::from(record_count) || is_overflow && directory_count != 0 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "catalog.page.directory_count",
        ));
    }
    Ok(CatalogPageFact {
        next_overflow: optional_disk_vpid(&view, base, "catalog.page.next_overflow")?,
        directory_count,
        is_overflow,
        record_count,
        record_bytes,
    })
}

fn optional_disk_vpid(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Option<Vpid>, DecodeError> {
    let page = view
        .read_i32_be(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?;
    let volume = view
        .read_i16_be(offset + 4, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?;
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

fn catalog_record<'a>(
    envelope: &'a DecodedPageEnvelope<'a>,
    slotted: &SlottedPage,
    slot_id: u16,
    rule: &'static str,
) -> Result<(ByteView<'a>, usize, u16), DecodeError> {
    if envelope.page_type() != PageType::Catalog {
        return Err(error(DecodeErrorKind::WrongPageType, rule));
    }
    let slot = slotted
        .slots()
        .get(usize::from(slot_id))
        .filter(|slot| slot.record_type() == RecordType::Home && !slot.is_empty())
        .ok_or_else(|| error(DecodeErrorKind::InvalidGeometry, rule))?;
    Ok((
        envelope.plaintext("catalog.record.encrypted")?,
        usize::from(slot.offset()),
        slot.length(),
    ))
}

fn required_oid(page: i32, slot: i16, volume: i16, rule: &'static str) -> Result<Oid, DecodeError> {
    if page < 0 || slot < 0 || volume < 0 {
        return Err(error(DecodeErrorKind::InvalidGeometry, rule));
    }
    Ok(Oid::new(
        VolId::new(volume).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        PageId::new(page).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        SlotId::new(slot).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
    ))
}

fn non_negative_i32(value: i32, rule: &'static str) -> Result<u32, DecodeError> {
    u32::try_from(value).map_err(|_| error(DecodeErrorKind::NegativeValue, rule))
}

fn read_i32_be(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i32, DecodeError> {
    view.read_i32_be(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

fn read_i16_be(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i16, DecodeError> {
    view.read_i16_be(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
