//! Record interpretation against the pinned `e1e651de-records` corpus.
//!
//! Expected values are the engine's own, captured from `csql` against the
//! database `fixtures/e1e651de-records/generate.sql` builds.

use core::fmt::Write as _;

use sha2::{Digest, Sha256};
use volmap::format::{
    AttributeDomainFact, AttributeInterpretation, AttributeValue, CalendarDate, ClassAttributeFact,
    ClassRepresentationFact, ClockTime, DbType, DecodedPageEnvelope, IO_PAGE_SIZE,
    InterpretedAttribute, RepresentationTarget, decode_class_representation,
    decode_heap_record_body, decode_page_envelope, decode_record_attributes, decode_slotted_page,
};
use volmap::model::{Oid, PageId, SlotId, VolId, Vpid};

const CLASS_SCALARS: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol0-page195.bin");
const CLASS_OOS: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol0-page199.bin");
const CLASS_ALTERED: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol0-page207.bin");
const CLASS_PLACEHOLDERS: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol0-page209.bin");
const CLASS_TARGET: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol0-page210.bin");
const ROWS_SCALARS: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol1-page641.bin");
const ROWS_TARGET: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol1-page769.bin");
const ROWS_REFERENCE: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol1-page897.bin");
const ROWS_PLACEHOLDERS: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol1-page1025.bin");
const ROWS_OOS: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol1-page1153.bin");
const ROWS_ALTERED: &[u8; IO_PAGE_SIZE] =
    include_bytes!("../fixtures/e1e651de-records/pages/vol1-page1345.bin");

/// Every page in this corpus, with the digest `manifest.toml` records.
const MANIFEST: &[(&[u8; IO_PAGE_SIZE], i16, i32, &str)] = &[
    (
        CLASS_SCALARS,
        0,
        195,
        "4b65735a9b5e3a58afacd213fa063126bc6c4ce1727913917c030b8beb112359",
    ),
    (
        CLASS_OOS,
        0,
        199,
        "9ffd211397854aa6d501984550a93f023a73e749e6c808f3bad353a5155e8c8a",
    ),
    (
        CLASS_ALTERED,
        0,
        207,
        "712ebb9576e232e3ecc29b4e7c4de259a75579e5d69723d1d3a1a579fce91672",
    ),
    (
        CLASS_PLACEHOLDERS,
        0,
        209,
        "037ad9279e02e46120c32db4f241993d348da0dd4bb25f71f717b9bfa8da78ca",
    ),
    (
        CLASS_TARGET,
        0,
        210,
        "6201df681892519af31a5c3be112b34eb011d06a3e96ac0a660843071c87cb22",
    ),
    (
        ROWS_SCALARS,
        1,
        641,
        "eeb6e79d4b3c2d133f67d4085dee7bb8f04a8313a2a116878ca8381d0ec72495",
    ),
    (
        ROWS_TARGET,
        1,
        769,
        "582c79551c94857c86069f424c02066d19c85f3864d1e06ba6dc9fea26588c24",
    ),
    (
        ROWS_REFERENCE,
        1,
        897,
        "9527b8b00967f79c6a7e268763b8b74835c3f9173f763147eed25daf1acf154f",
    ),
    (
        ROWS_PLACEHOLDERS,
        1,
        1025,
        "79bc1f482a67228c4d2ac4f54c9bf7d19963796b2ab72b870f0fc75dc9e63e30",
    ),
    (
        ROWS_OOS,
        1,
        1153,
        "cc167f6f0336ff2a2869ff85555a77b4d96faa2c1a107c08650f137068220b7b",
    ),
    (
        ROWS_ALTERED,
        1,
        1345,
        "1b1e7f84bf24451be5febe79868151ca182f24c8fa9f7b22e78e0a4eb4dbe079",
    ),
];

fn envelope(
    bytes: &'static [u8; IO_PAGE_SIZE],
    vol: i16,
    page: i32,
) -> DecodedPageEnvelope<'static> {
    decode_page_envelope(
        bytes,
        Vpid::new(VolId::new(vol).unwrap(), PageId::new(page).unwrap()),
    )
    .unwrap()
}

/// Parses the representation a class object's record describes.
fn representation(
    bytes: &'static [u8; IO_PAGE_SIZE],
    vol: i16,
    page: i32,
    slot: u16,
    target: RepresentationTarget,
) -> ClassRepresentationFact {
    let envelope = envelope(bytes, vol, page);
    let slotted = decode_slotted_page(&envelope).unwrap();
    let (header, body) = decode_heap_record_body(&envelope, &slotted, slot, true).unwrap();
    decode_class_representation(
        body,
        header.variable_offset_width,
        header.representation_id,
        target,
    )
    .unwrap()
}

/// Interprets one row against a representation.
fn row(
    bytes: &'static [u8; IO_PAGE_SIZE],
    vol: i16,
    page: i32,
    slot: u16,
    representation: &ClassRepresentationFact,
) -> Vec<InterpretedAttribute> {
    let envelope = envelope(bytes, vol, page);
    let slotted = decode_slotted_page(&envelope).unwrap();
    let (header, body) = decode_heap_record_body(&envelope, &slotted, slot, true).unwrap();
    decode_record_attributes(
        body,
        header.variable_offset_width,
        header.has_bound_bits,
        representation,
    )
    .unwrap()
}

fn named<'a>(row: &'a [InterpretedAttribute], name: &str) -> &'a AttributeInterpretation {
    &row.iter()
        .find(|attribute| attribute.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no attribute named {name}"))
        .interpretation
}

fn decoded<'a>(row: &'a [InterpretedAttribute], name: &str) -> &'a AttributeValue {
    match named(row, name) {
        AttributeInterpretation::Decoded(value) => value,
        other => panic!("{name} is {other:?}, expected a decoded value"),
    }
}

fn text(row: &[InterpretedAttribute], name: &str) -> String {
    match decoded(row, name) {
        AttributeValue::Text(text) => text.clone(),
        other => panic!("{name} is {other:?}, expected text"),
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut encoded, "{byte:02x}").unwrap();
    }
    encoded
}

#[test]
fn corpus_pages_match_their_manifest_digests() {
    for (bytes, vol, page, expected) in MANIFEST {
        assert_eq!(sha256(*bytes), *expected, "vol{vol} page{page}");
    }
}

#[test]
fn class_records_yield_their_names_and_attribute_layout() {
    let scalars = representation(CLASS_SCALARS, 0, 195, 2, RepresentationTarget::Current);
    assert_eq!(scalars.class_name, "dba.interp_scalars");
    assert!(scalars.is_current);
    assert_eq!(scalars.attributes.len(), 15);
    assert_eq!((scalars.fixed_count, scalars.variable_count), (10, 5));

    // Representation order is the engine's storage order, not declaration
    // order: it groups the fixed region by width. Decoding must follow each
    // attribute's own position and location rather than the DDL.
    let fixed: Vec<&str> = scalars
        .attributes
        .iter()
        .filter(|attribute| attribute.is_fixed)
        .map(|attribute| attribute.name.as_deref().unwrap())
        .collect();
    assert_eq!(
        fixed,
        [
            "c_timestamp",
            "c_time",
            "c_date",
            "c_float",
            "id",
            "c_datetime",
            "c_double",
            "c_bigint",
            "c_monetary",
            "c_short",
        ]
    );

    // CHAR and NUMERIC are variable-region types despite their fixed precision.
    let variable: Vec<&str> = scalars
        .attributes
        .iter()
        .filter(|attribute| !attribute.is_fixed)
        .map(|attribute| attribute.name.as_deref().unwrap())
        .collect();
    assert_eq!(
        variable,
        ["c_varnchar", "c_nchar", "c_varchar", "c_numeric", "c_char"]
    );

    let by_name = |name: &str| {
        scalars
            .attributes
            .iter()
            .find(|attribute| attribute.name.as_deref() == Some(name))
            .unwrap()
            .clone()
    };
    assert_eq!(by_name("c_numeric").domain.db_type, DbType::Numeric);
    assert_eq!(by_name("c_numeric").domain.precision, 10);
    assert_eq!(by_name("c_numeric").domain.scale, 2);
    assert_eq!(by_name("c_char").domain.db_type, DbType::Char);
    assert_eq!(by_name("c_char").domain.precision, 8);

    // NCHAR was deprecated: the engine stores national columns under the CHAR
    // and VARCHAR type codes, so a decoder never sees DB_TYPE_VARNCHAR.
    assert_eq!(by_name("c_nchar").domain.db_type, DbType::Char);
    assert_eq!(by_name("c_varnchar").domain.db_type, DbType::String);

    // Fixed locations are cumulative disk sizes in representation order, and
    // the last one plus its width must align to the declared fixed length.
    assert_eq!(by_name("c_timestamp").location, 0);
    assert_eq!(by_name("c_time").location, 4);
    assert_eq!(by_name("id").location, 16);
    assert_eq!(by_name("c_datetime").location, 20);
    assert_eq!(by_name("c_monetary").location, 44);
    assert_eq!(by_name("c_short").location, 56);
    assert_eq!(scalars.fixed_length, 60);

    let placeholders = representation(CLASS_PLACEHOLDERS, 0, 209, 3, RepresentationTarget::Current);
    assert_eq!(placeholders.class_name, "dba.interp_placeholders");
    let target = representation(CLASS_TARGET, 0, 210, 3, RepresentationTarget::Current);
    assert_eq!(target.class_name, "dba.interp_target");
    let oos = representation(CLASS_OOS, 0, 199, 1, RepresentationTarget::Current);
    assert_eq!(oos.class_name, "dba.interp_oos");
}

#[test]
fn every_supported_type_decodes_to_the_value_the_engine_stored() {
    let scalars = representation(CLASS_SCALARS, 0, 195, 2, RepresentationTarget::Current);
    let first = row(ROWS_SCALARS, 1, 641, 1, &scalars);

    assert_eq!(decoded(&first, "id"), &AttributeValue::Integer(1));
    assert_eq!(decoded(&first, "c_short"), &AttributeValue::Short(-32_768));
    assert_eq!(
        decoded(&first, "c_bigint"),
        &AttributeValue::BigInt(-9_223_372_036_854_775_807)
    );
    assert_eq!(decoded(&first, "c_float"), &AttributeValue::Float(1.25));
    assert_eq!(decoded(&first, "c_double"), &AttributeValue::Double(-2.5));
    assert_eq!(
        decoded(&first, "c_monetary"),
        &AttributeValue::Monetary {
            currency_code: 0,
            amount: 1_234.56
        }
    );
    assert_eq!(
        decoded(&first, "c_date"),
        &AttributeValue::Date(CalendarDate {
            year: 2026,
            month: 8,
            day: 21
        })
    );
    assert_eq!(
        decoded(&first, "c_time"),
        &AttributeValue::Time(ClockTime {
            hour: 13,
            minute: 45,
            second: 59
        })
    );
    assert_eq!(
        decoded(&first, "c_datetime"),
        &AttributeValue::DateTime {
            date: CalendarDate {
                year: 2026,
                month: 8,
                day: 21
            },
            time: ClockTime {
                hour: 13,
                minute: 45,
                second: 59
            },
            millisecond: 123
        }
    );

    // NUMERIC(10,2) keeps its scale, sign, and full precision as text.
    assert_eq!(
        decoded(&first, "c_numeric"),
        &AttributeValue::Numeric("-12345678.90".to_owned())
    );

    // CHAR is space-padded to its declared precision before storage.
    assert_eq!(text(&first, "c_char"), "fixed8ch");
    assert_eq!(text(&first, "c_varchar"), "plain varchar value");
    assert_eq!(text(&first, "c_nchar"), "nchar8ch");
    assert_eq!(text(&first, "c_varnchar"), "varnchar value");

    // TIMESTAMP is Unix epoch seconds; only its presence is pinned here
    // because the stored instant depends on the generator's time zone.
    assert!(matches!(
        decoded(&first, "c_timestamp"),
        AttributeValue::Timestamp(_)
    ));
}

#[test]
fn unset_attributes_report_null_from_bound_bits_and_empty_extents() {
    let scalars = representation(CLASS_SCALARS, 0, 195, 2, RepresentationTarget::Current);
    let all_null = row(ROWS_SCALARS, 1, 641, 2, &scalars);

    // The primary key stays bound; every other column is NULL.
    assert_eq!(decoded(&all_null, "id"), &AttributeValue::Integer(2));
    for name in [
        "c_short",
        "c_bigint",
        "c_float",
        "c_double",
        "c_monetary",
        "c_date",
        "c_time",
        "c_timestamp",
        "c_datetime",
        "c_char",
        "c_numeric",
        "c_varchar",
        "c_nchar",
        "c_varnchar",
    ] {
        assert_eq!(
            named(&all_null, name),
            &AttributeInterpretation::Null,
            "{name} should be NULL"
        );
    }
}

#[test]
fn a_compressed_varchar_decodes_through_the_lz4_prefix() {
    let scalars = representation(CLASS_SCALARS, 0, 195, 2, RepresentationTarget::Current);
    let compressed = row(ROWS_SCALARS, 1, 641, 3, &scalars);

    let expected = "compressible-varchar-payload-".repeat(40);
    assert_eq!(expected.len(), 1_160);
    assert_eq!(text(&compressed, "c_varchar"), expected);

    // The whole record is far smaller than the value it carries, which is what
    // proves the stored form was compressed rather than raw.
    let envelope = envelope(ROWS_SCALARS, 1, 641);
    let slotted = decode_slotted_page(&envelope).unwrap();
    let record_length = slotted.slots()[3].length();
    assert!(
        usize::from(record_length) < expected.len(),
        "record of {record_length} bytes should be smaller than its 1160-byte value"
    );

    assert_eq!(
        decoded(&compressed, "c_numeric"),
        &AttributeValue::Numeric("3.00".to_owned())
    );
}

#[test]
fn types_version_one_refuses_to_decode_render_typed_placeholders_without_bytes() {
    let placeholders = representation(CLASS_PLACEHOLDERS, 0, 209, 3, RepresentationTarget::Current);
    let bound = row(ROWS_PLACEHOLDERS, 1, 1025, 1, &placeholders);

    for (name, expected_type) in [
        ("c_set", DbType::Set),
        ("c_seq", DbType::Sequence),
        ("c_enum", DbType::Enumeration),
        ("c_bit", DbType::Bit),
        ("c_varbit", DbType::VarBit),
        ("c_json", DbType::Json),
    ] {
        let attribute = bound
            .iter()
            .find(|attribute| attribute.name.as_deref() == Some(name))
            .unwrap();
        assert_eq!(attribute.domain.db_type, expected_type, "{name} type");
        let AttributeInterpretation::Undecodable { reason, length, .. } = attribute.interpretation
        else {
            panic!(
                "{name} is {:?}, expected a placeholder",
                attribute.interpretation
            );
        };
        // A placeholder states the extent but never the bytes in it.
        assert!(length > 0, "{name} should report its extent");
        assert!(!reason.is_empty(), "{name} should explain itself");
    }

    // The same columns left NULL are NULL, not placeholders: an absent value
    // outranks an undecodable type.
    let unset = row(ROWS_PLACEHOLDERS, 1, 1025, 2, &placeholders);
    for name in ["c_set", "c_seq", "c_enum", "c_bit", "c_varbit", "c_json"] {
        assert_eq!(
            named(&unset, name),
            &AttributeInterpretation::Null,
            "{name}"
        );
    }
}

#[test]
fn an_out_of_row_attribute_reports_its_chain_head_and_full_length() {
    let oos = representation(CLASS_OOS, 0, 199, 1, RepresentationTarget::Current);

    let inline = row(ROWS_OOS, 1, 1153, 1, &oos);
    assert_eq!(text(&inline, "label"), "inline");
    // A short BIT VARYING stays in the record, where its type still withholds it.
    assert!(matches!(
        named(&inline, "out_value"),
        AttributeInterpretation::Undecodable { .. }
    ));

    let demoted = row(ROWS_OOS, 1, 1153, 2, &oos);
    assert_eq!(text(&demoted, "label"), "out-of-row");
    let AttributeInterpretation::OutOfRow { head, total_length } = named(&demoted, "out_value")
    else {
        panic!(
            "expected an out-of-row reference, got {:?}",
            named(&demoted, "out_value")
        );
    };
    assert_eq!(*total_length, 32_776);
    assert_eq!(head.vol_id.get(), 1);
}

#[test]
fn a_bound_object_column_decodes_to_the_oid_it_references() {
    let reference = representation(CLASS_SCALARS, 0, 195, 5, RepresentationTarget::Current);
    assert_eq!(reference.class_name, "dba.interp_reference");

    let bound = row(ROWS_REFERENCE, 1, 897, 1, &reference);
    let AttributeValue::Object(oid) = decoded(&bound, "target") else {
        panic!("expected an OBJECT value");
    };

    // The reference resolves to the single row of interp_target.
    let target_representation =
        representation(CLASS_TARGET, 0, 210, 3, RepresentationTarget::Current);
    let target_row = row(ROWS_TARGET, 1, 769, 1, &target_representation);
    assert_eq!(text(&target_row, "label"), "target-one");
    assert_eq!(
        *oid,
        Oid::new(
            VolId::new(1).unwrap(),
            PageId::new(769).unwrap(),
            SlotId::new(1).unwrap()
        )
    );

    let unset = row(ROWS_REFERENCE, 1, 897, 2, &reference);
    assert_eq!(named(&unset, "target"), &AttributeInterpretation::Null);
}

#[test]
fn a_row_written_before_an_alter_interprets_against_its_own_old_representation() {
    let envelope = envelope(ROWS_ALTERED, 1, 1345);
    let slotted = decode_slotted_page(&envelope).unwrap();
    let (before, _) = decode_heap_record_body(&envelope, &slotted, 1, true).unwrap();
    let (after, _) = decode_heap_record_body(&envelope, &slotted, 2, true).unwrap();
    assert_eq!(
        (before.representation_id, after.representation_id),
        (1, 2),
        "the ALTER should have advanced the representation id"
    );

    let current = representation(CLASS_ALTERED, 0, 207, 4, RepresentationTarget::Current);
    assert_eq!(current.representation_id, 2);
    assert!(current.is_current);

    let old = representation(
        CLASS_ALTERED,
        0,
        207,
        4,
        RepresentationTarget::Id(before.representation_id),
    );
    assert!(!old.is_current);
    assert_eq!(old.representation_id, 1);
    // The added column exists only in the current representation.
    assert_eq!(
        current.attributes.len(),
        old.attributes.len() + 1,
        "the ALTER added exactly one attribute"
    );

    // Interpreting the pre-ALTER row against its own representation reads its
    // stored values; the column added later is simply absent.
    let historical = row(ROWS_ALTERED, 1, 1345, 1, &old);
    assert_eq!(decoded(&historical, "id"), &AttributeValue::Integer(1));
    assert_eq!(
        decoded(&historical, "pre_alter"),
        &AttributeValue::Integer(11)
    );
    assert_eq!(text(&historical, "label"), "before-alter");
    assert!(
        !historical
            .iter()
            .any(|attribute| attribute.name.as_deref() == Some("post_alter")),
        "post_alter did not exist in representation 1"
    );

    // The post-ALTER row carries the new column under the current representation.
    let recent = row(ROWS_ALTERED, 1, 1345, 2, &current);
    assert_eq!(decoded(&recent, "id"), &AttributeValue::Integer(2));
    assert_eq!(decoded(&recent, "pre_alter"), &AttributeValue::Integer(22));
    assert_eq!(text(&recent, "label"), "after-alter");
    assert_eq!(
        decoded(&recent, "post_alter"),
        &AttributeValue::Integer(222)
    );
}

#[test]
fn an_unknown_representation_id_is_rejected_rather_than_guessed() {
    let envelope = envelope(CLASS_ALTERED, 0, 207);
    let slotted = decode_slotted_page(&envelope).unwrap();
    let (header, body) = decode_heap_record_body(&envelope, &slotted, 4, true).unwrap();
    let error = decode_class_representation(
        body,
        header.variable_offset_width,
        header.representation_id,
        RepresentationTarget::Id(9_999),
    )
    .unwrap_err();
    assert_eq!(error.rule(), "classrep.rep.unknown_id");
}

#[test]
fn system_classes_parse_from_the_same_class_record_layout() {
    // D2 puts every class reachable by a valid class OID in scope, system
    // classes included. Page 195 carries three besides the fixture classes,
    // and `_db_serial` is the interesting one: its `current_val NUMERIC(38)`
    // is the widest NUMERIC domain the engine has, so its stored total size
    // reaches the maximum a NUMERIC header may claim.
    let serial = representation(CLASS_SCALARS, 0, 195, 3, RepresentationTarget::Current);
    assert_eq!(serial.class_name, "_db_serial");
    assert!(
        serial
            .attributes
            .iter()
            .any(|attribute| attribute.name.as_deref() == Some("current_val")
                && attribute.domain.db_type == DbType::Numeric
                && attribute.domain.precision == 38),
        "expected _db_serial.current_val as NUMERIC(38), got {:?}",
        serial
            .attributes
            .iter()
            .map(|attribute| (attribute.name.clone(), attribute.domain.db_type))
            .collect::<Vec<_>>()
    );

    // A system class stored as REC_NEWHOME and one stored as REC_HOME both
    // parse: the record type is the slot's business, not the parser's.
    let ha = representation(CLASS_SCALARS, 0, 195, 4, RepresentationTarget::Current);
    assert_eq!(ha.class_name, "_db_ha_apply_info");
    let dual = representation(CLASS_SCALARS, 0, 195, 1, RepresentationTarget::Current);
    assert_eq!(dual.class_name, "dual");
}

#[test]
fn a_numeric_at_the_engines_widest_stored_size_decodes() {
    // The fixture's NUMERIC(10,2) stores a total of 8 bytes, so it cannot reach
    // the widest form the engine emits: `_gv_mr_*_numeric_*_to_size` top out at
    // 20, which is a 3-byte header plus a 17-byte magnitude. A cap set at 17
    // instead of 20 silently withholds every high-precision NUMERIC, so the
    // boundary is pinned here rather than left to a fixture that lacks one.
    const TOTAL: u8 = 20;
    let mut numeric = vec![0_u8; usize::from(TOTAL)];
    numeric[0] = TOTAL; // total size, positive
    numeric[1] = 38; // precision, non-negative scale
    numeric[2] = 2; // scale
    numeric[usize::from(TOTAL) - 1] = 42; // magnitude 42, scaled to 0.42

    let mut body = Vec::new();
    body.extend_from_slice(&8_u32.to_be_bytes()); // value starts after the table
    body.extend_from_slice(&(8 + u32::from(TOTAL)).to_be_bytes()); // and ends here
    body.extend_from_slice(&numeric);

    let representation = ClassRepresentationFact {
        class_name: "synthetic".to_owned(),
        representation_id: 1,
        is_current: true,
        fixed_count: 0,
        variable_count: 1,
        fixed_length: 0,
        attributes: vec![ClassAttributeFact {
            id: 0,
            name: Some("widest".to_owned()),
            domain: AttributeDomainFact {
                db_type: DbType::Numeric,
                precision: 38,
                scale: 2,
                codeset: 0,
                collation_id: 0,
            },
            is_fixed: false,
            location: 0,
            position: 0,
        }],
    };

    let attributes = decode_record_attributes(&body, 4, false, &representation).unwrap();
    assert_eq!(
        attributes[0].interpretation,
        AttributeInterpretation::Decoded(AttributeValue::Numeric("0.42".to_owned()))
    );

    // One byte past the engine's maximum is not a NUMERIC and stays withheld.
    let mut overlong = body.clone();
    overlong[8] = TOTAL + 1;
    let attributes = decode_record_attributes(&overlong, 4, false, &representation).unwrap();
    assert!(matches!(
        attributes[0].interpretation,
        AttributeInterpretation::Undecodable { .. }
    ));
}

#[test]
fn a_truncated_class_record_is_rejected_at_every_length() {
    let envelope = envelope(CLASS_SCALARS, 0, 195);
    let slotted = decode_slotted_page(&envelope).unwrap();
    let (header, body) = decode_heap_record_body(&envelope, &slotted, 2, true).unwrap();
    let full = decode_class_representation(
        body,
        header.variable_offset_width,
        header.representation_id,
        RepresentationTarget::Current,
    )
    .unwrap();

    // Every prefix short of the whole record must fail rather than return a
    // partial representation, which would silently mislocate every attribute.
    for length in 0..body.len() {
        let truncated = &body[..length];
        if let Ok(partial) = decode_class_representation(
            truncated,
            header.variable_offset_width,
            header.representation_id,
            RepresentationTarget::Current,
        ) {
            assert_eq!(
                partial, full,
                "a {length}-byte prefix decoded to a different representation"
            );
        }
    }
}

#[test]
fn corrupting_any_single_byte_of_a_record_never_panics_or_invents_a_value() {
    let scalars = representation(CLASS_SCALARS, 0, 195, 2, RepresentationTarget::Current);
    let envelope = envelope(ROWS_SCALARS, 1, 641);
    let slotted = decode_slotted_page(&envelope).unwrap();
    let (header, body) = decode_heap_record_body(&envelope, &slotted, 1, true).unwrap();

    // Sweeping every byte reaches the hostile cases without hard-coding where
    // they live: a NUMERIC size byte claiming more than its extent, a string
    // prefix claiming a compressed length it does not have, an offset-table
    // entry pointing outside the record.
    for index in 0..body.len() {
        for replacement in [0x00_u8, 0x7f, 0xff] {
            let mut damaged = body.to_vec();
            damaged[index] = replacement;
            let Ok(attributes) = decode_record_attributes(
                &damaged,
                header.variable_offset_width,
                header.has_bound_bits,
                &scalars,
            ) else {
                continue;
            };
            for attribute in &attributes {
                // Whatever a damaged record yields, a withheld value must never
                // carry bytes and text must stay within the record's own size.
                if let AttributeInterpretation::Decoded(AttributeValue::Text(text)) =
                    &attribute.interpretation
                {
                    assert!(
                        text.len() <= body.len() * 256,
                        "byte {index}={replacement:#x} produced {} bytes of text from a {} byte record",
                        text.len(),
                        body.len()
                    );
                }
            }
        }
    }
}

#[test]
fn arbitrary_bytes_never_panic_in_either_decoder() {
    // A cheap always-on stand-in for the fuzz target: deterministic pseudo
    // random bodies across every offset width and representation target.
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        u8::try_from(state & 0xff).unwrap()
    };
    let scalars = representation(CLASS_SCALARS, 0, 195, 2, RepresentationTarget::Current);

    for length in [0_usize, 1, 7, 8, 17, 64, 200, 1_000] {
        for _ in 0..64 {
            let body: Vec<u8> = (0..length).map(|_| next()).collect();
            for width in [0_u8, 1, 2, 3, 4, 255] {
                for target in [RepresentationTarget::Current, RepresentationTarget::Id(1)] {
                    let _ = decode_class_representation(&body, width, 1, target);
                }
                for has_bound_bits in [false, true] {
                    let _ = decode_record_attributes(&body, width, has_bound_bits, &scalars);
                }
            }
        }
    }
}

#[test]
fn a_class_record_offset_width_other_than_four_is_refused() {
    let envelope = envelope(CLASS_SCALARS, 0, 195);
    let slotted = decode_slotted_page(&envelope).unwrap();
    let (header, body) = decode_heap_record_body(&envelope, &slotted, 2, true).unwrap();
    assert_eq!(header.variable_offset_width, 4);
    for width in [1_u8, 2] {
        let error = decode_class_representation(
            body,
            width,
            header.representation_id,
            RepresentationTarget::Current,
        )
        .unwrap_err();
        assert_eq!(error.rule(), "classrep.record.offset_width");
    }
}
