//! Class-representation parser.
//!
//! A record's representation id resolves against the **class object's own heap
//! record**, not the system catalog: the catalog's `DISK_REPR`/`DISK_ATTR`
//! records serve the query optimizer, and its extendible hash is dead code
//! (`docs/adr/0002-classrepr-from-class-record.md`,
//! `docs/record-interpretation-research.md` §3.1/§3.5).
//!
//! This is a port of the engine's `or_get_current_representation` and
//! `or_get_old_representation` (`src/base/object_representation_sr.c:2414`,
//! `:2934`) over untrusted bytes: every offset is bounds-checked and no input
//! can panic.

use crate::bytes::ByteView;

use super::object_layout::{
    lossy_text, range, read_i32_be, read_packed_string, var_entry_offset, var_table_size,
};
use super::{DecodeError, DecodeErrorKind};

/// Class records always use the 4-byte offset table (`BIG_VAR_OFFSET_SIZE`).
const CLASS_OFFSET_WIDTH: u8 = 4;

/// Variable-attribute counts of each substructure, from the `ORC_*` index enums
/// in `src/base/object_representation.h:774`.
const ORC_CLASS_VAR_ATT_COUNT: u32 = 17;
const ORC_ATT_VAR_ATT_COUNT: u32 = 7;
const ORC_DOMAIN_VAR_ATT_COUNT: u32 = 3;
const ORC_REP_VAR_ATT_COUNT: u32 = 2;
const ORC_REPATT_VAR_ATT_COUNT: u32 = 1;

/// Class variable-attribute indexes.
const ORC_NAME_INDEX: u32 = 0;
const ORC_REPRESENTATIONS_INDEX: u32 = 2;
const ORC_ATTRIBUTES_INDEX: u32 = 5;

/// Class fixed-region field offsets.
const ORC_FIXED_COUNT_OFFSET: usize = 28;
const ORC_VARIABLE_COUNT_OFFSET: usize = 32;
const ORC_FIXED_LENGTH_OFFSET: usize = 36;

/// Attribute substructure.
const ORC_ATT_ID_OFFSET: usize = 0;
const ORC_ATT_TYPE_OFFSET: usize = 4;
const ORC_ATT_NAME_INDEX: u32 = 0;
const ORC_ATT_DOMAIN_INDEX: u32 = 3;

/// Old-representation substructure.
const ORC_REP_ID_OFFSET: usize = 0;
const ORC_REP_FIXED_COUNT_OFFSET: usize = 4;
const ORC_REP_VARIABLE_COUNT_OFFSET: usize = 8;
const ORC_REP_ATTRIBUTES_INDEX: u32 = 0;
const ORC_REPATT_ID_OFFSET: usize = 0;
const ORC_REPATT_TYPE_OFFSET: usize = 4;
const ORC_REPATT_DOMAIN_INDEX: u32 = 0;

/// Domain substructure.
const ORC_DOMAIN_TYPE_OFFSET: usize = 0;
const ORC_DOMAIN_PRECISION_OFFSET: usize = 4;
const ORC_DOMAIN_SCALE_OFFSET: usize = 8;
const ORC_DOMAIN_CODESET_OFFSET: usize = 12;
const ORC_DOMAIN_COLLATION_ID_OFFSET: usize = 16;

/// A substructure set: an 8-byte header whose element count sits at +4,
/// followed by a 4-byte offset table (`OR_SET_HEADER_SIZE`,
/// `OR_SET_ELEMENT_COUNT`, `src/base/object_representation.h:671`, `:705`).
const SET_HEADER_SIZE: usize = 8;
const SET_COUNT_OFFSET: usize = 4;

/// A ceiling on attributes per representation. CUBRID's own limit is far lower
/// than this; the bound exists so a corrupt count cannot drive a large
/// allocation before any element is read.
const MAX_ATTRIBUTES: u32 = 4_096;

/// A CUBRID `DB_TYPE` as stored in a packed domain.
///
/// The `Unsupported` arm keeps an unrecognized code visible instead of failing
/// the whole representation: an attribute this decoder cannot type is still a
/// real attribute at a real location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DbType {
    Integer,
    Float,
    Double,
    String,
    Object,
    Set,
    Multiset,
    Sequence,
    Time,
    Timestamp,
    Date,
    Monetary,
    Short,
    Numeric,
    Bit,
    VarBit,
    Char,
    NChar,
    VarNChar,
    BigInt,
    DateTime,
    Blob,
    Clob,
    Enumeration,
    TimestampTz,
    TimestampLtz,
    DateTimeTz,
    DateTimeLtz,
    Json,
    Unsupported(i32),
}

impl DbType {
    #[must_use]
    pub const fn from_code(code: i32) -> Self {
        match code {
            1 => Self::Integer,
            2 => Self::Float,
            3 => Self::Double,
            4 => Self::String,
            5 => Self::Object,
            6 => Self::Set,
            7 => Self::Multiset,
            8 => Self::Sequence,
            10 => Self::Time,
            11 => Self::Timestamp,
            12 => Self::Date,
            13 => Self::Monetary,
            18 => Self::Short,
            22 => Self::Numeric,
            23 => Self::Bit,
            24 => Self::VarBit,
            25 => Self::Char,
            26 => Self::NChar,
            27 => Self::VarNChar,
            31 => Self::BigInt,
            32 => Self::DateTime,
            33 => Self::Blob,
            34 => Self::Clob,
            35 => Self::Enumeration,
            36 => Self::TimestampTz,
            37 => Self::TimestampLtz,
            38 => Self::DateTimeTz,
            39 => Self::DateTimeLtz,
            40 => Self::Json,
            other => Self::Unsupported(other),
        }
    }

    /// The SQL-facing type name. Reaches users verbatim, so it stays in the
    /// human vocabulary rather than the dotted-rule one.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "INTEGER",
            Self::Float => "FLOAT",
            Self::Double => "DOUBLE",
            Self::String => "VARCHAR",
            Self::Object => "OBJECT",
            Self::Set => "SET",
            Self::Multiset => "MULTISET",
            Self::Sequence => "SEQUENCE",
            Self::Time => "TIME",
            Self::Timestamp => "TIMESTAMP",
            Self::Date => "DATE",
            Self::Monetary => "MONETARY",
            Self::Short => "SHORT",
            Self::Numeric => "NUMERIC",
            Self::Bit => "BIT",
            Self::VarBit => "BIT VARYING",
            Self::Char => "CHAR",
            Self::NChar => "NCHAR",
            Self::VarNChar => "NCHAR VARYING",
            Self::BigInt => "BIGINT",
            Self::DateTime => "DATETIME",
            Self::Blob => "BLOB",
            Self::Clob => "CLOB",
            Self::Enumeration => "ENUM",
            Self::TimestampTz => "TIMESTAMP WITH TIME ZONE",
            Self::TimestampLtz => "TIMESTAMP WITH LOCAL TIME ZONE",
            Self::DateTimeTz => "DATETIME WITH TIME ZONE",
            Self::DateTimeLtz => "DATETIME WITH LOCAL TIME ZONE",
            Self::Json => "JSON",
            Self::Unsupported(_) => "UNRECOGNIZED",
        }
    }

    /// Bytes a value of this type occupies in the fixed region when that width
    /// is the same for every value, or `None` when the width depends on the
    /// domain or the type lives in the variable region.
    ///
    /// Use [`AttributeDomainFact::fixed_disk_size`] instead of this: `BIT` is a
    /// fixed-region type whose width comes from its precision, so the type alone
    /// cannot answer.
    const fn constant_disk_size(self) -> Option<usize> {
        match self {
            Self::Short | Self::Enumeration => Some(2),
            Self::Integer
            | Self::Float
            | Self::Time
            | Self::Timestamp
            | Self::Date
            | Self::TimestampLtz => Some(4),
            Self::Double
            | Self::Object
            | Self::BigInt
            | Self::DateTime
            | Self::TimestampTz
            | Self::DateTimeLtz => Some(8),
            Self::Monetary | Self::DateTimeTz => Some(12),
            Self::Bit
            | Self::String
            | Self::Set
            | Self::Multiset
            | Self::Sequence
            | Self::Numeric
            | Self::VarBit
            | Self::Char
            | Self::NChar
            | Self::VarNChar
            | Self::Blob
            | Self::Clob
            | Self::Json
            | Self::Unsupported(_) => None,
        }
    }
}

/// The packed domain of one attribute.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttributeDomainFact {
    pub db_type: DbType,
    pub precision: i32,
    pub scale: i32,
    pub codeset: i32,
    pub collation_id: i32,
}

impl AttributeDomainFact {
    /// Bytes this attribute occupies in the record's fixed region, or `None`
    /// when it lives in the variable region instead.
    ///
    /// Two arms of this are counterintuitive and are where a reimplementation
    /// most often goes wrong. `CHAR` and `NUMERIC` are *variable*-region types
    /// despite having a fixed precision. `BIT` is the reverse: it is a
    /// fixed-region type (`tp_Bit` declares `variable_p = 0`) whose width is
    /// `STR_SIZE` over the raw-bits codeset, so it depends on the precision.
    /// A `BIT` of floating precision has no fixed width at all.
    #[must_use]
    pub fn fixed_disk_size(&self) -> Option<usize> {
        if self.db_type == DbType::Bit {
            let precision = usize::try_from(self.precision).ok()?;
            if precision == 0 {
                return None;
            }
            return Some(precision.div_ceil(8));
        }
        self.db_type.constant_disk_size()
    }
}

/// One attribute of one representation.
///
/// `name` is absent for attributes recovered from an old representation whose
/// id no longer exists in the class's current attribute set: the engine stores
/// no name in a `rep_attribute` substructure, so a dropped column's name is
/// genuinely not on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassAttributeFact {
    pub id: i32,
    pub name: Option<String>,
    pub domain: AttributeDomainFact,
    pub is_fixed: bool,
    /// Byte offset within the fixed region when fixed; index into the record's
    /// variable-offset table when variable.
    pub location: u32,
    pub position: u32,
}

/// Which representation of a class record to decode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepresentationTarget {
    /// The class's current representation.
    Current,
    /// A specific representation id, which may be current or historical.
    Id(u32),
}

/// One decoded representation of one class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassRepresentationFact {
    pub class_name: String,
    pub representation_id: u32,
    /// Whether this is the class's current representation rather than one
    /// recovered from the old-representation set.
    pub is_current: bool,
    pub fixed_count: u32,
    pub variable_count: u32,
    pub fixed_length: u32,
    pub attributes: Vec<ClassAttributeFact>,
}

/// Decodes one representation from a class object's heap-record body.
///
/// `body` starts after the record's object header; `offset_width` and
/// `current_representation_id` come from that header, which
/// [`super::decode_heap_record_envelope`] has already validated. Class records
/// always carry a 4-byte offset table, so a differing width is rejected rather
/// than accommodated.
pub fn decode_class_representation(
    body: &[u8],
    offset_width: u8,
    current_representation_id: u32,
    target: RepresentationTarget,
) -> Result<ClassRepresentationFact, DecodeError> {
    if offset_width != CLASS_OFFSET_WIDTH {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "classrep.record.offset_width",
        ));
    }
    let view = ByteView::new(body, 0);
    let class_table = ClassTable::new(&view)?;
    match target {
        RepresentationTarget::Id(id) if id != current_representation_id => {
            old_representation(&view, id)
        }
        RepresentationTarget::Current | RepresentationTarget::Id(_) => {
            current_representation(&view, &class_table, current_representation_id)
        }
    }
}

/// The class object's own variable-offset table and fixed region.
struct ClassTable {
    fixed: usize,
}

impl ClassTable {
    fn new(view: &ByteView<'_>) -> Result<Self, DecodeError> {
        // The class object's table starts at the body origin, so a class
        // variable attribute is addressed relative to that same origin.
        let fixed = var_table_size(ORC_CLASS_VAR_ATT_COUNT, CLASS_OFFSET_WIDTH)?;
        // Touch the last fixed field so a truncated class record fails here
        // rather than midway through building attributes.
        read_i32_be(
            view,
            fixed + ORC_FIXED_LENGTH_OFFSET,
            "classrep.class.fixed_region",
        )?;
        Ok(Self { fixed })
    }

    fn field(&self, view: &ByteView<'_>, offset: usize) -> Result<i32, DecodeError> {
        read_i32_be(view, self.fixed + offset, "classrep.class.fixed_field")
    }
}

/// Offset of class variable attribute `index`. The class object's own offset
/// table starts at the body origin, so these are body-relative.
fn class_entry(view: &ByteView<'_>, index: u32) -> Result<usize, DecodeError> {
    var_entry_offset(
        view,
        0,
        CLASS_OFFSET_WIDTH,
        index,
        "classrep.class.var_table",
    )
}

fn class_entry_span(view: &ByteView<'_>, index: u32) -> Result<(usize, usize), DecodeError> {
    let start = class_entry(view, index)?;
    let end = class_entry(view, index + 1)?;
    if end < start {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "classrep.class.var_table_order",
        ));
    }
    Ok((start, end - start))
}

fn class_name_text(view: &ByteView<'_>) -> Result<String, DecodeError> {
    let (offset, extent) = class_entry_span(view, ORC_NAME_INDEX)?;
    Ok(lossy_text(&read_packed_string(
        view,
        offset,
        extent,
        "classrep.class.name",
    )?))
}

fn current_representation(
    view: &ByteView<'_>,
    table: &ClassTable,
    representation_id: u32,
) -> Result<ClassRepresentationFact, DecodeError> {
    let class_name = class_name_text(view)?;
    let fixed_count = count(
        table.field(view, ORC_FIXED_COUNT_OFFSET)?,
        "classrep.class.n_fixed",
    )?;
    let variable_count = count(
        table.field(view, ORC_VARIABLE_COUNT_OFFSET)?,
        "classrep.class.n_variable",
    )?;
    let fixed_length = count(
        table.field(view, ORC_FIXED_LENGTH_OFFSET)?,
        "classrep.class.fixed_length",
    )?;

    let attset = class_entry(view, ORC_ATTRIBUTES_INDEX)?;
    let set = SubstructureSet::new(view, attset, "classrep.attset")?;
    let total = fixed_count
        .checked_add(variable_count)
        .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "classrep.attset.total"))?;
    if set.count != total {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "classrep.attset.count",
        ));
    }

    let mut attributes = Vec::with_capacity(capacity(total));
    let mut fixed_cursor: usize = 0;
    for position in 0..total {
        let element = set.element(view, position)?;
        let element_fixed = element
            .checked_add(var_table_size(ORC_ATT_VAR_ATT_COUNT, CLASS_OFFSET_WIDTH)?)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "classrep.att.fixed"))?;
        let id = read_i32_be(view, element_fixed + ORC_ATT_ID_OFFSET, "classrep.att.id")?;
        let declared = read_i32_be(
            view,
            element_fixed + ORC_ATT_TYPE_OFFSET,
            "classrep.att.type",
        )?;
        let (name, name_extent) =
            element_entry_span(view, element, ORC_ATT_NAME_INDEX, "classrep.att.name_entry")?;
        let name = lossy_text(&read_packed_string(
            view,
            name,
            name_extent,
            "classrep.att.name",
        )?);

        let domain_offset = element_entry(
            view,
            element,
            ORC_ATT_DOMAIN_INDEX,
            "classrep.att.domain_entry",
        )?;
        let domain = decode_domain(view, domain_offset)?;
        if domain.db_type != DbType::from_code(declared) {
            return Err(error(
                DecodeErrorKind::IdentityMismatch,
                "classrep.att.type_agreement",
            ));
        }

        attributes.push(place(
            id,
            Some(name),
            domain,
            position,
            fixed_count,
            &mut fixed_cursor,
        )?);
    }

    Ok(ClassRepresentationFact {
        class_name,
        representation_id,
        is_current: true,
        fixed_count,
        variable_count,
        fixed_length,
        attributes,
    })
}

fn old_representation(
    view: &ByteView<'_>,
    representation_id: u32,
) -> Result<ClassRepresentationFact, DecodeError> {
    let class_name = class_name_text(view)?;
    let (repset, repset_length) = class_entry_span(view, ORC_REPRESENTATIONS_INDEX)?;
    if repset_length == 0 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "classrep.repset.absent",
        ));
    }
    let set = SubstructureSet::new(view, repset, "classrep.repset")?;
    let (element, element_fixed) = find_representation(view, &set, representation_id)?;

    let fixed_count = count(
        read_i32_be(
            view,
            element_fixed + ORC_REP_FIXED_COUNT_OFFSET,
            "classrep.rep.n_fixed",
        )?,
        "classrep.rep.n_fixed",
    )?;
    let variable_count = count(
        read_i32_be(
            view,
            element_fixed + ORC_REP_VARIABLE_COUNT_OFFSET,
            "classrep.rep.n_variable",
        )?,
        "classrep.rep.n_variable",
    )?;
    let total = fixed_count
        .checked_add(variable_count)
        .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "classrep.rep.total"))?;

    let attset = element_entry(
        view,
        element,
        ORC_REP_ATTRIBUTES_INDEX,
        "classrep.rep.attset_entry",
    )?;
    let attributes_set = SubstructureSet::new(view, attset, "classrep.rep.attset")?;
    if attributes_set.count != total {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "classrep.rep.attset_count",
        ));
    }

    // A rep_attribute stores no name. Recover one by attribute id from the
    // class's current attribute set, which is authoritative for any id that
    // still exists; an id dropped since keeps no name anywhere on disk.
    let current_names = current_attribute_names(view)?;

    let mut attributes = Vec::with_capacity(capacity(total));
    let mut fixed_cursor: usize = 0;
    for position in 0..total {
        let repatt = attributes_set.element(view, position)?;
        let (id, domain) = decode_rep_attribute(view, repatt)?;
        let name = current_names
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .map(|(_, name)| name.clone());
        attributes.push(place(
            id,
            name,
            domain,
            position,
            fixed_count,
            &mut fixed_cursor,
        )?);
    }

    // An old representation does not store its fixed width; the engine
    // recomputes it by summing disk sizes and aligning, so this does too.
    let fixed_length = u32::try_from((fixed_cursor + 3) & !3).map_err(|_| {
        error(
            DecodeErrorKind::ArithmeticOverflow,
            "classrep.rep.fixed_length",
        )
    })?;

    Ok(ClassRepresentationFact {
        class_name,
        representation_id,
        is_current: false,
        fixed_count,
        variable_count,
        fixed_length,
        attributes,
    })
}

/// Locates the element of the old-representation set carrying `representation_id`,
/// returning its start and the start of its fixed region.
fn find_representation(
    view: &ByteView<'_>,
    set: &SubstructureSet,
    representation_id: u32,
) -> Result<(usize, usize), DecodeError> {
    for index in 0..set.count {
        let element = set.element(view, index)?;
        let element_fixed = element
            .checked_add(var_table_size(ORC_REP_VAR_ATT_COUNT, CLASS_OFFSET_WIDTH)?)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "classrep.rep.fixed"))?;
        let id = read_i32_be(view, element_fixed + ORC_REP_ID_OFFSET, "classrep.rep.id")?;
        if u32::try_from(id) == Ok(representation_id) {
            return Ok((element, element_fixed));
        }
    }
    Err(error(
        DecodeErrorKind::OutOfRange,
        "classrep.rep.unknown_id",
    ))
}

/// Reads one `rep_attribute`: an id, a type, and a domain. Unlike a current
/// attribute it carries no name.
fn decode_rep_attribute(
    view: &ByteView<'_>,
    repatt: usize,
) -> Result<(i32, AttributeDomainFact), DecodeError> {
    let repatt_fixed = repatt
        .checked_add(var_table_size(
            ORC_REPATT_VAR_ATT_COUNT,
            CLASS_OFFSET_WIDTH,
        )?)
        .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "classrep.repatt.fixed"))?;
    let id = read_i32_be(
        view,
        repatt_fixed + ORC_REPATT_ID_OFFSET,
        "classrep.repatt.id",
    )?;
    let declared = read_i32_be(
        view,
        repatt_fixed + ORC_REPATT_TYPE_OFFSET,
        "classrep.repatt.type",
    )?;
    let domain = match element_entry_span(
        view,
        repatt,
        ORC_REPATT_DOMAIN_INDEX,
        "classrep.repatt.domain_entry",
    )? {
        // The engine synthesizes a default domain when the substructure is
        // absent; carry the declared type with unknown parameters rather than
        // inventing a precision.
        (_, 0) => AttributeDomainFact {
            db_type: DbType::from_code(declared),
            precision: 0,
            scale: 0,
            codeset: 0,
            collation_id: 0,
        },
        (offset, _) => decode_domain(view, offset)?,
    };
    if domain.db_type != DbType::from_code(declared) {
        return Err(error(
            DecodeErrorKind::IdentityMismatch,
            "classrep.repatt.type_agreement",
        ));
    }
    Ok((id, domain))
}

/// `(attribute id, name)` pairs of the class's current attribute set.
fn current_attribute_names(view: &ByteView<'_>) -> Result<Vec<(i32, String)>, DecodeError> {
    let attset = class_entry(view, ORC_ATTRIBUTES_INDEX)?;
    let set = SubstructureSet::new(view, attset, "classrep.attset")?;
    let mut names = Vec::with_capacity(capacity(set.count));
    for index in 0..set.count {
        let element = set.element(view, index)?;
        let element_fixed = element
            .checked_add(var_table_size(ORC_ATT_VAR_ATT_COUNT, CLASS_OFFSET_WIDTH)?)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "classrep.att.fixed"))?;
        let id = read_i32_be(view, element_fixed + ORC_ATT_ID_OFFSET, "classrep.att.id")?;
        let (name, name_extent) =
            element_entry_span(view, element, ORC_ATT_NAME_INDEX, "classrep.att.name_entry")?;
        names.push((
            id,
            lossy_text(&read_packed_string(
                view,
                name,
                name_extent,
                "classrep.att.name",
            )?),
        ));
    }
    Ok(names)
}

/// Assigns an attribute its fixed-region offset or offset-table index, matching
/// the engine's single forward pass over the attribute list.
fn place(
    id: i32,
    name: Option<String>,
    domain: AttributeDomainFact,
    position: u32,
    fixed_count: u32,
    fixed_cursor: &mut usize,
) -> Result<ClassAttributeFact, DecodeError> {
    let (is_fixed, location) = if position < fixed_count {
        let size = domain.fixed_disk_size().ok_or_else(|| {
            // A variable-region type inside the fixed count means the fixed
            // region's width is unknowable, so nothing after it can be trusted.
            error(
                DecodeErrorKind::InvalidGeometry,
                "classrep.att.variable_in_fixed_region",
            )
        })?;
        let location = *fixed_cursor;
        *fixed_cursor = fixed_cursor
            .checked_add(size)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "classrep.att.location"))?;
        (
            true,
            u32::try_from(location)
                .map_err(|_| error(DecodeErrorKind::ArithmeticOverflow, "classrep.att.location"))?,
        )
    } else {
        (false, position - fixed_count)
    };
    Ok(ClassAttributeFact {
        id,
        name,
        domain,
        is_fixed,
        location,
        position,
    })
}

fn decode_domain(
    view: &ByteView<'_>,
    set_offset: usize,
) -> Result<AttributeDomainFact, DecodeError> {
    // A domain is itself a substructure set; the attribute's own domain is its
    // first element (`or_get_domain_internal`).
    let set = SubstructureSet::new(view, set_offset, "classrep.domain.set")?;
    if set.count == 0 {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "classrep.domain.empty",
        ));
    }
    let element = set.element(view, 0)?;
    let fixed = element
        .checked_add(var_table_size(
            ORC_DOMAIN_VAR_ATT_COUNT,
            CLASS_OFFSET_WIDTH,
        )?)
        .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "classrep.domain.fixed"))?;
    Ok(AttributeDomainFact {
        db_type: DbType::from_code(read_i32_be(
            view,
            fixed + ORC_DOMAIN_TYPE_OFFSET,
            "classrep.domain.type",
        )?),
        precision: read_i32_be(
            view,
            fixed + ORC_DOMAIN_PRECISION_OFFSET,
            "classrep.domain.precision",
        )?,
        scale: read_i32_be(
            view,
            fixed + ORC_DOMAIN_SCALE_OFFSET,
            "classrep.domain.scale",
        )?,
        codeset: read_i32_be(
            view,
            fixed + ORC_DOMAIN_CODESET_OFFSET,
            "classrep.domain.codeset",
        )?,
        collation_id: read_i32_be(
            view,
            fixed + ORC_DOMAIN_COLLATION_ID_OFFSET,
            "classrep.domain.collation",
        )?,
    })
}

/// A packed set of substructures.
struct SubstructureSet {
    start: usize,
    count: u32,
}

impl SubstructureSet {
    fn new(view: &ByteView<'_>, start: usize, rule: &'static str) -> Result<Self, DecodeError> {
        let raw = read_i32_be(
            view,
            start
                .checked_add(SET_COUNT_OFFSET)
                .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, rule))?,
            rule,
        )?;
        let count = count(raw, rule)?;
        if count > MAX_ATTRIBUTES {
            return Err(error(DecodeErrorKind::OutOfRange, rule));
        }
        // Prove the whole offset table is present before any element is read.
        let table = start
            .checked_add(SET_HEADER_SIZE)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, rule))?;
        range(
            view,
            table,
            var_table_size(count, CLASS_OFFSET_WIDTH)?,
            rule,
        )?;
        Ok(Self { start, count })
    }

    fn element(&self, view: &ByteView<'_>, index: u32) -> Result<usize, DecodeError> {
        if index >= self.count {
            return Err(error(DecodeErrorKind::OutOfRange, "classrep.set.element"));
        }
        let relative = var_entry_offset(
            view,
            self.start + SET_HEADER_SIZE,
            CLASS_OFFSET_WIDTH,
            index,
            "classrep.set.element",
        )?;
        self.start
            .checked_add(relative)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, "classrep.set.element"))
    }
}

/// Offset of variable attribute `index` of a substructure whose offset table
/// starts at the substructure itself.
fn element_entry(
    view: &ByteView<'_>,
    element: usize,
    index: u32,
    rule: &'static str,
) -> Result<usize, DecodeError> {
    let relative = var_entry_offset(view, element, CLASS_OFFSET_WIDTH, index, rule)?;
    element
        .checked_add(relative)
        .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, rule))
}

fn element_entry_span(
    view: &ByteView<'_>,
    element: usize,
    index: u32,
    rule: &'static str,
) -> Result<(usize, usize), DecodeError> {
    let start = var_entry_offset(view, element, CLASS_OFFSET_WIDTH, index, rule)?;
    let end = var_entry_offset(view, element, CLASS_OFFSET_WIDTH, index + 1, rule)?;
    if end < start {
        return Err(error(DecodeErrorKind::InvalidGeometry, rule));
    }
    Ok((
        element
            .checked_add(start)
            .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, rule))?,
        end - start,
    ))
}

/// Allocation size for a count already bounded by [`MAX_ATTRIBUTES`].
fn capacity(count: u32) -> usize {
    usize::try_from(count.min(MAX_ATTRIBUTES)).unwrap_or(0)
}

fn count(value: i32, rule: &'static str) -> Result<u32, DecodeError> {
    u32::try_from(value).map_err(|_| error(DecodeErrorKind::NegativeValue, rule))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
