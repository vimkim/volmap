use volmap::format::{
    DecodeErrorKind, IO_PAGE_SIZE, PageType, decode_page_envelope, decode_sector_bitmap,
    decode_volume_header,
};
use volmap::model::{PageId, SectorId, VolId, Vpid};

fn vpid(page_id: i32) -> Vpid {
    Vpid::new(VolId::new(0).unwrap(), PageId::new(page_id).unwrap())
}

fn envelope_page(id: Vpid, page_type: PageType) -> Vec<u8> {
    let mut page = vec![0_u8; IO_PAGE_SIZE];
    page[8..12].copy_from_slice(&id.page_id.get().to_le_bytes());
    page[12..14].copy_from_slice(&id.vol_id.get().to_le_bytes());
    page[14] = page_type.ordinal();
    page
}

fn volume_page() -> Vec<u8> {
    let mut page = envelope_page(vpid(0), PageType::VolumeHeader);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
    user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
    user[40..44].copy_from_slice(&64_i32.to_le_bytes());
    user[44..48].copy_from_slice(&128_i32.to_le_bytes());
    user[48..52].copy_from_slice(&128_i32.to_le_bytes());
    user[52..56].copy_from_slice(&(-1_i32).to_le_bytes());
    user[56..60].copy_from_slice(&1_i32.to_le_bytes());
    user[60..64].copy_from_slice(&1_i32.to_le_bytes());
    user[64..68].copy_from_slice(&1_i32.to_le_bytes());
    user[96..100].copy_from_slice(&(-1_i32).to_le_bytes());
    user[100..102].copy_from_slice(&(-1_i16).to_le_bytes());
    user[104..108].copy_from_slice(&(-1_i32).to_le_bytes());
    user[124..126].copy_from_slice(&(-1_i16).to_le_bytes());
    user[128..130].copy_from_slice(&1_i16.to_le_bytes());
    user[130..132].copy_from_slice(&2_i16.to_le_bytes());
    user[132..135].copy_from_slice(b"\0\0\0");
    page
}

#[test]
fn bitmap_reads_little_endian_words_lsb_first_without_allocating_from_disk_counts() {
    let volume_bytes = volume_page();
    let volume_envelope = decode_page_envelope(&volume_bytes, vpid(0)).unwrap();
    let header = decode_volume_header(&volume_envelope, 128 * 64 * IO_PAGE_SIZE as u64).unwrap();

    let mut bitmap_bytes = envelope_page(vpid(1), PageType::VolumeBitmap);
    bitmap_bytes[32..40].copy_from_slice(&(1_u64 | (1_u64 << 63)).to_le_bytes());
    bitmap_bytes[40..48].copy_from_slice(&1_u64.to_le_bytes());
    let bitmap_envelope = decode_page_envelope(&bitmap_bytes, vpid(1)).unwrap();
    let bitmap = decode_sector_bitmap(&bitmap_envelope, &header, 0).unwrap();

    assert_eq!(bitmap.first_sector(), SectorId::new(0).unwrap());
    assert_eq!(bitmap.sector_count(), 128);
    assert!(bitmap.is_reserved(SectorId::new(0).unwrap()).unwrap());
    assert!(bitmap.is_reserved(SectorId::new(63).unwrap()).unwrap());
    assert!(bitmap.is_reserved(SectorId::new(64).unwrap()).unwrap());
    assert!(!bitmap.is_reserved(SectorId::new(65).unwrap()).unwrap());
}

#[test]
fn bitmap_rejects_wrong_pages_and_out_of_coverage_queries() {
    let volume_bytes = volume_page();
    let volume_envelope = decode_page_envelope(&volume_bytes, vpid(0)).unwrap();
    let header = decode_volume_header(&volume_envelope, 128 * 64 * IO_PAGE_SIZE as u64).unwrap();

    let wrong_bytes = envelope_page(vpid(1), PageType::Heap);
    let wrong_envelope = decode_page_envelope(&wrong_bytes, vpid(1)).unwrap();
    assert_eq!(
        decode_sector_bitmap(&wrong_envelope, &header, 0)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::WrongPageType
    );

    let bitmap_bytes = envelope_page(vpid(1), PageType::VolumeBitmap);
    let bitmap_envelope = decode_page_envelope(&bitmap_bytes, vpid(1)).unwrap();
    let bitmap = decode_sector_bitmap(&bitmap_envelope, &header, 0).unwrap();
    assert_eq!(
        bitmap
            .is_reserved(SectorId::new(128).unwrap())
            .unwrap_err()
            .kind(),
        DecodeErrorKind::OutOfRange
    );
}
