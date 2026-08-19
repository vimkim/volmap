use volmap::bytes::{
    ByteAccessErrorKind, ByteView, checked_align_up, checked_mul, non_negative_i32,
};

#[test]
fn bounded_view_reads_explicit_little_endian_values() {
    let bytes = [0xaa, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12];
    let view = ByteView::new(&bytes, 100);

    assert_eq!(view.read_u16_le(1, "short").unwrap(), 0x1234);
    assert_eq!(view.read_i32_le(3, "integer").unwrap(), 0x1234_5678);
}

#[test]
fn bounded_view_reads_explicit_big_endian_values() {
    let bytes = [0x12, 0x34, 0x56, 0x78];
    let view = ByteView::new(&bytes, 0);
    assert_eq!(view.read_i16_be(0, "be16").unwrap(), 0x1234);
    assert_eq!(view.read_i32_be(0, "be32").unwrap(), 0x1234_5678);
}

#[test]
fn bounded_view_rejects_a_range_past_its_container() {
    let view = ByteView::new(&[0; 8], 4096);

    let error = view.read_u32_le(6, "crossing field").unwrap_err();

    assert_eq!(error.kind(), ByteAccessErrorKind::OutOfBounds);
    assert_eq!(error.container_len(), 8);
    assert_eq!(error.relative_offset(), 6);
    assert_eq!(error.requested_len(), 4);
}

#[test]
fn bounded_view_rejects_absolute_offset_overflow() {
    let view = ByteView::new(&[0; 2], u64::MAX);

    let error = view.read_u16_le(0, "overflowing field").unwrap_err();

    assert_eq!(error.kind(), ByteAccessErrorKind::ArithmeticOverflow);
}

#[test]
fn disk_derived_arithmetic_rejects_negative_overflow_and_invalid_alignment() {
    assert_eq!(
        non_negative_i32(-1, "count").unwrap_err().kind(),
        ByteAccessErrorKind::NegativeValue
    );
    assert_eq!(
        checked_mul(u64::MAX, 2, "extent").unwrap_err().kind(),
        ByteAccessErrorKind::ArithmeticOverflow
    );
    assert_eq!(checked_align_up(17, 8, "alignment").unwrap(), 24);
    assert_eq!(
        checked_align_up(17, 3, "alignment").unwrap_err().kind(),
        ByteAccessErrorKind::InvalidAlignment
    );
}
