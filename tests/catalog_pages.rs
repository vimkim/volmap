use volmap::format::{
    CatalogPageFact, DB_PAGE_SIZE, IO_PAGE_SIZE, PageType, decode_catalog_class_info,
    decode_catalog_directory, decode_catalog_page, decode_catalog_representation_header,
    decode_page_envelope, decode_slotted_page,
};
use volmap::model::{PageId, VolId, Vpid};

fn catalog_page(page_id: i32, records: &[(u16, u16, u8)]) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    page[8..12].copy_from_slice(&page_id.to_le_bytes());
    page[12..14].copy_from_slice(&0_i16.to_le_bytes());
    page[14] = PageType::Catalog.ordinal();
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    let slots = i16::try_from(records.len()).unwrap();
    user[0..2].copy_from_slice(&slots.to_le_bytes());
    user[2..4].copy_from_slice(&slots.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    let free_offset = records
        .iter()
        .map(|(offset, length, _)| i32::from(*offset) + i32::from(*length))
        .max()
        .unwrap();
    let free = i32::try_from(DB_PAGE_SIZE).unwrap() - i32::from(slots) * 4 - free_offset;
    user[8..12].copy_from_slice(&free.to_le_bytes());
    user[12..16].copy_from_slice(&free.to_le_bytes());
    user[16..20].copy_from_slice(&free_offset.to_le_bytes());
    for (slot_id, (offset, length, record_type)) in records.iter().enumerate() {
        let word =
            u32::from(*offset) | (u32::from(*length) << 14) | (u32::from(*record_type) << 28);
        let position = DB_PAGE_SIZE - (slot_id + 1) * 4;
        user[position..position + 4].copy_from_slice(&word.to_le_bytes());
    }
    page
}

fn decode(bytes: &[u8], page_id: i32) -> CatalogPageFact {
    let envelope = decode_page_envelope(
        bytes,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(page_id).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    decode_catalog_page(&envelope, &slotted).unwrap()
}

fn put_header(
    page: &mut [u8; IO_PAGE_SIZE],
    next_page: i32,
    next_volume: i16,
    directories: i32,
    is_overflow: i32,
) {
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[32..36].copy_from_slice(&next_page.to_be_bytes());
    user[36..38].copy_from_slice(&next_volume.to_be_bytes());
    user[40..44].copy_from_slice(&directories.to_be_bytes());
    user[44..48].copy_from_slice(&is_overflow.to_be_bytes());
}

#[test]
fn catalog_primary_and_overflow_pages_expose_only_structure() {
    let mut primary = catalog_page(20, &[(32, 16, 2), (48, 24, 2), (72, 40, 2)]);
    put_header(&mut primary, 21, 0, 2, 0);
    assert_eq!(
        decode(&primary, 20),
        CatalogPageFact {
            next_overflow: Some(Vpid::new(VolId::new(0).unwrap(), PageId::new(21).unwrap())),
            directory_count: 2,
            is_overflow: false,
            record_count: 2,
            record_bytes: 64,
        }
    );

    let mut overflow = catalog_page(21, &[(32, 16, 2), (48, 80, 2)]);
    put_header(&mut overflow, -1, -1, 0, 1);
    assert_eq!(
        decode(&overflow, 21),
        CatalogPageFact {
            next_overflow: None,
            directory_count: 0,
            is_overflow: true,
            record_count: 1,
            record_bytes: 80,
        }
    );
}

#[test]
fn catalog_decoder_rejects_forged_headers_counts_and_record_types() {
    let mut invalid_flag = catalog_page(20, &[(32, 16, 2)]);
    put_header(&mut invalid_flag, -1, -1, 0, 2);
    assert_rule(&invalid_flag, 20, "catalog.page.overflow_flag");

    let mut invalid_count = catalog_page(21, &[(32, 16, 2), (48, 24, 2)]);
    put_header(&mut invalid_count, -1, -1, 2, 0);
    assert_rule(&invalid_count, 21, "catalog.page.directory_count");

    let mut invalid_role = catalog_page(22, &[(32, 16, 2), (48, 24, 5)]);
    put_header(&mut invalid_role, -1, -1, 0, 0);
    assert_rule(&invalid_role, 22, "catalog.page.record_type");

    let short_header = catalog_page(23, &[(32, 12, 2)]);
    assert_rule(&short_header, 23, "catalog.page.header_slot");
}

fn assert_rule(bytes: &[u8], page_id: i32, expected: &str) {
    let envelope = decode_page_envelope(
        bytes,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(page_id).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_catalog_page(&envelope, &slotted).unwrap_err().rule(),
        expected
    );
}

#[test]
fn caller_proven_catalog_records_decode_directory_and_representation_prefixes() {
    let mut page = catalog_page(30, &[(32, 16, 2), (48, 32, 2), (80, 56, 2)]);
    put_header(&mut page, -1, -1, 1, 0);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[48..52].copy_from_slice(&31_i32.to_be_bytes());
    user[52..54].copy_from_slice(&0_i16.to_be_bytes());
    user[56..58].copy_from_slice(&7_i16.to_be_bytes());
    user[58..60].copy_from_slice(&2_i16.to_be_bytes());
    user[60] = 1;
    user[80..84].copy_from_slice(&7_i32.to_be_bytes());
    user[84..88].copy_from_slice(&3_i32.to_be_bytes());
    user[88..92].copy_from_slice(&24_i32.to_be_bytes());
    user[92..96].copy_from_slice(&2_i32.to_be_bytes());

    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(30).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    let directory = decode_catalog_directory(&envelope, &slotted, 1).unwrap();
    assert_eq!(directory.items.len(), 1);
    assert_eq!(directory.items[0].target.page_id.get(), 31);
    assert_eq!(directory.items[0].representation_id, 7);
    let representation = decode_catalog_representation_header(&envelope, &slotted, 2).unwrap();
    assert_eq!(representation.representation_id, 7);
    assert_eq!(representation.fixed_count, 3);
    assert_eq!(representation.fixed_length, 24);
    assert_eq!(representation.variable_count, 2);
}

#[test]
fn catalog_record_decoders_reject_unproven_shapes_and_hostile_counts() {
    let mut page = catalog_page(30, &[(32, 16, 2), (48, 32, 2), (80, 56, 2)]);
    put_header(&mut page, -1, -1, 1, 0);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[60] = 3;
    user[80..84].copy_from_slice(&7_i32.to_be_bytes());
    user[84..88].copy_from_slice(&(-1_i32).to_be_bytes());
    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(30).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_catalog_directory(&envelope, &slotted, 1)
            .unwrap_err()
            .rule(),
        "catalog.directory.count"
    );
    assert_eq!(
        decode_catalog_representation_header(&envelope, &slotted, 2)
            .unwrap_err()
            .rule(),
        "catalog.representation.fixed_count"
    );
}

#[test]
fn caller_proven_class_info_links_heap_and_representation_directory() {
    let mut page = catalog_page(30, &[(32, 16, 2), (48, 56, 2)]);
    put_header(&mut page, -1, -1, 0, 0);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[48..52].copy_from_slice(&70_i32.to_be_bytes());
    user[52..56].copy_from_slice(&64_i32.to_be_bytes());
    user[56..60].copy_from_slice(&0_i32.to_be_bytes());
    user[60..64].copy_from_slice(&12_i32.to_be_bytes());
    user[64..68].copy_from_slice(&34_i32.to_be_bytes());
    user[72..76].copy_from_slice(&30_i32.to_be_bytes());
    user[76..78].copy_from_slice(&4_i16.to_be_bytes());
    user[78..80].copy_from_slice(&0_i16.to_be_bytes());
    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(30).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    let info = decode_catalog_class_info(&envelope, &slotted, 1).unwrap();
    assert_eq!(info.heap_file.unwrap().file_id.get(), 64);
    assert_eq!(info.heap_header.unwrap().page_id.get(), 70);
    assert_eq!(info.total_pages, 12);
    assert_eq!(info.total_objects, 34);
    assert_eq!(info.representation_directory.slot_id.get(), 4);

    page[32 + 48..32 + 52].copy_from_slice(&(-1_i32).to_be_bytes());
    page[32 + 52..32 + 56].copy_from_slice(&(-1_i32).to_be_bytes());
    page[32 + 56..32 + 60].copy_from_slice(&0_i32.to_be_bytes());
    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(30).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    let info = decode_catalog_class_info(&envelope, &slotted, 1).unwrap();
    assert_eq!(info.heap_file, None);
    assert_eq!(info.heap_header, None);

    page[32 + 48..32 + 52].copy_from_slice(&70_i32.to_be_bytes());
    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(30).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_catalog_class_info(&envelope, &slotted, 1)
            .unwrap_err()
            .rule(),
        "catalog.class_info.heap"
    );
}
