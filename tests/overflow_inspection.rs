use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use volmap::diagnostics::InspectionOutcome;
use volmap::format::{DB_PAGE_SIZE, IO_PAGE_SIZE, PageType};
use volmap::inspection::{CancelToken, Inspection, OpenRequest, ResourcePolicy, RevisionSelector};
use volmap::model::{Oid, PageId, SlotId, VolId};
use volmap::source::InputSpec;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "volmap-overflow-test-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn page(page_id: i32, page_type: PageType) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    let lsa = u64::try_from(page_id).unwrap().to_le_bytes();
    page[0..8].copy_from_slice(&lsa);
    page[8..12].copy_from_slice(&page_id.to_le_bytes());
    page[12..14].copy_from_slice(&0_i16.to_le_bytes());
    page[14] = page_type.ordinal();
    page[IO_PAGE_SIZE - 8..].copy_from_slice(&lsa);
    page
}

fn volume_header_page() -> [u8; IO_PAGE_SIZE] {
    let mut page = page(0, PageType::VolumeHeader);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
    user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
    user[40..44].copy_from_slice(&64_i32.to_le_bytes());
    user[44..48].copy_from_slice(&64_i32.to_le_bytes());
    user[48..52].copy_from_slice(&64_i32.to_le_bytes());
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
    page
}

fn put_null_vpid(bytes: &mut [u8], offset: usize) {
    bytes[offset..offset + 4].copy_from_slice(&(-1_i32).to_le_bytes());
    bytes[offset + 4..offset + 6].copy_from_slice(&(-1_i16).to_le_bytes());
}

fn put_vpid(bytes: &mut [u8], offset: usize, page_id: i32) {
    bytes[offset..offset + 4].copy_from_slice(&page_id.to_le_bytes());
    bytes[offset + 4..offset + 6].copy_from_slice(&0_i16.to_le_bytes());
}

fn file_header_page(
    file_id: i32,
    file_type: i32,
    page_user: i32,
    allocation_bitmap: u64,
    sticky_first: Option<i32>,
) -> [u8; IO_PAGE_SIZE] {
    let mut page = page(file_id, PageType::FileTable);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[8..12].copy_from_slice(&file_id.to_le_bytes());
    user[12..14].copy_from_slice(&0_i16.to_le_bytes());
    user[104..108].copy_from_slice(&(page_user + 1).to_le_bytes());
    user[108..112].copy_from_slice(&page_user.to_le_bytes());
    user[112..116].copy_from_slice(&1_i32.to_le_bytes());
    user[124..128].copy_from_slice(&1_i32.to_le_bytes());
    user[128..132].copy_from_slice(&1_i32.to_le_bytes());
    user[140..144].copy_from_slice(&file_type.to_le_bytes());
    user[150..152].copy_from_slice(&216_i16.to_le_bytes());
    user[152..154].copy_from_slice(&248_i16.to_le_bytes());
    user[154..156].copy_from_slice(&(-1_i16).to_le_bytes());
    match sticky_first {
        Some(page_id) => put_vpid(user, 156, page_id),
        None => put_null_vpid(user, 156),
    }
    put_null_vpid(user, 216);
    user[224..226].copy_from_slice(&16_i16.to_le_bytes());
    user[226..228].copy_from_slice(&16_i16.to_le_bytes());
    user[228..230].copy_from_slice(&1_i16.to_le_bytes());
    user[232..236].copy_from_slice(&0_i32.to_le_bytes());
    user[236..238].copy_from_slice(&0_i16.to_le_bytes());
    user[240..248].copy_from_slice(&allocation_bitmap.to_le_bytes());
    put_null_vpid(user, 248);
    user[256..258].copy_from_slice(&8_i16.to_le_bytes());
    user[258..260].copy_from_slice(&8_i16.to_le_bytes());
    page
}

fn tracker_items_page() -> [u8; IO_PAGE_SIZE] {
    let mut page = page(3, PageType::FileTable);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    put_null_vpid(user, 0);
    user[8..10].copy_from_slice(&32_i16.to_le_bytes());
    user[10..12].copy_from_slice(&16_i16.to_le_bytes());
    user[12..14].copy_from_slice(&2_i16.to_le_bytes());
    user[16..20].copy_from_slice(&4_i32.to_le_bytes());
    user[20..22].copy_from_slice(&0_i16.to_le_bytes());
    user[22..24].copy_from_slice(&1_i16.to_le_bytes());
    user[32..36].copy_from_slice(&7_i32.to_le_bytes());
    user[36..38].copy_from_slice(&0_i16.to_le_bytes());
    user[38..40].copy_from_slice(&3_i16.to_le_bytes());
    page
}

fn slotted_header(user: &mut [u8], slots: i16, records: i16, free_offset: i32) {
    user[0..2].copy_from_slice(&slots.to_le_bytes());
    user[2..4].copy_from_slice(&records.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    let slot_bytes = i32::from(slots) * 4;
    let free = i32::try_from(DB_PAGE_SIZE).unwrap() - slot_bytes - free_offset;
    user[8..12].copy_from_slice(&free.to_le_bytes());
    user[12..16].copy_from_slice(&free.to_le_bytes());
    user[16..20].copy_from_slice(&free_offset.to_le_bytes());
}

fn heap_header_page() -> [u8; IO_PAGE_SIZE] {
    let mut page = page(5, PageType::Heap);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    slotted_header(user, 1, 1, 1_192);
    user[32..40].fill(0xff);
    user[40..44].copy_from_slice(&7_i32.to_le_bytes());
    user[44..46].copy_from_slice(&0_i16.to_le_bytes());
    put_vpid(user, 48, 6);
    put_vpid(user, 56, 6);
    put_null_vpid(user, 64);
    user[72..76].copy_from_slice(&64_i32.to_le_bytes());
    user[76..80].copy_from_slice(&1_i32.to_le_bytes());
    let slot = 32_u32 | (1_160_u32 << 14) | (2_u32 << 28);
    user[DB_PAGE_SIZE - 4..DB_PAGE_SIZE].copy_from_slice(&slot.to_le_bytes());
    page
}

fn heap_bigone_page(relocation_target_type: u8) -> [u8; IO_PAGE_SIZE] {
    let mut page = page(6, PageType::Heap);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    slotted_header(user, 4, 4, 96);
    user[32..40].fill(0xff);
    put_vpid(user, 40, 5);
    put_null_vpid(user, 48);
    user[64..68].copy_from_slice(&1_u32.to_le_bytes());
    user[72..76].copy_from_slice(&8_i32.to_le_bytes());
    user[76..78].copy_from_slice(&(-1_i16).to_le_bytes());
    user[78..80].copy_from_slice(&0_i16.to_le_bytes());
    user[80..84].copy_from_slice(&6_i32.to_le_bytes());
    user[84..86].copy_from_slice(&3_i16.to_le_bytes());
    user[86..88].copy_from_slice(&0_i16.to_le_bytes());
    let chain = 32_u32 | (40_u32 << 14) | (2_u32 << 28);
    let bigone = 0x48_u32 | (8_u32 << 14) | (5_u32 << 28);
    let relocation = 0x50_u32 | (8_u32 << 14) | (4_u32 << 28);
    let newhome = 0x58_u32 | (8_u32 << 14) | (u32::from(relocation_target_type) << 28);
    user[DB_PAGE_SIZE - 4..DB_PAGE_SIZE].copy_from_slice(&chain.to_le_bytes());
    user[DB_PAGE_SIZE - 8..DB_PAGE_SIZE - 4].copy_from_slice(&bigone.to_le_bytes());
    user[DB_PAGE_SIZE - 12..DB_PAGE_SIZE - 8].copy_from_slice(&relocation.to_le_bytes());
    user[DB_PAGE_SIZE - 16..DB_PAGE_SIZE - 12].copy_from_slice(&newhome.to_le_bytes());
    page
}

fn overflow_page(page_id: i32, next: Option<i32>, total: Option<i32>) -> [u8; IO_PAGE_SIZE] {
    let mut page = page(page_id, PageType::Overflow);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    match next {
        Some(next) => put_vpid(user, 0, next),
        None => put_null_vpid(user, 0),
    }
    if let Some(total) = total {
        user[8..12].copy_from_slice(&total.to_le_bytes());
    }
    user[12..20].copy_from_slice(b"withheld");
    page
}

fn fixture(corrupt_tail: bool, relocation_target_type: u8) -> (TestDirectory, PathBuf) {
    let directory = TestDirectory::new();
    let volume = directory.path().join("fixture");
    let vinf = directory.path().join("fixture_vinf");
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&volume)
        .unwrap();
    file.set_len(64 * 64 * IO_PAGE_SIZE as u64).unwrap();
    let mut bitmap = page(1, PageType::VolumeBitmap);
    bitmap[32..40].copy_from_slice(&1_u64.to_le_bytes());
    let mut tracker = file_header_page(2, 0, 1, (1_u64 << 2) | (1_u64 << 3), Some(3));
    let heap = file_header_page(4, 1, 2, (1_u64 << 4) | (1_u64 << 5) | (1_u64 << 6), None);
    tracker[32 + 40..32 + 52].fill(0);
    let mut overflow = file_header_page(
        7,
        3,
        3,
        (1_u64 << 7) | (1_u64 << 8) | (1_u64 << 9) | (1_u64 << 10),
        None,
    );
    overflow[32 + 40..32 + 44].copy_from_slice(&4_i32.to_le_bytes());
    overflow[32 + 44..32 + 46].copy_from_slice(&0_i16.to_le_bytes());
    overflow[32 + 48..32 + 52].copy_from_slice(&5_i32.to_le_bytes());
    let mut pages = vec![
        volume_header_page(),
        bitmap,
        tracker,
        tracker_items_page(),
        heap,
        heap_header_page(),
        heap_bigone_page(relocation_target_type),
        overflow,
        overflow_page(8, Some(9), Some(40_000)),
        overflow_page(9, Some(10), None),
        overflow_page(10, corrupt_tail.then_some(8), None),
    ];
    let heap_descriptor = &mut pages[4][32..IO_PAGE_SIZE - 8];
    heap_descriptor[48..52].copy_from_slice(&4_i32.to_le_bytes());
    heap_descriptor[52..54].copy_from_slice(&0_i16.to_le_bytes());
    heap_descriptor[56..60].copy_from_slice(&5_i32.to_le_bytes());
    for (page_id, page_bytes) in pages.into_iter().enumerate() {
        file.write_all_at(
            &page_bytes,
            u64::try_from(page_id).unwrap() * IO_PAGE_SIZE as u64,
        )
        .unwrap();
    }
    drop(file);
    let mut manifest = File::create(&vinf).unwrap();
    writeln!(manifest, "0 {}", volume.display()).unwrap();
    (directory, vinf)
}

fn policy(max_steps: u64) -> ResourcePolicy {
    ResourcePolicy::new(4 * 1024 * 1024, 1024 * 1024, 1, max_steps, 1024 * 1024).unwrap()
}

fn open(corrupt_tail: bool) -> (TestDirectory, volmap::inspection::GraphView) {
    open_with_relocation(corrupt_tail, 3)
}

fn open_with_relocation(
    corrupt_tail: bool,
    relocation_target_type: u8,
) -> (TestDirectory, volmap::inspection::GraphView) {
    let (directory, vinf) = fixture(corrupt_tail, relocation_target_type);
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let view = Inspection::open(&request, policy(32), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
    assert_eq!(view.overview().revision.get(), 0);
    assert_eq!(
        view.overview()
            .coverage
            .iter()
            .find(|coverage| coverage.facet == "file-inventory")
            .unwrap()
            .coverage,
        volmap::model::Coverage::Complete
    );
    let idempotent = view
        .enrich_file_inventory(policy(32), &CancelToken::new())
        .unwrap();
    assert_eq!(idempotent.overview().revision.get(), 0);
    (directory, view)
}

fn source() -> Oid {
    Oid::new(
        VolId::new(0).unwrap(),
        PageId::new(6).unwrap(),
        SlotId::new(1).unwrap(),
    )
}

fn relocation_source() -> Oid {
    Oid::new(
        VolId::new(0).unwrap(),
        PageId::new(6).unwrap(),
        SlotId::new(2).unwrap(),
    )
}

#[test]
fn bigone_enrichment_validates_typed_ownership_and_payload_extents() {
    let (_directory, view) = open(false);
    let enriched = view
        .enrich_bigone(source(), policy(32), &CancelToken::new())
        .unwrap();
    let chain = enriched.overflow_chain(source()).unwrap();
    assert!(chain.complete);
    assert_eq!(chain.head.unwrap().page_id.get(), 8);
    assert_eq!(chain.total_data_length, Some(40_000));
    assert_eq!(chain.validated_payload_bytes, 40_000);
    assert_eq!(chain.pages.len(), 3);
    assert!(!format!("{chain:?}").contains("withheld"));
}

#[test]
fn bigone_enrichment_publishes_corrupt_and_resource_limited_prefixes() {
    let (_directory, corrupt) = open(true);
    let corrupt = corrupt
        .enrich_bigone(source(), policy(32), &CancelToken::new())
        .unwrap();
    let chain = corrupt.overflow_chain(source()).unwrap();
    assert!(!chain.complete);
    assert_eq!(chain.pages.len(), 2);
    assert_eq!(chain.diagnostic_rule, Some("overflow.page.link_shape"));
    assert_eq!(corrupt.overview().outcome, InspectionOutcome::Findings);

    let (_directory, view) = open(false);
    let limited = view
        .enrich_bigone(source(), policy(1), &CancelToken::new())
        .unwrap();
    let chain = limited.overflow_chain(source()).unwrap();
    assert!(!chain.complete);
    assert_eq!(chain.pages.len(), 1);
    assert_eq!(chain.validated_payload_bytes, 16_332);
    assert_eq!(chain.diagnostic_rule, Some("resource-limit"));
    assert_eq!(limited.overview().outcome, InspectionOutcome::Incomplete);
}

#[test]
fn relocation_enrichment_publishes_only_a_validated_same_heap_edge() {
    let (_directory, view) = open(false);
    let enriched = view
        .enrich_relocation(relocation_source(), policy(32), &CancelToken::new())
        .unwrap();
    let edge = enriched.relocation_edge(relocation_source()).unwrap();
    assert!(edge.valid);
    assert_eq!(edge.target.unwrap().page_id.get(), 6);
    assert_eq!(edge.target.unwrap().slot_id.get(), 3);
    assert_eq!(edge.diagnostic_rule, None);
}

#[test]
fn relocation_enrichment_rejects_a_target_that_is_not_newhome() {
    let (_directory, view) = open_with_relocation(false, 2);
    let enriched = view
        .enrich_relocation(relocation_source(), policy(32), &CancelToken::new())
        .unwrap();
    let edge = enriched.relocation_edge(relocation_source()).unwrap();
    assert!(!edge.valid);
    assert_eq!(edge.target.unwrap().slot_id.get(), 3);
    assert_eq!(
        edge.diagnostic_rule,
        Some("heap.relocation.target_slot_role")
    );
    assert_eq!(enriched.overview().outcome, InspectionOutcome::Findings);
}
