use core::fmt::Write as _;

use sha2::{Digest, Sha256};
use volmap::format::{
    BtreePageFact, HeapPageFact, IO_PAGE_SIZE, OosNext, PageType, RecordType, decode_btree_page,
    decode_catalog_page, decode_dropped_files_page, decode_file_header, decode_heap_page,
    decode_oos_chunk, decode_overflow_continuation, decode_overflow_head, decode_page_envelope,
    decode_sector_bitmap, decode_slotted_page, decode_vacuum_page, decode_volume_header,
};
use volmap::model::{PageId, VolId, Vpid};

type Fixture = (
    &'static [u8; IO_PAGE_SIZE],
    i16,
    i32,
    PageType,
    &'static str,
);

const FIXTURES: &[Fixture] = &[
    (
        include_bytes!("../fixtures/e1e651de/pages/vol0-page0.bin"),
        0,
        0,
        PageType::VolumeHeader,
        "4afa4e92ea5760fcc1d08295b41f2a6ea3a8e7e7c4ef47bf48ac9263c0ce2c9a",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol0-page1.bin"),
        0,
        1,
        PageType::VolumeBitmap,
        "57cb8c610f34c26c818cb51f4a2b6cc22647fbf243926b5b455bdff884cad319",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol0-page64.bin"),
        0,
        64,
        PageType::FileTable,
        "56703d76f09a0777374ab3ffa1c5a1f57a53c303db4b4b4d1474a7c599df14b7",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol0-page321.bin"),
        0,
        321,
        PageType::ExtensibleHash,
        "f6544cc4da208dc4ebddfc98c9f7571ef75993d21c26d6c7743bf3d67282f7d4",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol0-page577.bin"),
        0,
        577,
        PageType::Catalog,
        "42ea3bd54ade6e96db49e61b97bc5442576006e91409d0fd47e5c44e4af8810e",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol0-page641.bin"),
        0,
        641,
        PageType::VacuumData,
        "e3b296f75298ec33e16dfbbfc5cf8fec9b8d85153fb4f010a4962ce95c091e9b",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol0-page705.bin"),
        0,
        705,
        PageType::DroppedFiles,
        "69c40bcc8d53cab6fb3f8789d4c0421eec7695ddbc7d40ba129c30ded5a634a1",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page0.bin"),
        1,
        0,
        PageType::VolumeHeader,
        "6442c6aaa9edebb794b203b97f63b071c84e62e154c557a986b8e2f56f26de62",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page1.bin"),
        1,
        1,
        PageType::VolumeBitmap,
        "b2dc2cd64a2f5d1a290c9362cf9404e0ff60e5f7121cf91d853e88dc2357e08b",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page640.bin"),
        1,
        640,
        PageType::FileTable,
        "e4bc46effe9422a007946b8a7b6037e15740d3169e729f98847b6c3314e32639",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page641.bin"),
        1,
        641,
        PageType::Heap,
        "e5f4860d9a8e69fb9d120c7f50ac2cc538ca63a9b3803dd9f436a7cb191e9cc4",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page642.bin"),
        1,
        642,
        PageType::Heap,
        "2c66a157e098b9c8effdf321f935220e1785327e145ea408697b9e7bcc4e328a",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page704.bin"),
        1,
        704,
        PageType::FileTable,
        "a8c9f3e77527edd0a00d28a93288a04de6a5c6667f09a98349e44bc4f9efdf53",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page705.bin"),
        1,
        705,
        PageType::Btree,
        "7e5400c25da1d37cde2715e2816dc21f085a2cbdd656eb88b6182956980347ab",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page768.bin"),
        1,
        768,
        PageType::FileTable,
        "4cc62d4689ef20c1b1897ccb6153295816e4a0d231fd74c4cba4c702264361f7",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page769.bin"),
        1,
        769,
        PageType::Oos,
        "1412f5726b34b28fd568bb10a16304b8ab394bc9e81fda01a676f8f60e237cb7",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page770.bin"),
        1,
        770,
        PageType::Oos,
        "d6dba688583be079bd096cba6510d9001a9dd98124e85f323a05b62af9c263bf",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page771.bin"),
        1,
        771,
        PageType::Oos,
        "5ad84c862bf0aa60ead99c12f9c647ef8820142bc41772793870b5b44fd35bb7",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page772.bin"),
        1,
        772,
        PageType::Oos,
        "f164bbe425f564bfee4550d8575dcd2ab82734d31f96110adc45695a873c8252",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page832.bin"),
        1,
        832,
        PageType::FileTable,
        "c9163c1a74de9716a6566b3e1598d6e8b3e6a2338bc5ae23449e1bee3e5686be",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page833.bin"),
        1,
        833,
        PageType::Btree,
        "aebf419b4d249fa169c9145fac3f3cc66df618ad728f8e359dcd286ede8ac8c2",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page896.bin"),
        1,
        896,
        PageType::FileTable,
        "004588c0ed0e27b237b23f16837d31905eda93a8396623c886431bae7616b319",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page897.bin"),
        1,
        897,
        PageType::Heap,
        "3cd85812e9f7a1afc2e47d5193115aa1aa4bf1761dca2cba016e8b40c878775a",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page898.bin"),
        1,
        898,
        PageType::Heap,
        "b25b0856d2938f2c60fd5a8537d1face372bdf08e6997d665a52aaa5cb60269a",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page960.bin"),
        1,
        960,
        PageType::FileTable,
        "83234dd0c1012ce8b61359b9db7842b23817f026b28c2d49205ad833430e0cf9",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page961.bin"),
        1,
        961,
        PageType::Overflow,
        "65aa4d2a5207d2f98ed753ee781ffa41ee7a3787682ac34bb146d3a713100c38",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page962.bin"),
        1,
        962,
        PageType::Overflow,
        "c20524c0b1f95a7a0b8b44819b0e9c9eb0601ea5206bff764d59d09a77ee7df8",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page1088.bin"),
        1,
        1088,
        PageType::FileTable,
        "f60944af78e2b670b0e5f72a256d78b623a6625a8b23122583afcb80c5281085",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page1089.bin"),
        1,
        1089,
        PageType::Heap,
        "0c2c28be58ae74f269d28c5bae2cd3394664ce1b86b493a3a9dded4e70c8c82e",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page1152.bin"),
        1,
        1152,
        PageType::FileTable,
        "f2394f4600506f4963e753eb66635aede1a89c3659b1a67f784262e12d571090",
    ),
    (
        include_bytes!("../fixtures/e1e651de/pages/vol1-page1153.bin"),
        1,
        1153,
        PageType::Btree,
        "405a4e12d06d89d67d37efce3de5ebe592542d7aa782656dbf5e3672bab137f9",
    ),
];

fn vpid(vol_id: i16, page_id: i32) -> Vpid {
    Vpid::new(VolId::new(vol_id).unwrap(), PageId::new(page_id).unwrap())
}

fn page(
    bytes: &'static [u8; IO_PAGE_SIZE],
    vol_id: i16,
    page_id: i32,
) -> volmap::format::DecodedPageEnvelope<'static> {
    decode_page_envelope(bytes, vpid(vol_id, page_id)).unwrap()
}

fn sha256(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut encoded, "{byte:02x}").unwrap();
    }
    encoded
}

#[test]
fn pinned_pages_match_manifest_hashes_identities_and_types() {
    for (bytes, vol_id, page_id, page_type, expected_hash) in FIXTURES {
        assert_eq!(sha256(*bytes), *expected_hash);
        assert_eq!(page(bytes, *vol_id, *page_id).page_type(), *page_type);
    }
}

#[test]
fn volume_geometry_and_bitmaps_decode_from_engine_pages() {
    let primary_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol0-page0.bin"),
        0,
        0,
    );
    let primary = decode_volume_header(&primary_page, 67_108_864).unwrap();
    let primary_bitmap_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol0-page1.bin"),
        0,
        1,
    );
    let primary_bitmap = decode_sector_bitmap(&primary_bitmap_page, &primary, 0).unwrap();
    assert_eq!(primary.total_sectors(), 64);
    assert!(
        primary_bitmap
            .is_reserved(volmap::model::SectorId::new(0).unwrap())
            .unwrap()
    );

    let extension_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page0.bin"),
        1,
        0,
    );
    let extension = decode_volume_header(&extension_page, 134_217_728).unwrap();
    let extension_bitmap_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page1.bin"),
        1,
        1,
    );
    let extension_bitmap = decode_sector_bitmap(&extension_bitmap_page, &extension, 0).unwrap();
    assert_eq!(extension.total_sectors(), 128);
    assert!(
        extension_bitmap
            .is_reserved(volmap::model::SectorId::new(0).unwrap())
            .unwrap()
    );
}

#[test]
fn file_headers_preserve_fixture_ownership_graph() {
    let cases = [
        (
            include_bytes!("../fixtures/e1e651de/pages/vol0-page64.bin"),
            0,
            64,
            "tracker",
        ),
        (
            include_bytes!("../fixtures/e1e651de/pages/vol1-page640.bin"),
            1,
            640,
            "heap-reuse-slots",
        ),
        (
            include_bytes!("../fixtures/e1e651de/pages/vol1-page704.bin"),
            1,
            704,
            "btree",
        ),
        (
            include_bytes!("../fixtures/e1e651de/pages/vol1-page768.bin"),
            1,
            768,
            "oos",
        ),
        (
            include_bytes!("../fixtures/e1e651de/pages/vol1-page960.bin"),
            1,
            960,
            "multipage-object-heap",
        ),
    ];
    for (bytes, vol_id, page_id, expected_type) in cases {
        let envelope = page(bytes, vol_id, page_id);
        assert_eq!(
            decode_file_header(&envelope).unwrap().file_type().as_str(),
            expected_type
        );
    }
}

#[test]
fn heap_btree_catalog_hash_and_vacuum_families_decode() {
    let heap_header_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page641.bin"),
        1,
        641,
    );
    let heap_header_slots = decode_slotted_page(&heap_header_page).unwrap();
    let HeapPageFact::Header(heap_header) =
        decode_heap_page(&heap_header_page, &heap_header_slots, true).unwrap()
    else {
        panic!("expected heap header");
    };
    assert_eq!(heap_header.oos_vfid.unwrap().file_id.get(), 768);

    let heap_data_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page642.bin"),
        1,
        642,
    );
    let heap_data_slots = decode_slotted_page(&heap_data_page).unwrap();
    assert!(matches!(
        decode_heap_page(&heap_data_page, &heap_data_slots, false).unwrap(),
        HeapPageFact::Chain(_)
    ));

    for (bytes, page_id) in [
        (
            include_bytes!("../fixtures/e1e651de/pages/vol1-page705.bin"),
            705,
        ),
        (
            include_bytes!("../fixtures/e1e651de/pages/vol1-page833.bin"),
            833,
        ),
        (
            include_bytes!("../fixtures/e1e651de/pages/vol1-page1153.bin"),
            1153,
        ),
    ] {
        let envelope = page(bytes, 1, page_id);
        let slotted = decode_slotted_page(&envelope).unwrap();
        assert!(matches!(
            decode_btree_page(&envelope, &slotted, true).unwrap(),
            BtreePageFact::Root(_)
        ));
    }

    let ehash_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol0-page321.bin"),
        0,
        321,
    );
    assert!(!decode_slotted_page(&ehash_page).unwrap().slots().is_empty());
    let catalog_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol0-page577.bin"),
        0,
        577,
    );
    let catalog_slots = decode_slotted_page(&catalog_page).unwrap();
    assert!(decode_catalog_page(&catalog_page, &catalog_slots).is_ok());
    let vacuum_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol0-page641.bin"),
        0,
        641,
    );
    assert!(decode_vacuum_page(&vacuum_page).is_ok());
    let dropped_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol0-page705.bin"),
        0,
        705,
    );
    assert!(decode_dropped_files_page(&dropped_page).is_ok());
}

#[test]
fn oos_single_and_multichunk_values_decode_without_payload_exposure() {
    let page770 = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page770.bin"),
        1,
        770,
    );
    let slots770 = decode_slotted_page(&page770).unwrap();
    let single = decode_oos_chunk(&page770, &slots770, 0).unwrap();
    assert_eq!(single.total_data_length(), 3_008);
    assert_eq!(single.next(), OosNext::Terminal);

    let page772 = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page772.bin"),
        1,
        772,
    );
    let slots772 = decode_slotted_page(&page772).unwrap();
    let head = decode_oos_chunk(&page772, &slots772, 0).unwrap();
    assert_eq!(head.total_data_length(), 32_776);
    assert_eq!(head.chunk_index(), 0);
    assert_eq!(
        head.next(),
        OosNext::Link(volmap::model::Oid::new(
            VolId::new(1).unwrap(),
            PageId::new(771).unwrap(),
            volmap::model::SlotId::new(0).unwrap()
        ))
    );

    let page771 = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page771.bin"),
        1,
        771,
    );
    let slots771 = decode_slotted_page(&page771).unwrap();
    let middle = decode_oos_chunk(&page771, &slots771, 0).unwrap();
    let tail = decode_oos_chunk(&page770, &slots770, 1).unwrap();
    assert_eq!((middle.chunk_index(), tail.chunk_index()), (1, 2));
    assert_eq!(tail.next(), OosNext::Terminal);
    assert_eq!(
        u32::from(head.payload_length())
            + u32::from(middle.payload_length())
            + u32::from(tail.payload_length()),
        32_776
    );
}

#[test]
fn overflow_head_and_continuation_form_a_complete_engine_chain() {
    let head_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page961.bin"),
        1,
        961,
    );
    let head = decode_overflow_head(&head_page).unwrap();
    assert_eq!(head.next(), Some(vpid(1, 962)));
    let total = head.total_length().unwrap();
    let remaining = total - u32::from(head.payload_length());

    let tail_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page962.bin"),
        1,
        962,
    );
    let tail = decode_overflow_continuation(&tail_page, remaining).unwrap();
    assert_eq!(tail.next(), None);
    assert_eq!(
        u32::from(head.payload_length()) + u32::from(tail.payload_length()),
        total
    );

    let data_page = page(
        include_bytes!("../fixtures/e1e651de/pages/vol1-page898.bin"),
        1,
        898,
    );
    let heap_slots = decode_slotted_page(&data_page).unwrap();
    assert!(
        heap_slots
            .slots()
            .iter()
            .any(|slot| matches!(slot.record_type(), RecordType::Home | RecordType::BigOne))
    );
}
