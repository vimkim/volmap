use volmap::format::{
    DecodeErrorKind, IO_PAGE_SIZE, PageType, VolumePurpose, VolumeType, decode_page_envelope,
    decode_volume_header,
};
use volmap::model::{PageId, VolId, Vpid};

const USER_START: usize = 32;
const FILE_LENGTH: u64 = 64 * 64 * IO_PAGE_SIZE as u64;

fn page_zero() -> Vpid {
    Vpid::new(VolId::new(0).unwrap(), PageId::new(0).unwrap())
}

fn valid_volume_header_page() -> Vec<u8> {
    let mut page = vec![0_u8; IO_PAGE_SIZE];
    let lsa = 42_u64.to_le_bytes();
    page[0..8].copy_from_slice(&lsa);
    page[8..12].copy_from_slice(&0_i32.to_le_bytes());
    page[12..14].copy_from_slice(&0_i16.to_le_bytes());
    page[14] = PageType::VolumeHeader.ordinal();
    page[IO_PAGE_SIZE - 8..].copy_from_slice(&lsa);

    let user = &mut page[USER_START..IO_PAGE_SIZE - 8];
    user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
    user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
    user[28..30].copy_from_slice(&0_i16.to_le_bytes());
    user[30] = 5;
    user[32..36].copy_from_slice(&0_i32.to_le_bytes());
    user[36..40].copy_from_slice(&0_i32.to_le_bytes());
    user[40..44].copy_from_slice(&64_i32.to_le_bytes());
    user[44..48].copy_from_slice(&64_i32.to_le_bytes());
    user[48..52].copy_from_slice(&64_i32.to_le_bytes());
    user[52..56].copy_from_slice(&(-1_i32).to_le_bytes());
    user[56..60].copy_from_slice(&1_i32.to_le_bytes());
    user[60..64].copy_from_slice(&1_i32.to_le_bytes());
    user[64..68].copy_from_slice(&1_i32.to_le_bytes());
    user[124..126].copy_from_slice(&(-1_i16).to_le_bytes());
    user[126..128].copy_from_slice(&0_i16.to_le_bytes());
    user[128..130].copy_from_slice(&4_i16.to_le_bytes());
    user[130..132].copy_from_slice(&5_i16.to_le_bytes());
    user[132..140].copy_from_slice(b"vol\0\0ok\0");
    page
}

#[test]
fn valid_volume_header_decodes_pinned_geometry_and_bounded_strings() {
    let bytes = valid_volume_header_page();
    let envelope = decode_page_envelope(&bytes, page_zero()).unwrap();

    let header = decode_volume_header(&envelope, FILE_LENGTH).unwrap();

    assert_eq!(header.vol_id(), VolId::new(0).unwrap());
    assert_eq!(header.purpose(), VolumePurpose::PermanentData);
    assert_eq!(header.volume_type(), VolumeType::Permanent);
    assert_eq!(header.total_sectors(), 64);
    assert_eq!(header.maximum_sectors(), 64);
    assert_eq!(header.bitmap_page_count(), 1);
    assert_eq!(header.current_volume_name().as_bytes(), b"vol");
    assert_eq!(header.next_volume_name().as_bytes(), b"");
    assert_eq!(header.remarks().as_bytes(), b"ok");
}

#[test]
fn invalid_volume_header_fields_are_rejected_without_clamping() {
    let cases: &[(usize, &[u8], DecodeErrorKind)] = &[
        (USER_START, b"NOTRID", DecodeErrorKind::InvalidMagic),
        (
            USER_START + 32,
            &9_i32.to_le_bytes(),
            DecodeErrorKind::UnknownEnum,
        ),
        (
            USER_START + 44,
            &63_i32.to_le_bytes(),
            DecodeErrorKind::InvalidGeometry,
        ),
        (
            USER_START + 126,
            &(-1_i16).to_le_bytes(),
            DecodeErrorKind::InvalidStringTable,
        ),
    ];

    for (offset, replacement, expected) in cases {
        let mut bytes = valid_volume_header_page();
        bytes[*offset..*offset + replacement.len()].copy_from_slice(replacement);
        let envelope = decode_page_envelope(&bytes, page_zero()).unwrap();
        assert_eq!(
            decode_volume_header(&envelope, FILE_LENGTH)
                .unwrap_err()
                .kind(),
            *expected
        );
    }
}

#[test]
fn volume_header_rejects_truncated_physical_volume() {
    let bytes = valid_volume_header_page();
    let envelope = decode_page_envelope(&bytes, page_zero()).unwrap();

    assert_eq!(
        decode_volume_header(&envelope, FILE_LENGTH - IO_PAGE_SIZE as u64)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::FileLengthInvalid
    );
}

#[test]
fn volume_header_rejects_bitmap_pages_beyond_the_physical_volume() {
    let mut bytes = valid_volume_header_page();
    let user = &mut bytes[USER_START..IO_PAGE_SIZE - 8];
    let maximum_sectors = 2_147_483_584_i32;
    let bitmap_pages = 16_425_i32;
    user[48..52].copy_from_slice(&maximum_sectors.to_le_bytes());
    user[56..60].copy_from_slice(&bitmap_pages.to_le_bytes());
    user[64..68].copy_from_slice(&bitmap_pages.to_le_bytes());
    let envelope = decode_page_envelope(&bytes, page_zero()).unwrap();

    assert_eq!(
        decode_volume_header(&envelope, FILE_LENGTH)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::FileLengthInvalid
    );
}

#[test]
fn volume_header_never_parses_an_encrypted_user_region() {
    let mut bytes = valid_volume_header_page();
    bytes[15] = 0x01;
    let envelope = decode_page_envelope(&bytes, page_zero()).unwrap();

    assert_eq!(
        decode_volume_header(&envelope, FILE_LENGTH)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::EncryptedOpaque
    );
}
