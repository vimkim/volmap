use volmap::format::{
    DB_PAGE_SIZE, HeapPageFact, IO_PAGE_SIZE, PageType, decode_heap_page,
    decode_heap_record_envelope, decode_page_envelope, decode_slotted_page,
};
use volmap::model::{PageId, VolId, Vpid};

fn heap_page(page_id: i32, record_length: u16) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    page[8..12].copy_from_slice(&page_id.to_le_bytes());
    page[12..14].copy_from_slice(&0_i16.to_le_bytes());
    page[14] = PageType::Heap.ordinal();
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[0..2].copy_from_slice(&1_i16.to_le_bytes());
    user[2..4].copy_from_slice(&1_i16.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    let free = 32_i32 + i32::from(record_length);
    let free_bytes = 16_308_i32 - i32::from(record_length);
    user[8..12].copy_from_slice(&free_bytes.to_le_bytes());
    user[12..16].copy_from_slice(&free_bytes.to_le_bytes());
    user[16..20].copy_from_slice(&free.to_le_bytes());
    let slot = 32_u32 | (u32::from(record_length) << 14) | (2_u32 << 28);
    user[DB_PAGE_SIZE - 4..DB_PAGE_SIZE].copy_from_slice(&slot.to_le_bytes());
    page
}

fn decode(bytes: &[u8], page_id: i32, is_header: bool) -> HeapPageFact {
    let envelope = decode_page_envelope(
        bytes,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(page_id).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    decode_heap_page(&envelope, &slotted, is_header).unwrap()
}

fn put_oid(user: &mut [u8], offset: usize, page: i32, slot: i16, volume: i16) {
    user[offset..offset + 4].copy_from_slice(&page.to_le_bytes());
    user[offset + 4..offset + 6].copy_from_slice(&slot.to_le_bytes());
    user[offset + 6..offset + 8].copy_from_slice(&volume.to_le_bytes());
}

fn put_null_oid(user: &mut [u8], offset: usize) {
    put_oid(user, offset, -1, -1, -1);
}

fn put_null_vpid(user: &mut [u8], offset: usize) {
    user[offset..offset + 4].copy_from_slice(&(-1_i32).to_le_bytes());
    user[offset + 4..offset + 6].copy_from_slice(&(-1_i16).to_le_bytes());
}

fn heap_record_page(record_length: u16, record_type: u8) -> [u8; IO_PAGE_SIZE] {
    let mut page = heap_page(130, 40);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[0..2].copy_from_slice(&2_i16.to_le_bytes());
    user[2..4].copy_from_slice(&2_i16.to_le_bytes());
    let free_area = 72_i32 + i32::from(record_length);
    let free_bytes = i32::try_from(DB_PAGE_SIZE - 8).unwrap() - free_area;
    user[8..12].copy_from_slice(&free_bytes.to_le_bytes());
    user[12..16].copy_from_slice(&free_bytes.to_le_bytes());
    user[16..20].copy_from_slice(&free_area.to_le_bytes());
    let slot = 0x48_u32 | (u32::from(record_length) << 14) | (u32::from(record_type) << 28);
    user[DB_PAGE_SIZE - 8..DB_PAGE_SIZE - 4].copy_from_slice(&slot.to_le_bytes());
    page
}

#[test]
fn role_gated_heap_header_and_chain_decode_structural_metadata() {
    let mut header = heap_page(129, 1_160);
    let user = &mut header[32..IO_PAGE_SIZE - 8];
    put_oid(user, 32, 7, 2, 0);
    put_null_vpid(user, 40);
    put_null_vpid(user, 48);
    user[56..60].copy_from_slice(&130_i32.to_le_bytes());
    user[60..62].copy_from_slice(&0_i16.to_le_bytes());
    put_null_vpid(user, 64);
    user[72..76].copy_from_slice(&64_i32.to_le_bytes());
    user[76..80].copy_from_slice(&2_i32.to_le_bytes());
    user[80..88].copy_from_slice(&3_u64.to_le_bytes());
    user[88..96].copy_from_slice(&400_u64.to_le_bytes());
    let HeapPageFact::Header(fact) = decode(&header, 129, true) else {
        panic!("expected header");
    };
    assert_eq!(fact.class_oid.unwrap().page_id.get(), 7);
    assert_eq!(fact.last.page_id.get(), 130);
    assert_eq!(fact.estimated_records, 3);

    put_null_oid(&mut header[32..IO_PAGE_SIZE - 8], 32);
    let HeapPageFact::Header(fact) = decode(&header, 129, true) else {
        panic!("expected header");
    };
    assert_eq!(fact.class_oid, None);

    let mut chain = heap_page(130, 40);
    let user = &mut chain[32..IO_PAGE_SIZE - 8];
    put_oid(user, 32, 7, 2, 0);
    user[40..44].copy_from_slice(&129_i32.to_le_bytes());
    user[44..46].copy_from_slice(&0_i16.to_le_bytes());
    put_null_vpid(user, 48);
    user[56..64].copy_from_slice(&99_u64.to_le_bytes());
    user[64..68].copy_from_slice(&0x8000_0001_u32.to_le_bytes());
    let HeapPageFact::Chain(fact) = decode(&chain, 130, false) else {
        panic!("expected chain");
    };
    assert_eq!(fact.previous.unwrap().page_id.get(), 129);
    assert_eq!(fact.max_mvccid, 99);
}

#[test]
fn heap_decoder_rejects_role_confusion_and_invalid_vacuum_flags() {
    let header = heap_page(129, 1_160);
    let envelope = decode_page_envelope(
        &header,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(129).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_heap_page(&envelope, &slotted, false)
            .unwrap_err()
            .rule(),
        "heap.page.role_length"
    );

    let mut chain = heap_page(130, 40);
    let user = &mut chain[32..IO_PAGE_SIZE - 8];
    put_oid(user, 32, 7, 2, 0);
    put_null_vpid(user, 40);
    put_null_vpid(user, 48);
    user[64..68].copy_from_slice(&0xc000_0000_u32.to_le_bytes());
    let envelope = decode_page_envelope(
        &chain,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(130).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_heap_page(&envelope, &slotted, false)
            .unwrap_err()
            .rule(),
        "heap.chain.flags"
    );
}

#[test]
fn heap_record_envelope_decodes_mvcc_structure_without_value_bytes() {
    let mut page = heap_record_page(40, 3);
    let record = &mut page[32 + 72..32 + 112];
    let first_word = 0x8000_0000_u32 | 0x4000_0000 | (u32::from(0x0f_u8) << 24) | 0x00ab_cdef;
    record[0..4].copy_from_slice(&first_word.to_be_bytes());
    record[4..8].copy_from_slice(&(-7_i32).to_be_bytes());
    record[8..16].copy_from_slice(&11_u64.to_be_bytes());
    record[16..24].copy_from_slice(&22_u64.to_be_bytes());
    record[24..32].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());

    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(130).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    let fact = decode_heap_record_envelope(&envelope, &slotted, 1, true).unwrap();
    assert_eq!(fact.record_type.as_str(), "new-home");
    assert_eq!(fact.representation_id, 0x00ab_cdef);
    assert_eq!(fact.chn, -7);
    assert_eq!(fact.record_flags, 0x0f);
    assert_eq!(fact.mvcc_flags, 0x07);
    assert!(fact.has_bound_bits);
    assert!(fact.has_oos);
    assert_eq!(fact.variable_offset_width, 2);
    assert_eq!(fact.header_length, 32);
    assert_eq!(fact.insert_mvccid, Some(11));
    assert_eq!(fact.delete_mvccid, Some(22));
    assert_eq!(fact.previous_version_lsa_word, Some(0x1122_3344_5566_7788));
    assert_eq!(fact.body_offset, 104);
    assert_eq!(fact.body_length, 8);
}

#[test]
fn heap_record_envelope_distinguishes_non_mvcc_and_rejects_ambiguous_shapes() {
    let mut page = heap_record_page(8, 2);
    let record = &mut page[32 + 72..32 + 80];
    let first_word = (u32::from(0x08_u8) << 24) | 7;
    record[0..4].copy_from_slice(&first_word.to_be_bytes());
    record[4..8].copy_from_slice(&9_i32.to_be_bytes());
    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(130).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    let fact = decode_heap_record_envelope(&envelope, &slotted, 1, false).unwrap();
    assert!(!fact.is_mvcc);
    assert_eq!(fact.variable_offset_width, 4);
    assert_eq!(fact.header_length, 8);
    assert_eq!(fact.body_length, 0);

    assert_eq!(
        decode_heap_record_envelope(&envelope, &slotted, 0, false)
            .unwrap_err()
            .rule(),
        "heap.record.data_slot"
    );

    page[32 + 72] = 0x01;
    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(130).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_heap_record_envelope(&envelope, &slotted, 1, false)
            .unwrap_err()
            .rule(),
        "heap.record.flags"
    );

    page[32 + 72] = 0x10;
    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(130).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_heap_record_envelope(&envelope, &slotted, 1, true)
            .unwrap_err()
            .rule(),
        "heap.record.flags"
    );
}

#[test]
fn heap_record_envelope_rejects_truncated_optional_header() {
    let mut page = heap_record_page(8, 2);
    page[32 + 72..32 + 76].copy_from_slice(&(u32::from(0x07_u8) << 24).to_be_bytes());
    let envelope = decode_page_envelope(
        &page,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(130).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_heap_record_envelope(&envelope, &slotted, 1, true)
            .unwrap_err()
            .rule(),
        "heap.record.header_length"
    );
}
