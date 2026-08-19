use volmap::format::{
    DB_PAGE_SIZE, IO_PAGE_SIZE, PageType, decode_bigone_target, decode_overflow_continuation,
    decode_overflow_head, decode_page_envelope, decode_slotted_page,
};
use volmap::model::{PageId, VolId, Vpid};

fn page(page_id: i32, page_type: PageType) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    page[8..12].copy_from_slice(&page_id.to_le_bytes());
    page[12..14].copy_from_slice(&0_i16.to_le_bytes());
    page[14] = page_type.ordinal();
    page
}

fn envelope(bytes: &[u8], page_id: i32) -> volmap::format::DecodedPageEnvelope<'_> {
    decode_page_envelope(
        bytes,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(page_id).unwrap()),
    )
    .unwrap()
}

fn put_vpid(user: &mut [u8], page_id: i32, volume_id: i16) {
    user[0..4].copy_from_slice(&page_id.to_le_bytes());
    user[4..6].copy_from_slice(&volume_id.to_le_bytes());
}

#[test]
fn overflow_pages_decode_exact_payload_extents_without_retaining_bytes() {
    let mut head = page(20, PageType::Overflow);
    let user = &mut head[32..IO_PAGE_SIZE - 8];
    put_vpid(user, 21, 0);
    user[8..12].copy_from_slice(&40_000_i32.to_le_bytes());
    let first = decode_overflow_head(&envelope(&head, 20)).unwrap();
    assert_eq!(first.total_length(), Some(40_000));
    assert_eq!(first.payload_offset(), 12);
    assert_eq!(usize::from(first.payload_length()), DB_PAGE_SIZE - 12);
    assert_eq!(first.next().unwrap().page_id.get(), 21);

    let mut middle = page(21, PageType::Overflow);
    put_vpid(&mut middle[32..IO_PAGE_SIZE - 8], 22, 0);
    let second = decode_overflow_continuation(&envelope(&middle, 21), 23_668).unwrap();
    assert_eq!(second.payload_offset(), 8);
    assert_eq!(usize::from(second.payload_length()), DB_PAGE_SIZE - 8);
    assert_eq!(second.next().unwrap().page_id.get(), 22);

    let mut tail = page(22, PageType::Overflow);
    put_vpid(&mut tail[32..IO_PAGE_SIZE - 8], -1, -1);
    let last = decode_overflow_continuation(&envelope(&tail, 22), 7_332).unwrap();
    assert_eq!(last.payload_length(), 7_332);
    assert_eq!(last.next(), None);
}

#[test]
fn overflow_pages_reject_invalid_lengths_and_chain_shapes() {
    let mut head = page(20, PageType::Overflow);
    put_vpid(&mut head[32..IO_PAGE_SIZE - 8], -1, -1);
    head[32 + 8..32 + 12].copy_from_slice(&40_000_i32.to_le_bytes());
    assert_eq!(
        decode_overflow_head(&envelope(&head, 20))
            .unwrap_err()
            .rule(),
        "overflow.page.link_shape"
    );

    head[32 + 8..32 + 12].copy_from_slice(&(-1_i32).to_le_bytes());
    assert_eq!(
        decode_overflow_head(&envelope(&head, 20))
            .unwrap_err()
            .rule(),
        "overflow.head.length"
    );

    let mut tail = page(21, PageType::Overflow);
    put_vpid(&mut tail[32..IO_PAGE_SIZE - 8], 22, 0);
    assert_eq!(
        decode_overflow_continuation(&envelope(&tail, 21), 7_332)
            .unwrap_err()
            .rule(),
        "overflow.page.link_shape"
    );
}

#[test]
fn bigone_slot_decodes_only_an_exact_null_slot_overflow_target() {
    let mut heap = page(10, PageType::Heap);
    let user = &mut heap[32..IO_PAGE_SIZE - 8];
    user[0..2].copy_from_slice(&1_i16.to_le_bytes());
    user[2..4].copy_from_slice(&1_i16.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    user[8..12].copy_from_slice(&16_304_i32.to_le_bytes());
    user[12..16].copy_from_slice(&16_304_i32.to_le_bytes());
    user[16..20].copy_from_slice(&40_i32.to_le_bytes());
    user[32..36].copy_from_slice(&20_i32.to_le_bytes());
    user[36..38].copy_from_slice(&(-1_i16).to_le_bytes());
    user[38..40].copy_from_slice(&0_i16.to_le_bytes());
    let slot = 32_u32 | (8_u32 << 14) | (5_u32 << 28);
    user[DB_PAGE_SIZE - 4..DB_PAGE_SIZE].copy_from_slice(&slot.to_le_bytes());
    let decoded = envelope(&heap, 10);
    let slotted = decode_slotted_page(&decoded).unwrap();
    assert_eq!(
        decode_bigone_target(&decoded, &slotted, 0)
            .unwrap()
            .page_id
            .get(),
        20
    );

    heap[32 + 36..32 + 38].copy_from_slice(&0_i16.to_le_bytes());
    let decoded = envelope(&heap, 10);
    let slotted = decode_slotted_page(&decoded).unwrap();
    assert_eq!(
        decode_bigone_target(&decoded, &slotted, 0)
            .unwrap_err()
            .rule(),
        "heap.bigone.null_slot"
    );
}
