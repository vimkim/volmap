use volmap::format::{
    DB_PAGE_SIZE, HeapPageFact, IO_PAGE_SIZE, PageType, decode_heap_page, decode_page_envelope,
    decode_slotted_page,
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
