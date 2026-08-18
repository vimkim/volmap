//! Pinned sector-allocation bitmap decoder.
//!
//! Authority: CUBRID commit `e1e651debf6cc100172bde96603b17424f9c135a`,
//! `src/storage/disk_manager.c` (`DISK_STAB_UNIT` and allocation-table macros).

use crate::bytes::ByteView;
use crate::model::SectorId;

use super::{DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType, VolumeHeader};

const BITS_PER_WORD: u32 = 64;
const SECTORS_PER_BITMAP_PAGE: u32 = 130_752;

#[derive(Clone, Copy, Debug)]
pub struct SectorBitmap<'a> {
    bytes: ByteView<'a>,
    first_sector: SectorId,
    sector_count: u32,
}

impl SectorBitmap<'_> {
    #[must_use]
    pub const fn first_sector(&self) -> SectorId {
        self.first_sector
    }

    #[must_use]
    pub const fn sector_count(&self) -> u32 {
        self.sector_count
    }

    pub fn is_reserved(&self, sector: SectorId) -> Result<bool, DecodeError> {
        let relative = sector
            .get()
            .checked_sub(self.first_sector.get())
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value < self.sector_count)
            .ok_or_else(|| {
                DecodeError::new(DecodeErrorKind::OutOfRange, "sector.bitmap.query_range")
            })?;
        let word_index = relative / BITS_PER_WORD;
        let byte_offset = usize::try_from(word_index)
            .ok()
            .and_then(|word| word.checked_mul(size_of::<u64>()))
            .ok_or_else(|| {
                DecodeError::new(
                    DecodeErrorKind::ArithmeticOverflow,
                    "sector.bitmap.word_offset",
                )
            })?;
        let word = self
            .bytes
            .read_u64_le(byte_offset, "sector bitmap word")
            .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, "sector.bitmap.word"))?;
        Ok(word & (1_u64 << (relative % BITS_PER_WORD)) != 0)
    }
}

pub fn decode_sector_bitmap<'a>(
    page: &DecodedPageEnvelope<'a>,
    header: &VolumeHeader<'_>,
    bitmap_page_index: u32,
) -> Result<SectorBitmap<'a>, DecodeError> {
    if bitmap_page_index >= header.bitmap_page_count() {
        return Err(DecodeError::new(
            DecodeErrorKind::OutOfRange,
            "sector.bitmap.page_index",
        ));
    }
    let expected_page_id = bitmap_page_index
        .checked_add(1)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| {
            DecodeError::new(DecodeErrorKind::ArithmeticOverflow, "sector.bitmap.page_id")
        })?;
    if page.page_type() != PageType::VolumeBitmap
        || page.id().vol_id != header.vol_id()
        || page.id().page_id.get() != expected_page_id
    {
        return Err(DecodeError::new(
            DecodeErrorKind::WrongPageType,
            "sector.bitmap.page_identity_and_type",
        ));
    }

    let first = bitmap_page_index
        .checked_mul(SECTORS_PER_BITMAP_PAGE)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::ArithmeticOverflow,
                "sector.bitmap.first_sector",
            )
        })?;
    let sector_count = if first >= header.total_sectors() {
        0
    } else {
        header
            .total_sectors()
            .checked_sub(first)
            .ok_or_else(|| {
                DecodeError::new(
                    DecodeErrorKind::ArithmeticOverflow,
                    "sector.bitmap.sector_count",
                )
            })?
            .min(SECTORS_PER_BITMAP_PAGE)
    };
    let first_sector = SectorId::new(i32::try_from(first).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::ArithmeticOverflow,
            "sector.bitmap.first_sector",
        )
    })?)
    .map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "sector.bitmap.first_sector",
        )
    })?;

    Ok(SectorBitmap {
        bytes: page.plaintext("sector.bitmap.plaintext")?,
        first_sector,
        sector_count,
    })
}
