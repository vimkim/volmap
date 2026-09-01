use volmap::format::{
    DecodeErrorKind, IO_PAGE_SIZE, decode_class_record_name, decode_heap_record_body,
    decode_page_envelope, decode_slotted_page,
};
use volmap::model::{PageId, VolId, Vpid};

const CLASS_VARIABLE_ATTRIBUTES: usize = 17;
const CLASS_TABLE_SIZE: usize = (CLASS_VARIABLE_ATTRIBUTES + 1) * 4;
const CLASS_FIXED_REGION_SIZE: usize = 88;
const CLASS_VARIABLE_START: usize = CLASS_TABLE_SIZE + CLASS_FIXED_REGION_SIZE;

fn class_body(attribute: &[u8]) -> Vec<u8> {
    let end = (CLASS_VARIABLE_START + attribute.len() + 3) & !3;
    let mut body = vec![0_u8; end];
    body[0..4].copy_from_slice(&u32::try_from(CLASS_VARIABLE_START).unwrap().to_be_bytes());
    for entry in 1..=CLASS_VARIABLE_ATTRIBUTES {
        let at = entry * 4;
        body[at..at + 4].copy_from_slice(&u32::try_from(end).unwrap().to_be_bytes());
    }
    body[CLASS_VARIABLE_START..CLASS_VARIABLE_START + attribute.len()].copy_from_slice(attribute);
    body
}

#[test]
fn class_name_decoder_handles_short_long_and_compressed_varchars() {
    let short = class_body(b"\x0bowner.table\0");
    assert_eq!(decode_class_record_name(&short, 4).unwrap(), b"owner.table");

    let long_name = vec![b'l'; 255];
    let mut long_attribute = vec![0xff];
    long_attribute.extend_from_slice(&0_i32.to_be_bytes());
    long_attribute.extend_from_slice(&255_i32.to_be_bytes());
    long_attribute.extend_from_slice(&long_name);
    long_attribute.push(0);
    assert_eq!(
        decode_class_record_name(&class_body(&long_attribute), 4).unwrap(),
        long_name
    );

    let compressed_name = vec![b'c'; 255];
    let compressed = lz4_flex::block::compress(&compressed_name);
    let mut compressed_attribute = vec![0xff];
    compressed_attribute.extend_from_slice(&i32::try_from(compressed.len()).unwrap().to_be_bytes());
    compressed_attribute.extend_from_slice(&255_i32.to_be_bytes());
    compressed_attribute.extend_from_slice(&compressed);
    compressed_attribute.push(0);
    assert_eq!(
        decode_class_record_name(&class_body(&compressed_attribute), 4).unwrap(),
        compressed_name
    );
}

#[test]
fn class_name_decoder_rejects_hostile_offsets_lengths_and_terminators() {
    let valid = class_body(b"\x05table\0");
    let error = decode_class_record_name(&valid, 2).unwrap_err();
    assert_eq!(error.rule(), "class.name.offset_width");

    let mut out_of_row = valid.clone();
    let flagged = (u32::try_from(CLASS_VARIABLE_START).unwrap() | 1).to_be_bytes();
    out_of_row[0..4].copy_from_slice(&flagged);
    assert_eq!(
        decode_class_record_name(&out_of_row, 4).unwrap_err().rule(),
        "class.name.out_of_row"
    );

    let mut inverted = valid.clone();
    inverted[4..8].copy_from_slice(&64_u32.to_be_bytes());
    assert_eq!(
        decode_class_record_name(&inverted, 4).unwrap_err().kind(),
        DecodeErrorKind::InvalidGeometry
    );

    let mut missing_nul = class_body(b"\x05tableX");
    missing_nul[CLASS_VARIABLE_START + 6] = b'X';
    assert_eq!(
        decode_class_record_name(&missing_nul, 4)
            .unwrap_err()
            .rule(),
        "class.name.terminator"
    );

    let truncated_long = class_body(b"\xff\0\0\0\x08\0\0\0\xffshort\0");
    assert!(decode_class_record_name(&truncated_long, 4).is_err());

    let mut truncated_fixed_region = valid.clone();
    truncated_fixed_region[0..4]
        .copy_from_slice(&u32::try_from(CLASS_TABLE_SIZE).unwrap().to_be_bytes());
    assert_eq!(
        decode_class_record_name(&truncated_fixed_region, 4)
            .unwrap_err()
            .rule(),
        "class.name.var_table_order"
    );

    let compressed_over_limit = class_body(b"\xff\0\0\0\x01\0\0\x01\0X\0");
    assert_eq!(
        decode_class_record_name(&compressed_over_limit, 4)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidLength
    );
}

#[test]
fn class_name_decoder_rejects_corrupt_long_varchar_length_pairs() {
    fn long_attribute(compressed: i32, decompressed: i32, payload: &[u8]) -> Vec<u8> {
        let mut attribute = vec![0xff];
        attribute.extend_from_slice(&compressed.to_be_bytes());
        attribute.extend_from_slice(&decompressed.to_be_bytes());
        attribute.extend_from_slice(payload);
        attribute.push(0);
        attribute
    }

    for (label, attribute, expected_kind) in [
        (
            "negative compressed length",
            long_attribute(-1, 1, b"X"),
            DecodeErrorKind::NegativeValue,
        ),
        (
            "negative decompressed length",
            long_attribute(0, -1, b""),
            DecodeErrorKind::NegativeValue,
        ),
        (
            "truncated uncompressed payload",
            long_attribute(0, 255, b"short"),
            DecodeErrorKind::ByteAccess,
        ),
        (
            "truncated compressed payload",
            long_attribute(32, 255, b"short"),
            DecodeErrorKind::ByteAccess,
        ),
        (
            "decompressed identifier over limit",
            long_attribute(1, 256, b"X"),
            DecodeErrorKind::InvalidLength,
        ),
        (
            "invalid compressed payload",
            long_attribute(1, 255, b"X"),
            DecodeErrorKind::InvalidLength,
        ),
    ] {
        let error = decode_class_record_name(&class_body(&attribute), 4).unwrap_err();
        assert_eq!(error.kind(), expected_kind, "{label}");
        assert_eq!(error.rule(), "class.name.varchar", "{label}");
    }
}

#[test]
fn class_name_decoder_matches_a_pinned_engine_class_record() {
    const PAGE: &[u8; IO_PAGE_SIZE] =
        include_bytes!("../fixtures/e1e651de-records/pages/vol0-page195.bin");
    let vpid = Vpid::new(VolId::new(0).unwrap(), PageId::new(195).unwrap());
    let envelope = decode_page_envelope(PAGE, vpid).unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    let (record, body) = decode_heap_record_body(&envelope, &slotted, 2, true).unwrap();
    assert_eq!(
        decode_class_record_name(body, record.variable_offset_width).unwrap(),
        b"dba.interp_scalars"
    );
}
