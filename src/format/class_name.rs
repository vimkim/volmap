//! Decoder for the stored name in a CUBRID class-object record body.

use crate::bytes::ByteView;

use super::object_layout::{
    OR_VAR_BIT_OOS, read_packed_string_with_length, var_entry_offset, var_entry_raw, var_table_size,
};
use super::{DecodeError, DecodeErrorKind};

const CLASS_OFFSET_WIDTH: u8 = 4;
const CLASS_VARIABLE_ATTRIBUTE_COUNT: u32 = 17;
/// Fixed fields end with the four-byte TDE algorithm at body offset 84.
const CLASS_FIXED_REGION_SIZE: usize = 88;
const CLASS_NAME_INDEX: u32 = 0;
const MAX_IDENTIFIER_LENGTH: usize = 255;

/// Decodes variable attribute zero from one validated class-record body.
///
/// The caller is responsible for validating and removing the heap object
/// header. This decoder owns the class offset-table and packed-VARCHAR rules.
pub fn decode_class_record_name(
    body: &[u8],
    variable_offset_width: u8,
) -> Result<Vec<u8>, DecodeError> {
    if variable_offset_width != CLASS_OFFSET_WIDTH {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "class.name.offset_width",
        ));
    }
    let view = ByteView::new(body, 0);
    let table_size = var_table_size(CLASS_VARIABLE_ATTRIBUTE_COUNT, CLASS_OFFSET_WIDTH)?;
    let variable_region = table_size
        .checked_add(CLASS_FIXED_REGION_SIZE)
        .ok_or_else(|| {
            error(
                DecodeErrorKind::ArithmeticOverflow,
                "class.name.fixed_region",
            )
        })?;
    let raw = var_entry_raw(
        &view,
        0,
        CLASS_OFFSET_WIDTH,
        CLASS_NAME_INDEX,
        "class.name.var_table",
    )?;
    if raw & OR_VAR_BIT_OOS != 0 {
        return Err(error(
            DecodeErrorKind::InvalidFlags,
            "class.name.out_of_row",
        ));
    }
    let start = var_entry_offset(
        &view,
        0,
        CLASS_OFFSET_WIDTH,
        CLASS_NAME_INDEX,
        "class.name.var_table",
    )?;
    let end = var_entry_offset(
        &view,
        0,
        CLASS_OFFSET_WIDTH,
        CLASS_NAME_INDEX + 1,
        "class.name.var_table",
    )?;
    if start < variable_region || end <= start || end > body.len() {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "class.name.var_table_order",
        ));
    }
    let extent = end - start;
    let packed = read_packed_string_with_length(
        &view,
        start,
        extent,
        MAX_IDENTIFIER_LENGTH,
        "class.name.varchar",
    )?;
    if packed.bytes.is_empty()
        || packed.bytes.len() > MAX_IDENTIFIER_LENGTH
        || packed.bytes.contains(&0)
    {
        return Err(error(
            DecodeErrorKind::InvalidLength,
            "class.name.identifier",
        ));
    }
    if packed.encoded_length >= extent || body[start + packed.encoded_length] != 0 {
        return Err(error(
            DecodeErrorKind::InvalidStringTable,
            "class.name.terminator",
        ));
    }
    Ok(packed.bytes)
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
