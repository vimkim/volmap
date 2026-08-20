//! Byte-layout primitives for CUBRID object records.
//!
//! These translate the pinned engine's `OR_*` accessors: the variable-offset
//! table, the packed string prefix, and the big-endian scalar reads that every
//! record body uses. Page prologues and `boot_dbparm` are native-endian struct
//! images by contrast; everything reached through this module is big-endian
//! (`docs/record-interpretation-research.md` §4.3).
//!
//! Shared by the class-representation parser and the attribute-value decoder so
//! the offset-table and string rules exist in exactly one place.

use crate::bytes::ByteView;

use super::{DecodeError, DecodeErrorKind};

/// Low bits of a variable-offset entry are flags, not offset (feat-oos delta,
/// research §1.4). Every real offset is 4-aligned, which is what frees them.
const OR_VAR_FLAG_MASK: u32 = 0x3;

/// `OR_VAR_BIT_OOS` — the attribute is stored out of row as a 16-byte stub.
pub(super) const OR_VAR_BIT_OOS: u32 = 0x1;

/// `OR_MINIMUM_STRING_LENGTH_FOR_COMPRESSION` — a prefix byte of 255 switches
/// the string header to its compressed-length/decompressed-length form.
const STRING_COMPRESSION_PREFIX: u8 = 255;

/// Largest expansion an LZ4 block can encode, used to bound the buffer a
/// claimed decompressed length may allocate. A record body is at most one page,
/// so this keeps a corrupt length from requesting an unbounded allocation.
const LZ4_MAX_EXPANSION_RATIO: usize = 256;

/// Aligns to a 4-byte boundary the way `DB_ATT_ALIGN` does.
const fn align4(value: usize) -> usize {
    (value + 3) & !3
}

/// `OR_VAR_TABLE_SIZE_INTERNAL` — a `count`-entry table stores `count + 1`
/// offsets so every entry's length is the difference to its successor.
pub(super) fn var_table_size(count: u32, width: u8) -> Result<usize, DecodeError> {
    if count == 0 {
        return Ok(0);
    }
    let entries = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| overflow("object.var_table.count"))?;
    entries
        .checked_mul(usize::from(width))
        .map(align4)
        .ok_or_else(|| overflow("object.var_table.size"))
}

/// Raw entry `index` of the variable-offset table at `table`, flags included.
pub(super) fn var_entry_raw(
    view: &ByteView<'_>,
    table: usize,
    width: u8,
    index: u32,
    rule: &'static str,
) -> Result<u32, DecodeError> {
    let index = usize::try_from(index).map_err(|_| overflow(rule))?;
    let at = index
        .checked_mul(usize::from(width))
        .and_then(|scaled| table.checked_add(scaled))
        .ok_or_else(|| overflow(rule))?;
    match width {
        1 => Ok(u32::from(read_u8(view, at, rule)?)),
        2 => Ok(u32::from(read_u16_be(view, at, rule)?)),
        4 => read_u32_be(view, at, rule),
        _ => Err(DecodeError::new(
            DecodeErrorKind::InvalidGeometry,
            "object.var_table.width",
        )),
    }
}

/// Entry `index` with the flag bits masked off — the body-relative byte offset.
pub(super) fn var_entry_offset(
    view: &ByteView<'_>,
    table: usize,
    width: u8,
    index: u32,
    rule: &'static str,
) -> Result<usize, DecodeError> {
    let raw = var_entry_raw(view, table, width, index, rule)?;
    usize::try_from(raw & !OR_VAR_FLAG_MASK).map_err(|_| overflow(rule))
}

/// Whether a variable-offset width is one the engine can emit.
pub(super) const fn is_supported_offset_width(width: u8) -> bool {
    matches!(width, 1 | 2 | 4)
}

/// The bytes of a packed string, decompressing an LZ4 payload when present.
///
/// `extent` is the attribute's own byte span from the variable-offset table.
/// Every read is bounded by it, not merely by the record: a length prefix that
/// overstates the value must not let the decoder absorb the bytes of the next
/// attribute and present them as this one's value.
///
/// Returns raw bytes rather than text: the codeset that decides how to render
/// them lives in the attribute's domain, not in the value (research §4.4).
pub(super) fn read_packed_string(
    view: &ByteView<'_>,
    offset: usize,
    extent: usize,
    rule: &'static str,
) -> Result<Vec<u8>, DecodeError> {
    let value = view
        .subview(offset, extent, rule)
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))?;
    let prefix = read_u8(&value, 0, rule)?;
    if prefix < STRING_COMPRESSION_PREFIX {
        return Ok(range(&value, 1, usize::from(prefix), rule)?.to_vec());
    }

    let compressed = non_negative(read_i32_be(&value, 1, rule)?, rule)?;
    let decompressed = non_negative(read_i32_be(&value, 5, rule)?, rule)?;
    if compressed == 0 {
        // Compression was skipped or unprofitable; the payload is stored raw.
        return Ok(range(&value, 9, decompressed, rule)?.to_vec());
    }

    // The payload is read first, so `compressed` is proven to lie inside the
    // extent before the claimed decompressed size can size a buffer.
    let payload = range(&value, 9, compressed, rule)?;
    if decompressed > payload.len().saturating_mul(LZ4_MAX_EXPANSION_RATIO) {
        return Err(DecodeError::new(DecodeErrorKind::InvalidLength, rule));
    }
    lz4_flex::block::decompress(payload, decompressed)
        .map_err(|_| DecodeError::new(DecodeErrorKind::InvalidLength, rule))
}

/// Renders string bytes for display. Version one assumes a UTF-8-compatible
/// codeset and converts lossily; the codeset itself travels on the fact so a
/// later revision can transcode without re-reading the volume.
pub(super) fn lossy_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub(super) fn range<'a>(
    view: &ByteView<'a>,
    offset: usize,
    length: usize,
    rule: &'static str,
) -> Result<&'a [u8], DecodeError> {
    view.range(offset, length, rule)
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))
}

pub(super) fn read_u8(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<u8, DecodeError> {
    view.read_u8(offset, rule)
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))
}

pub(super) fn read_i16_be(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<i16, DecodeError> {
    view.read_i16_be(offset, rule)
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))
}

pub(super) fn read_u16_be(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<u16, DecodeError> {
    Ok(u16::from_be_bytes(array(view, offset, rule)?))
}

pub(super) fn read_i32_be(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<i32, DecodeError> {
    view.read_i32_be(offset, rule)
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))
}

pub(super) fn read_u32_be(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<u32, DecodeError> {
    Ok(u32::from_be_bytes(array(view, offset, rule)?))
}

pub(super) fn read_i64_be(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<i64, DecodeError> {
    Ok(i64::from_be_bytes(array(view, offset, rule)?))
}

pub(super) fn read_f32_be(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<f32, DecodeError> {
    Ok(f32::from_be_bytes(array(view, offset, rule)?))
}

pub(super) fn read_f64_be(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<f64, DecodeError> {
    Ok(f64::from_be_bytes(array(view, offset, rule)?))
}

fn array<const N: usize>(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<[u8; N], DecodeError> {
    range(view, offset, N, rule)?
        .try_into()
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, rule))
}

pub(super) fn non_negative(value: i32, rule: &'static str) -> Result<usize, DecodeError> {
    usize::try_from(value).map_err(|_| DecodeError::new(DecodeErrorKind::NegativeValue, rule))
}

fn overflow(rule: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::ArithmeticOverflow, rule)
}
