//! Pinned x86-64/GCC `DISK_VOLUME_HEADER` decoder.
//!
//! Authority: CUBRID commit `e1e651debf6cc100172bde96603b17424f9c135a`,
//! `src/storage/disk_manager.c` (`DISK_VOLUME_HEADER`,
//! `disk_volume_header_set_stab`, and `disk_verify_volume_header`).

use crate::bytes::ByteView;
use crate::model::{FileId, Hfid, PageId, SectorId, Vfid, VolId};

use super::{
    DB_PAGE_SIZE, DecodeError, DecodeErrorKind, DecodedPageEnvelope, IO_PAGE_SIZE, PageType,
};

const HEADER_FIXED_SIZE: usize = 132;
const MAGIC_SIZE: usize = 25;
const MAGIC: &[u8; MAGIC_SIZE] = b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0";
const SECTOR_PAGES: u32 = 64;
const BITMAP_SECTORS_PER_PAGE: u32 = 130_752;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VolumePurpose {
    PermanentData,
    TemporaryData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VolumeType {
    Permanent,
    Temporary,
}

/// A NUL-terminated field validated inside its immediate header container.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ValidatedCString<'a> {
    bytes_without_nul: &'a [u8],
}

impl core::fmt::Debug for ValidatedCString<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ValidatedCString")
            .field("len", &self.bytes_without_nul.len())
            .finish()
    }
}

impl<'a> ValidatedCString<'a> {
    #[must_use]
    pub const fn as_bytes(&self) -> &'a [u8] {
        self.bytes_without_nul
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VolumeHeader<'a> {
    vol_id: VolId,
    purpose: VolumePurpose,
    volume_type: VolumeType,
    database_charset: u8,
    total_sectors: u32,
    maximum_sectors: u32,
    allocation_hint: Option<SectorId>,
    bitmap_page_count: u32,
    system_last_page: PageId,
    database_creation: i64,
    volume_creation: i64,
    checkpoint_lsa_word: u64,
    boot_hfid: Option<Hfid>,
    next_vol_id: Option<VolId>,
    current_volume_name: ValidatedCString<'a>,
    next_volume_name: ValidatedCString<'a>,
    remarks: ValidatedCString<'a>,
}

impl<'a> VolumeHeader<'a> {
    #[must_use]
    pub const fn vol_id(&self) -> VolId {
        self.vol_id
    }

    #[must_use]
    pub const fn purpose(&self) -> VolumePurpose {
        self.purpose
    }

    #[must_use]
    pub const fn volume_type(&self) -> VolumeType {
        self.volume_type
    }

    #[must_use]
    pub const fn database_charset(&self) -> u8 {
        self.database_charset
    }

    #[must_use]
    pub const fn total_sectors(&self) -> u32 {
        self.total_sectors
    }

    #[must_use]
    pub const fn maximum_sectors(&self) -> u32 {
        self.maximum_sectors
    }

    #[must_use]
    pub const fn allocation_hint(&self) -> Option<SectorId> {
        self.allocation_hint
    }

    #[must_use]
    pub const fn bitmap_page_count(&self) -> u32 {
        self.bitmap_page_count
    }

    #[must_use]
    pub const fn system_last_page(&self) -> PageId {
        self.system_last_page
    }

    #[must_use]
    pub const fn database_creation(&self) -> i64 {
        self.database_creation
    }

    #[must_use]
    pub const fn volume_creation(&self) -> i64 {
        self.volume_creation
    }

    #[must_use]
    pub const fn checkpoint_lsa_word(&self) -> u64 {
        self.checkpoint_lsa_word
    }

    #[must_use]
    pub const fn boot_hfid(&self) -> Option<Hfid> {
        self.boot_hfid
    }

    #[must_use]
    pub const fn next_vol_id(&self) -> Option<VolId> {
        self.next_vol_id
    }

    #[must_use]
    pub const fn current_volume_name(&self) -> ValidatedCString<'a> {
        self.current_volume_name
    }

    #[must_use]
    pub const fn next_volume_name(&self) -> ValidatedCString<'a> {
        self.next_volume_name
    }

    #[must_use]
    pub const fn remarks(&self) -> ValidatedCString<'a> {
        self.remarks
    }
}

#[allow(clippy::too_many_lines)] // One fail-closed validation flow keeps prerequisite order local.
pub fn decode_volume_header<'a>(
    page: &DecodedPageEnvelope<'a>,
    file_length: u64,
) -> Result<VolumeHeader<'a>, DecodeError> {
    if page.page_type() != PageType::VolumeHeader || page.id().page_id.get() != 0 {
        return Err(DecodeError::new(
            DecodeErrorKind::WrongPageType,
            "volume.header.page_type",
        ));
    }
    let bytes = page.plaintext("volume.header.plaintext")?;

    if bytes
        .range(0, MAGIC_SIZE, "volume magic")
        .map_err(|_| access("volume.header.magic"))?
        != MAGIC
    {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidMagic,
            "volume.header.magic",
        ));
    }

    let io_page_size = bytes
        .read_i16_le(26, "I/O page size")
        .map_err(|_| access("volume.header.io_page_size"))?;
    let raw_vol_id = bytes
        .read_i16_le(28, "volume identifier")
        .map_err(|_| access("volume.header.vol_id"))?;
    if io_page_size != 16_384 || raw_vol_id != page.id().vol_id.get() {
        return Err(DecodeError::new(
            DecodeErrorKind::IdentityMismatch,
            "volume.header.identity_and_profile",
        ));
    }

    let purpose = decode_purpose(read_i32(&bytes, 32, "volume purpose")?)?;
    let volume_type = decode_volume_type(read_i32(&bytes, 36, "volume type")?)?;
    if purpose == VolumePurpose::PermanentData && volume_type == VolumeType::Temporary {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "volume.header.purpose_type_pair",
        ));
    }

    let sectors_per_volume_sector = read_positive_u32(&bytes, 40, "pages per sector")?;
    let total_sectors = read_positive_u32(&bytes, 44, "total sectors")?;
    let maximum_sectors = read_positive_u32(&bytes, 48, "maximum sectors")?;
    let bitmap_page_count = read_positive_u32(&bytes, 56, "bitmap page count")?;
    let bitmap_first_page = read_non_negative_u32(&bytes, 60, "bitmap first page")?;
    let system_last_page = read_non_negative_u32(&bytes, 64, "last system page")?;

    let expected_bitmap_pages = maximum_sectors
        .checked_add(BITMAP_SECTORS_PER_PAGE - 1)
        .map(|rounded| rounded / BITMAP_SECTORS_PER_PAGE)
        .ok_or_else(|| arithmetic("volume.header.bitmap_page_count"))?;
    let expected_system_last = bitmap_first_page
        .checked_add(bitmap_page_count)
        .and_then(|end| end.checked_sub(1))
        .ok_or_else(|| arithmetic("volume.header.system_last_page"))?;

    if sectors_per_volume_sector != SECTOR_PAGES
        || total_sectors > maximum_sectors
        || total_sectors % 64 != 0
        || maximum_sectors % 64 != 0
        || bitmap_first_page != 1
        || bitmap_page_count != expected_bitmap_pages
        || system_last_page != expected_system_last
    {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "volume.header.geometry",
        ));
    }

    validate_file_length(file_length, total_sectors)?;

    let allocation_hint = optional_sector(read_i32(&bytes, 52, "allocation hint")?)?;
    let database_creation = bytes
        .read_i64_le(72, "database creation")
        .map_err(|_| access("volume.header.database_creation"))?;
    let volume_creation = bytes
        .read_i64_le(80, "volume creation")
        .map_err(|_| access("volume.header.volume_creation"))?;
    let checkpoint_lsa_word = bytes
        .read_u64_le(88, "checkpoint LSA")
        .map_err(|_| access("volume.header.checkpoint_lsa"))?;
    let boot_hfid = decode_optional_hfid(&bytes)?;
    let next_vol_id = optional_vol_id(
        bytes
            .read_i16_le(124, "next volume identifier")
            .map_err(|_| access("volume.header.next_vol_id"))?,
    )?;

    let current_offset = read_string_offset(&bytes, 126, "current volume name offset")?;
    let next_offset = read_string_offset(&bytes, 128, "next volume name offset")?;
    let remarks_offset = read_string_offset(&bytes, 130, "remarks offset")?;
    if current_offset > next_offset || next_offset > remarks_offset {
        return Err(strings("volume.header.string_offset_order"));
    }
    let current_volume_name = c_string(&bytes, current_offset, next_offset)?;
    let next_volume_name = c_string(&bytes, next_offset, remarks_offset)?;
    let remarks = c_string(&bytes, remarks_offset, DB_PAGE_SIZE - HEADER_FIXED_SIZE)?;

    Ok(VolumeHeader {
        vol_id: page.id().vol_id,
        purpose,
        volume_type,
        database_charset: bytes
            .read_u8(30, "database charset")
            .map_err(|_| access("volume.header.charset"))?,
        total_sectors,
        maximum_sectors,
        allocation_hint,
        bitmap_page_count,
        system_last_page: PageId::new(
            i32::try_from(system_last_page)
                .map_err(|_| arithmetic("volume.header.system_last_page"))?,
        )
        .map_err(|_| geometry("volume.header.system_last_page"))?,
        database_creation,
        volume_creation,
        checkpoint_lsa_word,
        boot_hfid,
        next_vol_id,
        current_volume_name,
        next_volume_name,
        remarks,
    })
}

fn validate_file_length(file_length: u64, total_sectors: u32) -> Result<(), DecodeError> {
    let required = u64::from(total_sectors)
        .checked_mul(u64::from(SECTOR_PAGES))
        .and_then(|pages| pages.checked_mul(IO_PAGE_SIZE as u64))
        .ok_or_else(|| arithmetic("volume.header.file_length_arithmetic"))?;
    if !file_length.is_multiple_of(16_384) || file_length < required {
        return Err(DecodeError::new(
            DecodeErrorKind::FileLengthInvalid,
            "volume.header.file_length",
        ));
    }
    Ok(())
}

fn decode_purpose(value: i32) -> Result<VolumePurpose, DecodeError> {
    match value {
        0 => Ok(VolumePurpose::PermanentData),
        1 => Ok(VolumePurpose::TemporaryData),
        _ => Err(DecodeError::new(
            DecodeErrorKind::UnknownEnum,
            "volume.header.purpose_known",
        )),
    }
}

fn decode_volume_type(value: i32) -> Result<VolumeType, DecodeError> {
    match value {
        0 => Ok(VolumeType::Permanent),
        1 => Ok(VolumeType::Temporary),
        _ => Err(DecodeError::new(
            DecodeErrorKind::UnknownEnum,
            "volume.header.type_known",
        )),
    }
}

fn decode_optional_hfid(bytes: &ByteView<'_>) -> Result<Option<Hfid>, DecodeError> {
    let file_id = read_i32(bytes, 96, "boot file identifier")?;
    let vol_id = bytes
        .read_i16_le(100, "boot volume identifier")
        .map_err(|_| access("volume.header.boot_hfid.vol_id"))?;
    let page_id = read_i32(bytes, 104, "boot header page identifier")?;
    if file_id == -1 && vol_id == -1 && page_id == -1 {
        return Ok(None);
    }
    let vol_id = VolId::new(vol_id).map_err(|_| geometry("volume.header.boot_hfid"))?;
    let file_id = FileId::new(file_id).map_err(|_| geometry("volume.header.boot_hfid"))?;
    let page_id = PageId::new(page_id).map_err(|_| geometry("volume.header.boot_hfid"))?;
    Ok(Some(Hfid::new(Vfid::new(vol_id, file_id), page_id)))
}

fn optional_sector(value: i32) -> Result<Option<SectorId>, DecodeError> {
    if value == -1 {
        return Ok(None);
    }
    SectorId::new(value)
        .map(Some)
        .map_err(|_| geometry("volume.header.allocation_hint"))
}

fn optional_vol_id(value: i16) -> Result<Option<VolId>, DecodeError> {
    if value == -1 {
        return Ok(None);
    }
    VolId::new(value)
        .map(Some)
        .map_err(|_| geometry("volume.header.next_vol_id"))
}

fn read_string_offset(
    bytes: &ByteView<'_>,
    offset: usize,
    field: &'static str,
) -> Result<usize, DecodeError> {
    let value = bytes
        .read_i16_le(offset, field)
        .map_err(|_| access(field))?;
    usize::try_from(value).map_err(|_| strings("volume.header.string_offset_non_negative"))
}

fn c_string<'a>(
    bytes: &ByteView<'a>,
    relative_start: usize,
    relative_end: usize,
) -> Result<ValidatedCString<'a>, DecodeError> {
    if relative_start > relative_end {
        return Err(strings("volume.header.string_range_order"));
    }
    let length = relative_end
        .checked_sub(relative_start)
        .ok_or_else(|| arithmetic("volume.header.string_range"))?;
    let absolute_start = HEADER_FIXED_SIZE
        .checked_add(relative_start)
        .ok_or_else(|| arithmetic("volume.header.string_start"))?;
    let field = bytes
        .range(absolute_start, length, "volume header string")
        .map_err(|_| strings("volume.header.string_range"))?;
    let nul = field
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| strings("volume.header.string_terminator"))?;
    Ok(ValidatedCString {
        bytes_without_nul: &field[..nul],
    })
}

fn read_i32(bytes: &ByteView<'_>, offset: usize, field: &'static str) -> Result<i32, DecodeError> {
    bytes.read_i32_le(offset, field).map_err(|_| access(field))
}

fn read_non_negative_u32(
    bytes: &ByteView<'_>,
    offset: usize,
    field: &'static str,
) -> Result<u32, DecodeError> {
    u32::try_from(read_i32(bytes, offset, field)?).map_err(|_| geometry(field))
}

fn read_positive_u32(
    bytes: &ByteView<'_>,
    offset: usize,
    field: &'static str,
) -> Result<u32, DecodeError> {
    let value = read_non_negative_u32(bytes, offset, field)?;
    if value == 0 {
        return Err(geometry(field));
    }
    Ok(value)
}

const fn access(rule: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::ByteAccess, rule)
}

const fn arithmetic(rule: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::ArithmeticOverflow, rule)
}

const fn geometry(rule: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::InvalidGeometry, rule)
}

const fn strings(rule: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::InvalidStringTable, rule)
}
