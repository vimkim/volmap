//! Attribute-value decoding for one heap record against a parsed class
//! representation.
//!
//! Every scalar in a record body is big-endian (`or_put_int` is `htonl`), in
//! contrast with the native-endian page prologue
//! (`docs/record-interpretation-research.md` §4.3).
//!
//! A value that cannot be decoded never yields bytes. It yields a typed
//! placeholder naming the type, extent, and reason — the disclosure rule in
//! `docs/adr/0001-explicit-target-disclosure.md` withholds raw payload bytes
//! everywhere, so there is deliberately no hex fallback.

use crate::bytes::ByteView;
use crate::model::{Oid, PageId, SlotId, VolId};

use super::classrep::{AttributeDomainFact, ClassRepresentationFact, DbType};
use super::object_layout::{
    OR_VAR_BIT_OOS, is_supported_offset_width, lossy_text, range, read_f32_be, read_f64_be,
    read_i16_be, read_i32_be, read_i64_be, read_packed_string, read_u8, read_u32_be,
    var_entry_offset, var_entry_raw, var_table_size,
};
use super::{DecodeError, DecodeErrorKind};

/// `NUMERIC_HEADER_SIZE` — size byte, precision byte, scale byte.
const NUMERIC_HEADER_SIZE: usize = 3;
/// The largest total size a NUMERIC header may claim, header included.
///
/// The engine picks it from `_gv_mr_float_numeric_precision_to_size` and
/// `_gv_mr_fixed_numeric_bytes_to_size` (`src/object/object_primitive.c:145`,
/// `:151`), whose maximum entry is 20. That leaves a 17-byte magnitude, which is
/// exactly `DB_NUMERIC_BUF_SIZE` — the buffer the writer slices from.
const NUMERIC_MAX_TOTAL_SIZE: usize = 20;
/// `NUMERIC_VALUE_SIGN_BIT_MASK` and `NUMERIC_HEADER_SCALE_SIGN_BIT_MASK`.
const NUMERIC_SIGN_BIT: u8 = 0x80;
/// A stored out-of-row attribute is an 8-byte OID plus an 8-byte total length.
const OOS_STUB_SIZE: usize = 16;

/// One decoded attribute value.
///
/// Temporal values carry their calendar fields rather than a rendered string so
/// that formatting stays a presentation choice. NUMERIC is the exception: its
/// precision exceeds every primitive, so the decimal text *is* the fact.
#[derive(Clone, Debug, PartialEq)]
pub enum AttributeValue {
    Integer(i32),
    Short(i16),
    BigInt(i64),
    Float(f32),
    Double(f64),
    Numeric(String),
    Monetary {
        currency_code: i32,
        amount: f64,
    },
    Date(CalendarDate),
    Time(ClockTime),
    /// Seconds since the Unix epoch, as the engine stores `DB_UTIME`.
    Timestamp(u32),
    DateTime {
        date: CalendarDate,
        time: ClockTime,
        millisecond: u32,
    },
    Text(String),
    Object(Oid),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CalendarDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockTime {
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// What one attribute of one record turned out to hold.
#[derive(Clone, Debug, PartialEq)]
pub enum AttributeInterpretation {
    Decoded(AttributeValue),
    /// SQL NULL: an unset bound bit for a fixed attribute, or a zero-length
    /// extent for a variable one.
    Null,
    /// The value lives out of row; the record holds only this reference.
    OutOfRow {
        head: Oid,
        total_length: i64,
    },
    /// No value is disclosed. `reason` is user-facing prose, not a rule code.
    Undecodable {
        reason: &'static str,
        offset: u32,
        length: u32,
    },
}

/// One attribute of one interpreted record.
#[derive(Clone, Debug, PartialEq)]
pub struct InterpretedAttribute {
    pub id: i32,
    pub name: Option<String>,
    pub domain: AttributeDomainFact,
    pub position: u32,
    pub interpretation: AttributeInterpretation,
}

/// Decodes every attribute of one record body against `representation`.
///
/// `body`, `offset_width`, and `has_bound_bits` come from the record's already
/// validated object header. The representation must be the one the record's own
/// representation id selects; passing a mismatched representation is what
/// produces nonsense rather than an error, so callers resolve it first.
///
/// Frame-level damage — an unreadable offset table, a fixed region that does
/// not fit — fails the call. A single value that cannot be decoded does not:
/// it becomes [`AttributeInterpretation::Undecodable`] so the rest of the
/// record still interprets.
pub fn decode_record_attributes(
    body: &[u8],
    offset_width: u8,
    has_bound_bits: bool,
    representation: &ClassRepresentationFact,
) -> Result<Vec<InterpretedAttribute>, DecodeError> {
    if !is_supported_offset_width(offset_width) {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "interpretation.record.offset_width",
        ));
    }
    let view = ByteView::new(body, 0);
    let frame = RecordFrame::new(&view, offset_width, representation)?;

    let mut attributes = Vec::with_capacity(representation.attributes.len());
    for attribute in &representation.attributes {
        let interpretation = if attribute.is_fixed {
            frame.fixed_attribute(
                &view,
                attribute.location,
                attribute.position,
                has_bound_bits,
                attribute.domain,
            )?
        } else {
            frame.variable_attribute(&view, attribute.location, attribute.domain)?
        };
        attributes.push(InterpretedAttribute {
            id: attribute.id,
            name: attribute.name.clone(),
            domain: attribute.domain,
            position: attribute.position,
            interpretation,
        });
    }
    Ok(attributes)
}

/// Where the regions of one record body begin.
struct RecordFrame {
    offset_width: u8,
    fixed_base: usize,
    bound_bits: usize,
}

impl RecordFrame {
    fn new(
        view: &ByteView<'_>,
        offset_width: u8,
        representation: &ClassRepresentationFact,
    ) -> Result<Self, DecodeError> {
        let rule = "interpretation.frame.bound_bits";
        let fixed_base = var_table_size(representation.variable_count, offset_width)?;
        let fixed_length = usize::try_from(representation.fixed_length)
            .map_err(|_| error(DecodeErrorKind::ArithmeticOverflow, rule))?;
        let bound_bits = fixed_base
            .checked_add(fixed_length)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, rule))?;
        if bound_bits > view.len() {
            return Err(error(
                DecodeErrorKind::InvalidLength,
                "interpretation.frame.fixed_region",
            ));
        }
        Ok(Self {
            offset_width,
            fixed_base,
            bound_bits,
        })
    }

    fn fixed_attribute(
        &self,
        view: &ByteView<'_>,
        location: u32,
        position: u32,
        has_bound_bits: bool,
        domain: AttributeDomainFact,
    ) -> Result<AttributeInterpretation, DecodeError> {
        let Some(size) = domain.fixed_disk_size() else {
            return Err(error(
                DecodeErrorKind::InvalidGeometry,
                "interpretation.fixed.variable_type",
            ));
        };
        if has_bound_bits && self.is_unbound(view, position)? {
            return Ok(AttributeInterpretation::Null);
        }
        let rule = "interpretation.fixed.offset";
        let location = usize::try_from(location)
            .map_err(|_| error(DecodeErrorKind::ArithmeticOverflow, rule))?;
        let offset = self
            .fixed_base
            .checked_add(location)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, rule))?;
        // A fixed attribute must lie wholly inside the fixed region. Reading
        // past it would pull in bound bits or another attribute's bytes and
        // present them as this attribute's value.
        if offset
            .checked_add(size)
            .is_none_or(|end| end > self.bound_bits)
        {
            return Err(error(
                DecodeErrorKind::InvalidGeometry,
                "interpretation.fixed.region_bounds",
            ));
        }
        Ok(decode_value(view, offset, size, domain))
    }

    /// A clear bound-bit flag in the record header means every fixed attribute
    /// is bound, so only the flagged case consults the vector.
    fn is_unbound(&self, view: &ByteView<'_>, position: u32) -> Result<bool, DecodeError> {
        let rule = "interpretation.bound_bits";
        let index = usize::try_from(position)
            .map_err(|_| error(DecodeErrorKind::ArithmeticOverflow, rule))?;
        let byte = self
            .bound_bits
            .checked_add(index / 8)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, rule))?;
        let bits = read_u8(view, byte, rule)?;
        Ok(bits & (1 << (index % 8)) == 0)
    }

    fn variable_attribute(
        &self,
        view: &ByteView<'_>,
        location: u32,
        domain: AttributeDomainFact,
    ) -> Result<AttributeInterpretation, DecodeError> {
        let rule = "interpretation.variable.offset_table";
        let raw = var_entry_raw(view, 0, self.offset_width, location, rule)?;
        let start = var_entry_offset(view, 0, self.offset_width, location, rule)?;
        let next = location
            .checked_add(1)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, rule))?;
        let end = var_entry_offset(view, 0, self.offset_width, next, rule)?;
        if end < start {
            return Err(error(DecodeErrorKind::InvalidGeometry, rule));
        }
        let length = end - start;

        // The out-of-row flag wins over the attribute's own type: whatever the
        // column holds, the record itself holds only the stub.
        if raw & OR_VAR_BIT_OOS != 0 {
            return Ok(decode_out_of_row_stub(view, start, length));
        }
        if length == 0 {
            return Ok(AttributeInterpretation::Null);
        }
        Ok(decode_value(view, start, length, domain))
    }
}

/// Decodes one value of known extent, or explains why it stays withheld.
fn decode_value(
    view: &ByteView<'_>,
    offset: usize,
    length: usize,
    domain: AttributeDomainFact,
) -> AttributeInterpretation {
    match read_value(view, offset, length, domain) {
        Some(value) => AttributeInterpretation::Decoded(value),
        None => withheld(
            offset,
            length,
            unsupported_reason(domain.db_type)
                .unwrap_or("the stored bytes do not form a valid value of this type"),
        ),
    }
}

/// A placeholder naming an attribute's extent without disclosing its bytes.
///
/// An extent is reported for display, so one too large to name exactly
/// saturates rather than failing an otherwise usable interpretation. Both
/// always fit in practice: a record body is at most one page.
fn withheld(offset: usize, length: usize, reason: &'static str) -> AttributeInterpretation {
    AttributeInterpretation::Undecodable {
        reason,
        offset: u32::try_from(offset).unwrap_or(u32::MAX),
        length: u32::try_from(length).unwrap_or(u32::MAX),
    }
}

/// Prose for a type version one decides not to decode, or `None` when the type
/// is one it does decode and the bytes are simply unusable.
const fn unsupported_reason(db_type: DbType) -> Option<&'static str> {
    match db_type {
        DbType::Set | DbType::Multiset | DbType::Sequence => {
            Some("collection elements are not decoded in this version")
        }
        DbType::Enumeration => {
            Some("enum literals are stored in the domain and are not resolved in this version")
        }
        DbType::Bit | DbType::VarBit => Some("bit strings are not rendered in this version"),
        DbType::Blob | DbType::Clob => {
            Some("large objects live outside the volume and are not read")
        }
        DbType::Json => Some("JSON documents are not decoded in this version"),
        DbType::TimestampTz | DbType::TimestampLtz | DbType::DateTimeTz | DbType::DateTimeLtz => {
            Some("time-zone-qualified values are not decoded in this version")
        }
        DbType::Unsupported(_) => Some("this attribute type is not recognized"),
        _ => None,
    }
}

fn read_value(
    view: &ByteView<'_>,
    offset: usize,
    length: usize,
    domain: AttributeDomainFact,
) -> Option<AttributeValue> {
    let rule = "interpretation.value";
    match domain.db_type {
        DbType::Integer => read_i32_be(view, offset, rule)
            .ok()
            .map(AttributeValue::Integer),
        DbType::Short => read_i16_be(view, offset, rule)
            .ok()
            .map(AttributeValue::Short),
        DbType::BigInt => read_i64_be(view, offset, rule)
            .ok()
            .map(AttributeValue::BigInt),
        DbType::Float => read_f32_be(view, offset, rule)
            .ok()
            .map(AttributeValue::Float),
        DbType::Double => read_f64_be(view, offset, rule)
            .ok()
            .map(AttributeValue::Double),
        // DB_DATE, DB_TIME, and DB_UTIME are all `unsigned int` in the engine.
        DbType::Date => read_u32_be(view, offset, rule)
            .ok()
            .and_then(julian_to_date)
            .map(AttributeValue::Date),
        DbType::Time => read_u32_be(view, offset, rule)
            .ok()
            .and_then(seconds_to_time)
            .map(AttributeValue::Time),
        DbType::Timestamp => read_u32_be(view, offset, rule)
            .ok()
            .map(AttributeValue::Timestamp),
        DbType::DateTime => {
            let date = read_u32_be(view, offset, rule).ok()?;
            let millis = read_u32_be(view, offset + 4, rule).ok()?;
            Some(AttributeValue::DateTime {
                date: julian_to_date(date)?,
                time: seconds_to_time(millis / 1_000)?,
                millisecond: millis % 1_000,
            })
        }
        DbType::Monetary => Some(AttributeValue::Monetary {
            currency_code: read_i32_be(view, offset, rule).ok()?,
            amount: read_f64_be(view, offset + 4, rule).ok()?,
        }),
        DbType::Object => read_oid(view, offset, rule).map(AttributeValue::Object),
        DbType::String | DbType::Char | DbType::NChar | DbType::VarNChar => {
            read_packed_string(view, offset, length, rule)
                .ok()
                .map(|bytes| AttributeValue::Text(lossy_text(&bytes)))
        }
        DbType::Numeric => read_numeric(view, offset, length).map(AttributeValue::Numeric),
        DbType::Set
        | DbType::Multiset
        | DbType::Sequence
        | DbType::Bit
        | DbType::VarBit
        | DbType::Blob
        | DbType::Clob
        | DbType::Enumeration
        | DbType::Json
        | DbType::TimestampTz
        | DbType::TimestampLtz
        | DbType::DateTimeTz
        | DbType::DateTimeLtz
        | DbType::Unsupported(_) => None,
    }
}

fn read_oid(view: &ByteView<'_>, offset: usize, rule: &'static str) -> Option<Oid> {
    let page = read_i32_be(view, offset, rule).ok()?;
    let slot = read_i16_be(view, offset + 4, rule).ok()?;
    let volume = read_i16_be(view, offset + 6, rule).ok()?;
    Some(Oid::new(
        VolId::new(volume).ok()?,
        PageId::new(page).ok()?,
        SlotId::new(slot).ok()?,
    ))
}

/// A NUMERIC is a 3-byte header then the low-order bytes of the engine's
/// internal magnitude buffer, big-endian (research §4.5).
fn read_numeric(view: &ByteView<'_>, offset: usize, length: usize) -> Option<String> {
    let size_byte = read_u8(view, offset, "interpretation.numeric.size").ok()?;
    let total = usize::from(size_byte & !NUMERIC_SIGN_BIT);
    if total <= NUMERIC_HEADER_SIZE || total > NUMERIC_MAX_TOTAL_SIZE || total > length {
        return None;
    }
    let negative = size_byte & NUMERIC_SIGN_BIT != 0;
    let scale_sign = read_u8(view, offset + 1, "interpretation.numeric.precision").ok()?;
    let scale_magnitude = read_u8(view, offset + 2, "interpretation.numeric.scale").ok()?;
    let magnitude = range(
        view,
        offset + NUMERIC_HEADER_SIZE,
        total - NUMERIC_HEADER_SIZE,
        "interpretation.numeric.magnitude",
    )
    .ok()?;

    let mut digits: u128 = 0;
    for byte in magnitude {
        digits = digits.checked_mul(256)?.checked_add(u128::from(*byte))?;
    }
    let sign = if negative { "-" } else { "" };
    let text = if scale_sign & NUMERIC_SIGN_BIT == 0 {
        insert_decimal_point(&digits.to_string(), usize::from(scale_magnitude))?
    } else {
        // A negative scale means trailing implicit zeroes.
        let mut text = digits.to_string();
        text.push_str(&"0".repeat(usize::from(scale_magnitude)));
        text
    };
    Some(format!("{sign}{text}"))
}

fn insert_decimal_point(digits: &str, scale: usize) -> Option<String> {
    if scale == 0 {
        return Some(digits.to_owned());
    }
    // Left-padding to `scale + 1` guarantees at least one integer digit, so the
    // split is in range — but it is computed, not assumed, because a corrupt
    // scale byte reaches here.
    let padded = format!("{digits:0>width$}", width = scale.checked_add(1)?);
    let split = padded.len().checked_sub(scale)?;
    let integer = padded.get(..split)?;
    let fraction = padded.get(split..)?;
    Some(format!("{integer}.{fraction}"))
}

fn decode_out_of_row_stub(
    view: &ByteView<'_>,
    offset: usize,
    length: usize,
) -> AttributeInterpretation {
    const TRUNCATED: &str = "the out-of-row reference in this record is truncated";
    let rule = "interpretation.out_of_row";
    if length < OOS_STUB_SIZE {
        return withheld(offset, length, TRUNCATED);
    }
    let Some(head) = read_oid(view, offset, rule) else {
        return withheld(
            offset,
            length,
            "the out-of-row reference in this record is not a valid location",
        );
    };
    match read_i64_be(view, offset + 8, rule) {
        Ok(total_length) => AttributeInterpretation::OutOfRow { head, total_length },
        Err(_) => withheld(offset, length, TRUNCATED),
    }
}

/// `julian_decode` (`src/compat/db_date.c`), the Fliegel and Van Flandern
/// algorithm the engine uses for `DB_DATE`.
///
/// Returns `None` for a day number outside CUBRID's supported date range rather
/// than reporting a fabricated calendar date: an out-of-domain raw value is not
/// a date, and interpreted evidence has to be evidence.
fn julian_to_date(julian: u32) -> Option<CalendarDate> {
    let mut remainder = i64::from(julian) + 68_569;
    let centuries = 4 * remainder / 146_097;
    remainder -= (146_097 * centuries + 3) / 4;
    let years = 4_000 * (remainder + 1) / 1_461_001;
    remainder = remainder - 1_461 * years / 4 + 31;
    let months = 80 * remainder / 2_447;
    let day = remainder - 2_447 * months / 80;
    let month_carry = months / 11;
    let date = CalendarDate {
        year: i32::try_from(100 * (centuries - 49) + years + month_carry).ok()?,
        month: u32::try_from(months + 2 - 12 * month_carry).ok()?,
        day: u32::try_from(day).ok()?,
    };
    // CUBRID's own DATE domain is 0001-01-01 through 9999-12-31.
    if !(1..=9_999).contains(&date.year)
        || !(1..=12).contains(&date.month)
        || !(1..=31).contains(&date.day)
    {
        return None;
    }
    Some(date)
}

/// `DB_TIME` holds seconds since midnight (`decode_time`).
///
/// A value at or past a full day is out of domain; reporting it modulo 24 hours
/// would invent a plausible-looking time from a corrupt one.
const fn seconds_to_time(seconds: u32) -> Option<ClockTime> {
    if seconds >= 86_400 {
        return None;
    }
    Some(ClockTime {
        hour: seconds / 3_600,
        minute: seconds / 60 % 60,
        second: seconds % 60,
    })
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
