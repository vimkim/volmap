use crate::bytes::ByteView;
use crate::model::{PageId, VolId, Vpid};

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

fn non_negative_i32(value: i32, rule: &'static str) -> Result<u32, DecodeError> {
    u32::try_from(value).map_err(|_| error(DecodeErrorKind::NegativeValue, rule))
}

fn read_i32_be(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i32, DecodeError> {
    view.read_i32_be(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
