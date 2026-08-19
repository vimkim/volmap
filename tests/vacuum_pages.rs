use volmap::format::{
    IO_PAGE_SIZE, PageType, decode_dropped_files_page, decode_page_envelope, decode_vacuum_page,
};
use volmap::model::{PageId, VolId, Vpid};

fn page(page_type: PageType) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    page[8..12].copy_from_slice(&7_i32.to_le_bytes());
    page[12..14].copy_from_slice(&1_i16.to_le_bytes());
    page[14] = page_type.ordinal();
    page
}

fn envelope(bytes: &[u8]) -> volmap::format::DecodedPageEnvelope<'_> {
    decode_page_envelope(
        bytes,
        Vpid::new(VolId::new(1).unwrap(), PageId::new(7).unwrap()),
    )
    .unwrap()
}

#[test]
fn vacuum_page_decodes_queue_indices_and_masked_block_metadata() {
    let mut bytes = page(PageType::VacuumData);
    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    user[0..4].copy_from_slice(&(-1_i32).to_le_bytes());
    user[4..6].copy_from_slice(&(-1_i16).to_le_bytes());
    user[8..10].copy_from_slice(&0_i16.to_le_bytes());
    user[10..12].copy_from_slice(&1_i16.to_le_bytes());
    user[16..24].copy_from_slice(&0x6000_0000_0000_0005_u64.to_le_bytes());
    user[24..32].copy_from_slice(&17_u64.to_le_bytes());
    user[32..40].copy_from_slice(&18_u64.to_le_bytes());
    user[40..48].copy_from_slice(&19_u64.to_le_bytes());

    let fact = decode_vacuum_page(&envelope(&bytes)).unwrap();
    assert_eq!(fact.index_unvacuumed, Some(0));
    assert_eq!(fact.entries[0].block_id, 5);
    assert_eq!(fact.entries[0].flags, 0x6000_0000_0000_0000);
    assert_eq!(fact.entries[0].newest_mvccid, 19);
}

#[test]
fn dropped_page_decodes_structural_file_lifecycle_entries() {
    let mut bytes = page(PageType::DroppedFiles);
    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    user[0..4].copy_from_slice(&8_i32.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[8..10].copy_from_slice(&1_i16.to_le_bytes());
    user[16..20].copy_from_slice(&64_i32.to_le_bytes());
    user[20..22].copy_from_slice(&1_i16.to_le_bytes());
    user[24..32].copy_from_slice(&123_u64.to_le_bytes());

    let fact = decode_dropped_files_page(&envelope(&bytes)).unwrap();
    assert_eq!(fact.next.unwrap().page_id.get(), 8);
    assert_eq!(fact.entries[0].vfid.file_id.get(), 64);
    assert_eq!(fact.entries[0].mvccid, 123);
}

#[test]
fn raw_page_decoders_reject_hostile_counts_and_reserved_status() {
    let mut vacuum = page(PageType::VacuumData);
    let user = &mut vacuum[32..IO_PAGE_SIZE - 8];
    user[8..10].copy_from_slice(&0_i16.to_le_bytes());
    user[10..12].copy_from_slice(&1_i16.to_le_bytes());
    user[16..24].copy_from_slice(&0xc000_0000_0000_0001_u64.to_le_bytes());
    assert_eq!(
        decode_vacuum_page(&envelope(&vacuum)).unwrap_err().rule(),
        "vacuum.entry.status"
    );

    let mut dropped = page(PageType::DroppedFiles);
    dropped[32 + 8..32 + 10].copy_from_slice(&i16::MAX.to_le_bytes());
    assert_eq!(
        decode_dropped_files_page(&envelope(&dropped))
            .unwrap_err()
            .rule(),
        "dropped.page.count"
    );
}
