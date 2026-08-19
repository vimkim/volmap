use volmap::format::{
    DB_PAGE_SIZE, IO_PAGE_SIZE, OosNext, PageType, RecordType, decode_oos_chunk,
    decode_page_envelope, decode_slotted_page,
};
use volmap::model::{PageId, VolId, Vpid};

fn page(page_type: PageType) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    page[8..12].copy_from_slice(&7_i32.to_le_bytes());
    page[12..14].copy_from_slice(&1_i16.to_le_bytes());
    page[14] = page_type.ordinal();
    page
}

fn put_header(page: &mut [u8; IO_PAGE_SIZE], slots: i16, records: i16) {
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[0..2].copy_from_slice(&slots.to_le_bytes());
    user[2..4].copy_from_slice(&records.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    user[8..12].copy_from_slice(&16_000_i32.to_le_bytes());
    user[12..16].copy_from_slice(&15_000_i32.to_le_bytes());
    user[16..20].copy_from_slice(&64_i32.to_le_bytes());
}

fn put_slot(page: &mut [u8; IO_PAGE_SIZE], slot: usize, offset: u16, length: u16, kind: u8) {
    let word = u32::from(offset) | (u32::from(length) << 14) | (u32::from(kind) << 28);
    let start = 32 + DB_PAGE_SIZE - 4 * (slot + 1);
    page[start..start + 4].copy_from_slice(&word.to_le_bytes());
}

fn envelope(bytes: &[u8]) -> volmap::format::DecodedPageEnvelope<'_> {
    decode_page_envelope(
        bytes,
        Vpid::new(VolId::new(1).unwrap(), PageId::new(7).unwrap()),
    )
    .unwrap()
}

#[test]
fn common_slot_geometry_decodes_without_exposing_record_bytes() {
    let mut bytes = page(PageType::Heap);
    put_header(&mut bytes, 2, 1);
    put_slot(&mut bytes, 0, 32, 24, 2);
    put_slot(&mut bytes, 1, 0, 0, 9);
    let decoded = decode_slotted_page(&envelope(&bytes)).unwrap();

    assert_eq!(decoded.alignment(), 8);
    assert_eq!(decoded.slots().len(), 2);
    assert_eq!(decoded.slots()[0].record_type(), RecordType::Home);
    assert_eq!(decoded.slots()[0].offset(), 32);
    assert_eq!(decoded.slots()[0].length(), 24);
    assert_eq!(decoded.slots()[1].record_type(), RecordType::Reserved(9));
    assert!(decoded.slots()[1].is_empty());
}

#[test]
fn overlapping_records_and_forged_counts_fail_closed() {
    let mut overlap = page(PageType::Catalog);
    put_header(&mut overlap, 2, 2);
    put_slot(&mut overlap, 0, 32, 32, 2);
    put_slot(&mut overlap, 1, 48, 32, 2);
    assert_eq!(
        decode_slotted_page(&envelope(&overlap)).unwrap_err().rule(),
        "slotted.slot.nonoverlap"
    );

    let mut count = page(PageType::Btree);
    put_header(&mut count, 1, 1);
    put_slot(&mut count, 0, 0, 0, 7);
    assert_eq!(
        decode_slotted_page(&envelope(&count)).unwrap_err().rule(),
        "slotted.header.record_count_match"
    );
}

#[test]
fn oos_chunk_reports_only_header_and_payload_extent() {
    let mut bytes = page(PageType::Oos);
    put_header(&mut bytes, 1, 1);
    put_slot(&mut bytes, 0, 32, 20, 2);
    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    user[32..36].copy_from_slice(&4_i32.to_le_bytes());
    user[36..40].copy_from_slice(&0_i32.to_le_bytes());
    user[40..44].copy_from_slice(&(-1_i32).to_le_bytes());
    user[44..46].copy_from_slice(&(-1_i16).to_le_bytes());
    user[46..48].copy_from_slice(&(-1_i16).to_le_bytes());
    user[48..52].copy_from_slice(b"hide");
    let envelope = envelope(&bytes);
    let slotted = decode_slotted_page(&envelope).unwrap();
    let chunk = decode_oos_chunk(&envelope, &slotted, 0).unwrap();

    assert_eq!(chunk.total_data_length(), 4);
    assert_eq!(chunk.chunk_index(), 0);
    assert_eq!(chunk.next(), OosNext::Terminal);
    assert_eq!(chunk.payload_offset(), 48);
    assert_eq!(chunk.payload_length(), 4);
    assert!(!format!("{chunk:?}").contains("hide"));
}

#[test]
fn oos_chunk_rejects_partial_null_links_and_empty_payloads() {
    let mut bytes = page(PageType::Oos);
    put_header(&mut bytes, 1, 1);
    put_slot(&mut bytes, 0, 32, 16, 2);
    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    user[32..36].copy_from_slice(&8_i32.to_le_bytes());
    user[36..40].copy_from_slice(&0_i32.to_le_bytes());
    user[40..44].copy_from_slice(&(-1_i32).to_le_bytes());
    user[44..46].copy_from_slice(&0_i16.to_le_bytes());
    user[46..48].copy_from_slice(&(-1_i16).to_le_bytes());
    let envelope = envelope(&bytes);
    let slotted = decode_slotted_page(&envelope).unwrap();

    assert_eq!(
        decode_oos_chunk(&envelope, &slotted, 0).unwrap_err().rule(),
        "oos.chunk.next_oid"
    );
}
