use volmap::format::{
    FormatProfile, HeapPageFact, decode_heap_page, decode_page_envelope_with_profile,
    decode_slotted_page,
};
use volmap::model::{PageId, VolId, Vpid};

#[test]
fn real_develop_heap_header_uses_its_persistent_layout() {
    let bytes = include_bytes!("../fixtures/8cb558b3/pages/vol0-page129.bin");
    let id = Vpid::new(VolId::new(0).unwrap(), PageId::new(129).unwrap());
    let page = decode_page_envelope_with_profile(bytes, id, FormatProfile::Develop).unwrap();
    let slots = decode_slotted_page(&page).unwrap();
    let HeapPageFact::Header(header) = decode_heap_page(&page, &slots, true).unwrap() else {
        panic!("expected the engine's boot heap header");
    };
    assert_eq!(header.oos_vfid, None);
    assert_eq!(header.unfill_space, 1_634);
    assert_eq!(header.estimated_pages, 1);
    assert_eq!(header.estimated_records, 1);
    assert_eq!(header.estimated_record_bytes, 136);
    assert_eq!(
        header.last,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(130).unwrap())
    );
}

fn corpus() -> serde_json::Value {
    serde_json::from_str(include_str!("../fixtures/8cb558b3/manifest.json")).unwrap()
}

fn bytes(volid: i16, pageid: i32) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "fixtures/8cb558b3/pages/vol{volid}-page{pageid}.bin"
        )),
    )
    .unwrap()
}

fn vpid(volid: i16, pageid: i32) -> Vpid {
    Vpid::new(VolId::new(volid).unwrap(), PageId::new(pageid).unwrap())
}

#[test]
fn pinned_real_develop_pages_match_native_identities_types_and_hashes() {
    use core::fmt::Write as _;
    use sha2::{Digest, Sha256};
    let manifest = corpus();
    let pages = manifest["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 19);
    assert_eq!(
        manifest["engine_base"],
        "8cb558b3b264b5ed045ab24fa50b0ef825925349"
    );
    for fixture in pages {
        let volid = i16::try_from(fixture["volid"].as_i64().unwrap()).unwrap();
        let pageid = i32::try_from(fixture["pageid"].as_i64().unwrap()).unwrap();
        let data = bytes(volid, pageid);
        let mut hash = String::with_capacity(64);
        for byte in Sha256::digest(&data) {
            write!(&mut hash, "{byte:02x}").unwrap();
        }
        assert_eq!(hash, fixture["sha256"]);
        let page =
            decode_page_envelope_with_profile(&data, vpid(volid, pageid), FormatProfile::Develop)
                .unwrap();
        assert_eq!(page.page_type().as_str(), fixture["kind"]);
        assert_ne!(page.page_type(), volmap::format::PageType::Oos);
    }
}

#[test]
fn real_develop_volume_geometry_bitmaps_and_file_headers_decode() {
    use volmap::format::{decode_file_header, decode_sector_bitmap, decode_volume_header};
    let manifest = corpus();
    for volume in manifest["volumes"].as_array().unwrap() {
        let volid = i16::try_from(volume["volid"].as_i64().unwrap()).unwrap();
        let header_bytes = bytes(volid, 0);
        let page = decode_page_envelope_with_profile(
            &header_bytes,
            vpid(volid, 0),
            FormatProfile::Develop,
        )
        .unwrap();
        let header = decode_volume_header(&page, volume["size"].as_u64().unwrap()).unwrap();
        assert_eq!(header.total_sectors(), if volid == 0 { 64 } else { 128 });
        let bitmap_bytes = bytes(volid, 1);
        let page = decode_page_envelope_with_profile(
            &bitmap_bytes,
            vpid(volid, 1),
            FormatProfile::Develop,
        )
        .unwrap();
        let bitmap = decode_sector_bitmap(&page, &header, 0).unwrap();
        assert!(
            bitmap
                .is_reserved(volmap::model::SectorId::new(0).unwrap())
                .unwrap()
        );
        let file_bytes = bytes(volid, 64);
        let page =
            decode_page_envelope_with_profile(&file_bytes, vpid(volid, 64), FormatProfile::Develop)
                .unwrap();
        let file = decode_file_header(&page).unwrap();
        assert_eq!(
            file.file_type().as_str(),
            if volid == 0 { "tracker" } else { "heap" }
        );
    }
}

#[test]
fn real_develop_heap_chains_and_shifted_structural_page_families_decode() {
    use volmap::format::{
        BtreePageFact, decode_btree_page, decode_catalog_page, decode_dropped_files_page,
        decode_vacuum_page,
    };
    for (volid, pageid) in [(0, 130), (1, 66)] {
        let data = bytes(volid, pageid);
        let page =
            decode_page_envelope_with_profile(&data, vpid(volid, pageid), FormatProfile::Develop)
                .unwrap();
        let slots = decode_slotted_page(&page).unwrap();
        let HeapPageFact::Chain(chain) = decode_heap_page(&page, &slots, false).unwrap() else {
            panic!("expected a native heap chain");
        };
        assert_eq!(chain.previous, Some(vpid(volid, pageid - 1)));
    }
    let data = bytes(0, 577);
    let page =
        decode_page_envelope_with_profile(&data, vpid(0, 577), FormatProfile::Develop).unwrap();
    decode_catalog_page(&page, &decode_slotted_page(&page).unwrap()).unwrap();
    let data = bytes(0, 641);
    let page =
        decode_page_envelope_with_profile(&data, vpid(0, 641), FormatProfile::Develop).unwrap();
    decode_vacuum_page(&page).unwrap();
    let data = bytes(0, 705);
    let page =
        decode_page_envelope_with_profile(&data, vpid(0, 705), FormatProfile::Develop).unwrap();
    decode_dropped_files_page(&page).unwrap();
    for (volid, pageid) in [(0, 897), (1, 129)] {
        let data = bytes(volid, pageid);
        let page =
            decode_page_envelope_with_profile(&data, vpid(volid, pageid), FormatProfile::Develop)
                .unwrap();
        assert!(matches!(
            decode_btree_page(&page, &decode_slotted_page(&page).unwrap(), true).unwrap(),
            BtreePageFact::Root(_)
        ));
    }
}

#[test]
fn real_develop_overflow_chain_preserves_all_links_and_total_length() {
    use volmap::format::{decode_overflow_continuation, decode_overflow_head};
    let data = bytes(1, 769);
    let page =
        decode_page_envelope_with_profile(&data, vpid(1, 769), FormatProfile::Develop).unwrap();
    let head = decode_overflow_head(&page).unwrap();
    assert_eq!(head.next(), Some(vpid(1, 770)));
    let mut remaining = head.total_length().unwrap() - u32::from(head.payload_length());
    for pageid in [770, 771] {
        let data = bytes(1, pageid);
        let page =
            decode_page_envelope_with_profile(&data, vpid(1, pageid), FormatProfile::Develop)
                .unwrap();
        let tail = decode_overflow_continuation(&page, remaining).unwrap();
        assert_eq!(
            tail.next(),
            if pageid == 770 {
                Some(vpid(1, 771))
            } else {
                None
            }
        );
        remaining -= u32::from(tail.payload_length());
    }
    assert_eq!(remaining, 0);
}

#[test]
fn wrong_profile_cannot_silently_accept_develop_heap_and_file_headers() {
    let data = bytes(0, 129);
    let page =
        decode_page_envelope_with_profile(&data, vpid(0, 129), FormatProfile::FeatOos).unwrap();
    let slots = decode_slotted_page(&page).unwrap();
    assert_eq!(
        decode_heap_page(&page, &slots, true).unwrap_err().rule(),
        "heap.page.role_length"
    );
    let data = bytes(0, 64);
    let page =
        decode_page_envelope_with_profile(&data, vpid(0, 64), FormatProfile::FeatOos).unwrap();
    assert_eq!(
        volmap::format::decode_file_header(&page)
            .unwrap_err()
            .rule(),
        "file.header.partial_table"
    );
}
