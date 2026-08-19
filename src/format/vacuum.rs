use crate::bytes::ByteView;
use crate::model::{FileId, PageId, Vfid, VolId, Vpid};

use super::{DB_PAGE_SIZE, DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType};

const RAW_PAGE_HEADER_SIZE: usize = 16;
const VACUUM_ENTRY_SIZE: usize = 32;
const DROPPED_ENTRY_SIZE: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VacuumEntryFact {
    pub block_id: u64,
    pub flags: u64,
    pub start_lsa_word: u64,
    pub oldest_visible_mvccid: u64,
    pub newest_mvccid: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VacuumPageFact {
    pub next: Option<Vpid>,
    pub index_unvacuumed: Option<u16>,
    pub index_free: u16,
    pub entries: Vec<VacuumEntryFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DroppedFileFact {
    pub vfid: Vfid,
    pub mvccid: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DroppedFilesPageFact {
    pub next: Option<Vpid>,
    pub entries: Vec<DroppedFileFact>,
}

pub fn decode_vacuum_page(
    envelope: &DecodedPageEnvelope<'_>,
) -> Result<VacuumPageFact, DecodeError> {
    if envelope.page_type() != PageType::VacuumData {
        return Err(error(DecodeErrorKind::WrongPageType, "vacuum.page.type"));
    }
    let view = envelope.plaintext("vacuum.page.encrypted")?;
    let next = optional_vpid(&view, 0, "vacuum.page.next")?;
    let raw_unvacuumed = read_i16(&view, 8, "vacuum.page.index_unvacuumed")?;
    let raw_free = read_i16(&view, 10, "vacuum.page.index_free")?;
    let index_free = u16::try_from(raw_free)
        .map_err(|_| error(DecodeErrorKind::NegativeValue, "vacuum.page.index_free"))?;
    let capacity = (DB_PAGE_SIZE - RAW_PAGE_HEADER_SIZE) / VACUUM_ENTRY_SIZE;
    if usize::from(index_free) > capacity {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "vacuum.page.index_free",
        ));
    }
    let index_unvacuumed = match raw_unvacuumed {
        -1 => None,
        value => {
            let value = u16::try_from(value).map_err(|_| {
                error(
                    DecodeErrorKind::NegativeValue,
                    "vacuum.page.index_unvacuumed",
                )
            })?;
            if value > index_free {
                return Err(error(
                    DecodeErrorKind::InvalidGeometry,
                    "vacuum.page.index_order",
                ));
            }
            Some(value)
        }
    };
    let entries = (0..index_free)
        .map(|index| {
            let offset = RAW_PAGE_HEADER_SIZE + usize::from(index) * VACUUM_ENTRY_SIZE;
            let packed = read_u64(&view, offset, "vacuum.entry.block")?;
            let status = packed & 0xc000_0000_0000_0000;
            if status == 0xc000_0000_0000_0000 {
                return Err(error(DecodeErrorKind::InvalidFlags, "vacuum.entry.status"));
            }
            Ok(VacuumEntryFact {
                block_id: packed & 0x1fff_ffff_ffff_ffff,
                flags: packed & 0xe000_0000_0000_0000,
                start_lsa_word: read_u64(&view, offset + 8, "vacuum.entry.start_lsa")?,
                oldest_visible_mvccid: read_u64(&view, offset + 16, "vacuum.entry.oldest_mvccid")?,
                newest_mvccid: read_u64(&view, offset + 24, "vacuum.entry.newest_mvccid")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(VacuumPageFact {
        next,
        index_unvacuumed,
        index_free,
        entries,
    })
}

pub fn decode_dropped_files_page(
    envelope: &DecodedPageEnvelope<'_>,
) -> Result<DroppedFilesPageFact, DecodeError> {
    if envelope.page_type() != PageType::DroppedFiles {
        return Err(error(DecodeErrorKind::WrongPageType, "dropped.page.type"));
    }
    let view = envelope.plaintext("dropped.page.encrypted")?;
    let next = optional_vpid(&view, 0, "dropped.page.next")?;
    let count = u16::try_from(read_i16(&view, 8, "dropped.page.count")?)
        .map_err(|_| error(DecodeErrorKind::NegativeValue, "dropped.page.count"))?;
    let capacity = (DB_PAGE_SIZE - RAW_PAGE_HEADER_SIZE) / DROPPED_ENTRY_SIZE;
    if usize::from(count) > capacity {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "dropped.page.count",
        ));
    }
    let entries = (0..count)
        .map(|index| {
            let offset = RAW_PAGE_HEADER_SIZE + usize::from(index) * DROPPED_ENTRY_SIZE;
            let file_id = read_i32(&view, offset, "dropped.entry.file")?;
            let vol_id = read_i16(&view, offset + 4, "dropped.entry.volume")?;
            Ok(DroppedFileFact {
                vfid: Vfid::new(
                    VolId::new(vol_id)
                        .map_err(|_| error(DecodeErrorKind::OutOfRange, "dropped.entry.volume"))?,
                    FileId::new(file_id)
                        .map_err(|_| error(DecodeErrorKind::OutOfRange, "dropped.entry.file"))?,
                ),
                mvccid: read_u64(&view, offset + 8, "dropped.entry.mvccid")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DroppedFilesPageFact { next, entries })
}

fn optional_vpid(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Option<Vpid>, DecodeError> {
    let page_id = read_i32(view, offset, rule)?;
    let vol_id = read_i16(view, offset + 4, rule)?;
    if page_id == -1 && vol_id == -1 {
        return Ok(None);
    }
    if page_id < 0 || vol_id < 0 {
        return Err(error(DecodeErrorKind::InvalidGeometry, rule));
    }
    Ok(Some(Vpid::new(
        VolId::new(vol_id).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        PageId::new(page_id).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
    )))
}

fn read_i16(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i16, DecodeError> {
    view.read_i16_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

fn read_i32(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<i32, DecodeError> {
    view.read_i32_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

fn read_u64(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Result<u64, DecodeError> {
    view.read_u64_le(offset, rule)
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
