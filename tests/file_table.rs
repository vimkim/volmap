use volmap::format::{
    FILE_DESCRIPTOR_SIZE, FileClassAssociation, FileDescriptor, FileType, IO_PAGE_SIZE, PageType,
    decode_extdata_header, decode_file_descriptor, decode_file_header, decode_full_sectors,
    decode_page_envelope, decode_partial_sectors, decode_tracker_items, decode_user_pages,
};
use volmap::model::{FileId, PageId, Vfid, VolId, Vpid};

fn file_page() -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    page[8..12].copy_from_slice(&5_i32.to_le_bytes());
    page[12..14].copy_from_slice(&1_i16.to_le_bytes());
    page[14] = PageType::FileTable.ordinal();
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[8..12].copy_from_slice(&5_i32.to_le_bytes());
    user[12..14].copy_from_slice(&1_i16.to_le_bytes());
    user[104..108].copy_from_slice(&3_i32.to_le_bytes());
    user[108..112].copy_from_slice(&1_i32.to_le_bytes());
    user[112..116].copy_from_slice(&1_i32.to_le_bytes());
    user[116..120].copy_from_slice(&1_i32.to_le_bytes());
    user[124..128].copy_from_slice(&1_i32.to_le_bytes());
    user[128..132].copy_from_slice(&1_i32.to_le_bytes());
    user[140..144].copy_from_slice(&13_i32.to_le_bytes());
    user[144..148].copy_from_slice(&1_u32.to_le_bytes());
    user[150..152].copy_from_slice(&216_i16.to_le_bytes());
    user[152..154].copy_from_slice(&232_i16.to_le_bytes());
    user[154..156].copy_from_slice(&248_i16.to_le_bytes());
    user[156..160].copy_from_slice(&(-1_i32).to_le_bytes());
    user[160..162].copy_from_slice(&(-1_i16).to_le_bytes());
    for offset in [216_usize, 232, 248] {
        user[offset..offset + 4].copy_from_slice(&(-1_i32).to_le_bytes());
        user[offset + 4..offset + 6].copy_from_slice(&(-1_i16).to_le_bytes());
        user[offset + 8..offset + 10].copy_from_slice(&16_i16.to_le_bytes());
        user[offset + 10..offset + 12].copy_from_slice(&16_i16.to_le_bytes());
    }
    page
}

fn envelope(bytes: &[u8]) -> volmap::format::DecodedPageEnvelope<'_> {
    decode_page_envelope(
        bytes,
        Vpid::new(VolId::new(1).unwrap(), PageId::new(5).unwrap()),
    )
    .unwrap()
}

fn file_vfid() -> Vfid {
    Vfid::new(VolId::new(1).unwrap(), FileId::new(5).unwrap())
}

fn write_oid(descriptor: &mut [u8; FILE_DESCRIPTOR_SIZE], offset: usize, values: (i32, i16, i16)) {
    descriptor[offset..offset + 4].copy_from_slice(&values.0.to_le_bytes());
    descriptor[offset + 4..offset + 6].copy_from_slice(&values.1.to_le_bytes());
    descriptor[offset + 6..offset + 8].copy_from_slice(&values.2.to_le_bytes());
}

fn write_file_identity(
    descriptor: &mut [u8; FILE_DESCRIPTOR_SIZE],
    offset: usize,
    values: (i32, i16, i32),
) {
    descriptor[offset..offset + 4].copy_from_slice(&values.0.to_le_bytes());
    descriptor[offset + 4..offset + 6].copy_from_slice(&values.1.to_le_bytes());
    descriptor[offset + 8..offset + 12].copy_from_slice(&values.2.to_le_bytes());
}

fn source_derived_descriptor(file_type: FileType) -> [u8; FILE_DESCRIPTOR_SIZE] {
    let mut descriptor = [0_u8; FILE_DESCRIPTOR_SIZE];
    match file_type {
        FileType::Heap | FileType::HeapReuseSlots => {
            write_oid(&mut descriptor, 0, (77, 3, 1));
            write_file_identity(&mut descriptor, 8, (5, 1, 129));
        }
        FileType::MultipageObjectHeap => {
            write_file_identity(&mut descriptor, 0, (128, 0, 129));
            write_oid(&mut descriptor, 12, (77, 3, 1));
        }
        FileType::Btree | FileType::ExtensibleHash | FileType::HashDirectory => {
            write_oid(&mut descriptor, 0, (77, 3, 1));
            descriptor[8..12].copy_from_slice(&42_i32.to_le_bytes());
        }
        FileType::BtreeOverflowKey => {
            write_file_identity(&mut descriptor, 0, (256, 1, 257));
            write_oid(&mut descriptor, 12, (77, 3, 1));
        }
        _ => panic!("not a class-associated source fixture"),
    }
    descriptor
}

const SCOPED_CLASS_FILES: [FileType; 7] = [
    FileType::Heap,
    FileType::HeapReuseSlots,
    FileType::MultipageObjectHeap,
    FileType::Btree,
    FileType::BtreeOverflowKey,
    FileType::ExtensibleHash,
    FileType::HashDirectory,
];

const fn class_oid_offset(file_type: FileType) -> usize {
    match file_type {
        FileType::MultipageObjectHeap | FileType::BtreeOverflowKey => 12,
        FileType::Heap
        | FileType::HeapReuseSlots
        | FileType::Btree
        | FileType::ExtensibleHash
        | FileType::HashDirectory => 0,
        _ => panic!("not a class-associated file type"),
    }
}

#[test]
fn scoped_descriptors_decode_source_traced_class_offsets_and_related_facts() {
    for file_type in SCOPED_CLASS_FILES {
        let descriptor = source_derived_descriptor(file_type);
        let decoded = decode_file_descriptor(&descriptor, file_type, file_vfid()).unwrap();
        let mut page = file_page();
        let user = &mut page[32..IO_PAGE_SIZE - 8];
        user[40..40 + FILE_DESCRIPTOR_SIZE].copy_from_slice(&descriptor);
        user[140..144].copy_from_slice(&file_type.ordinal().to_le_bytes());
        let header = decode_file_header(&envelope(&page)).unwrap();
        assert_eq!(header.vfid(), file_vfid(), "{file_type:?}");
        assert_eq!(header.file_type(), file_type, "{file_type:?}");
        assert_eq!(header.descriptor(), decoded, "{file_type:?}");
        assert_eq!(
            header.class_association(),
            decoded.class_association(),
            "{file_type:?}"
        );
        let FileClassAssociation::Associated(class_oid) = decoded.class_association() else {
            panic!("{file_type:?} did not retain its class association");
        };
        assert_eq!(class_oid.page_id.get(), 77, "{file_type:?}");
        assert_eq!(class_oid.slot_id.get(), 3, "{file_type:?}");
        assert_eq!(class_oid.vol_id.get(), 1, "{file_type:?}");

        match (file_type, decoded) {
            (FileType::Heap | FileType::HeapReuseSlots, FileDescriptor::Heap { hfid, .. }) => {
                assert_eq!(hfid.vfid, file_vfid());
                assert_eq!(hfid.header_page_id.get(), 129);
            }
            (FileType::MultipageObjectHeap, FileDescriptor::MultipageObjectHeap { hfid, .. }) => {
                assert_eq!(hfid.vfid.file_id.get(), 128);
                assert_eq!(hfid.vfid.vol_id.get(), 0);
                assert_eq!(hfid.header_page_id.get(), 129);
            }
            (
                FileType::Btree,
                FileDescriptor::Btree {
                    attribute_id: 42, ..
                },
            )
            | (
                FileType::ExtensibleHash,
                FileDescriptor::ExtensibleHash {
                    attribute_id: 42, ..
                },
            )
            | (
                FileType::HashDirectory,
                FileDescriptor::HashDirectory {
                    attribute_id: 42, ..
                },
            ) => {}
            (FileType::BtreeOverflowKey, FileDescriptor::BtreeOverflowKey { btid, .. }) => {
                assert_eq!(btid.vfid.file_id.get(), 256);
                assert_eq!(btid.vfid.vol_id.get(), 1);
                assert_eq!(btid.root_page_id.get(), 257);
            }
            _ => panic!("{file_type:?} decoded to the wrong descriptor variant"),
        }
    }
}

#[test]
fn scoped_descriptors_distinguish_null_and_system_class_oids() {
    for file_type in SCOPED_CLASS_FILES {
        let mut null = source_derived_descriptor(file_type);
        write_oid(&mut null, class_oid_offset(file_type), (-1, -1, -1));
        let decoded = decode_file_descriptor(&null, file_type, file_vfid()).unwrap();
        assert_eq!(
            decoded.class_association(),
            FileClassAssociation::Null,
            "{file_type:?}"
        );

        let mut system = source_derived_descriptor(file_type);
        write_oid(&mut system, class_oid_offset(file_type), (0, 0, 0));
        let decoded = decode_file_descriptor(&system, file_type, file_vfid()).unwrap();
        let FileClassAssociation::Associated(class_oid) = decoded.class_association() else {
            panic!("{file_type:?} rejected a source-valid system OID");
        };
        assert_eq!(class_oid.page_id.get(), 0, "{file_type:?}");
        assert_eq!(class_oid.slot_id.get(), 0, "{file_type:?}");
        assert_eq!(class_oid.vol_id.get(), 0, "{file_type:?}");
    }
}

#[test]
fn every_scoped_descriptor_rejects_short_and_malformed_input() {
    for file_type in SCOPED_CLASS_FILES {
        let descriptor = source_derived_descriptor(file_type);
        assert_eq!(
            decode_file_descriptor(
                &descriptor[..FILE_DESCRIPTOR_SIZE - 1],
                file_type,
                file_vfid()
            )
            .unwrap_err()
            .rule(),
            "file.descriptor.length",
            "{file_type:?}"
        );

        let mut malformed = descriptor;
        write_oid(&mut malformed, class_oid_offset(file_type), (-1, -1, 0));
        assert_eq!(
            decode_file_descriptor(&malformed, file_type, file_vfid())
                .unwrap_err()
                .kind(),
            volmap::format::DecodeErrorKind::InvalidGeometry,
            "{file_type:?}"
        );
    }
}

#[test]
fn descriptors_type_non_class_and_deferred_oos_associations() {
    let descriptor = [0_u8; FILE_DESCRIPTOR_SIZE];
    for file_type in [
        FileType::Tracker,
        FileType::Catalog,
        FileType::DroppedFiles,
        FileType::VacuumData,
    ] {
        assert_eq!(
            decode_file_descriptor(&descriptor, file_type, file_vfid())
                .unwrap()
                .class_association(),
            FileClassAssociation::Internal,
            "{file_type:?}"
        );
    }
    for file_type in [FileType::QueryArea, FileType::Temporary, FileType::Unknown] {
        assert_eq!(
            decode_file_descriptor(&descriptor, file_type, file_vfid())
                .unwrap()
                .class_association(),
            FileClassAssociation::NoSingleClass,
            "{file_type:?}"
        );
    }

    let mut oos = descriptor;
    write_file_identity(&mut oos, 0, (128, 0, 129));
    let decoded = decode_file_descriptor(&oos, FileType::Oos, file_vfid()).unwrap();
    assert_eq!(
        decoded.class_association(),
        FileClassAssociation::DeferredOos
    );
    let FileDescriptor::Oos { related_heap } = decoded else {
        panic!("OOS descriptor did not retain its related heap");
    };
    assert_eq!(related_heap.vfid.file_id.get(), 128);
    assert_eq!(related_heap.header_page_id.get(), 129);
}

#[test]
fn file_header_validates_identity_accounting_and_table_boundaries() {
    let bytes = file_page();
    let envelope = envelope(&bytes);
    let header = decode_file_header(&envelope).unwrap();

    assert_eq!(header.vfid().file_id.get(), 5);
    assert_eq!(header.file_type().as_str(), "oos");
    assert_eq!(header.page_total(), 3);
    assert_eq!(header.partial_table_offset(), Some(216));
    let table = decode_extdata_header(&envelope, 216, 16).unwrap();
    assert_eq!(table.item_count, 0);
    assert!(table.next.is_none());
}

#[test]
fn file_header_rejects_forged_self_and_counter_totals() {
    let mut identity = file_page();
    identity[32 + 8..32 + 12].copy_from_slice(&6_i32.to_le_bytes());
    assert_eq!(
        decode_file_header(&envelope(&identity)).unwrap_err().rule(),
        "file.header.self_identity"
    );

    let mut accounting = file_page();
    accounting[32 + 104..32 + 108].copy_from_slice(&4_i32.to_le_bytes());
    assert_eq!(
        decode_file_header(&envelope(&accounting))
            .unwrap_err()
            .rule(),
        "file.header.page_accounting"
    );
}

#[test]
fn extdata_rejects_untrusted_item_width_and_count_arithmetic() {
    let mut bytes = file_page();
    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    user[226..228].copy_from_slice(&8_i16.to_le_bytes());
    let envelope = envelope(&bytes);
    assert_eq!(
        decode_extdata_header(&envelope, 216, 16)
            .unwrap_err()
            .rule(),
        "file.extdata.bounds"
    );
}

#[test]
fn allocation_items_decode_ids_bitmaps_and_marked_user_pages_without_payloads() {
    let mut partial_bytes = file_page();
    let user = &mut partial_bytes[32..IO_PAGE_SIZE - 8];
    // max_size is item capacity and deliberately excludes the 16-byte header.
    user[224..226].copy_from_slice(&16_i16.to_le_bytes());
    user[226..228].copy_from_slice(&16_i16.to_le_bytes());
    user[228..230].copy_from_slice(&1_i16.to_le_bytes());
    user[232..236].copy_from_slice(&7_i32.to_le_bytes());
    user[236..238].copy_from_slice(&1_i16.to_le_bytes());
    user[240..248].copy_from_slice(&0x8000_0000_0000_0005_u64.to_le_bytes());
    let partial_envelope = envelope(&partial_bytes);
    let partial_header = decode_extdata_header(&partial_envelope, 216, 16).unwrap();
    let partial = decode_partial_sectors(&partial_envelope, partial_header).unwrap();
    assert_eq!(partial[0].sector_id.get(), 7);
    assert_eq!(partial[0].page_bitmap, 0x8000_0000_0000_0005);

    let mut item_bytes = file_page();
    let user = &mut item_bytes[32..IO_PAGE_SIZE - 8];
    user[256..258].copy_from_slice(&32_i16.to_le_bytes());
    user[258..260].copy_from_slice(&8_i16.to_le_bytes());
    user[260..262].copy_from_slice(&1_i16.to_le_bytes());
    user[264..268].copy_from_slice(&9_i32.to_le_bytes());
    user[268..270].copy_from_slice(&1_i16.to_le_bytes());
    let item_envelope = envelope(&item_bytes);
    let item_header = decode_extdata_header(&item_envelope, 248, 8).unwrap();
    let full = decode_full_sectors(&item_envelope, item_header).unwrap();
    assert_eq!(full[0].1.get(), 9);

    user_page_at(&mut item_bytes, 248, 11, 1, true);
    let item_envelope = envelope(&item_bytes);
    let item_header = decode_extdata_header(&item_envelope, 248, 8).unwrap();
    let pages = decode_user_pages(&item_envelope, item_header).unwrap();
    assert_eq!(pages[0].vpid.page_id.get(), 11);
    assert!(pages[0].marked_deleted);
}

fn user_page_at(
    bytes: &mut [u8; IO_PAGE_SIZE],
    table_offset: usize,
    page_id: u32,
    vol_id: i16,
    marked: bool,
) {
    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    let raw = page_id | if marked { 0x8000_0000 } else { 0 };
    user[table_offset + 16..table_offset + 20].copy_from_slice(&raw.to_le_bytes());
    user[table_offset + 20..table_offset + 22].copy_from_slice(&vol_id.to_le_bytes());
}

#[test]
fn tracker_items_decode_typed_file_identity_and_constrained_metadata() {
    let mut bytes = file_page();
    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    user[224..226].copy_from_slice(&32_i16.to_le_bytes());
    user[226..228].copy_from_slice(&16_i16.to_le_bytes());
    user[228..230].copy_from_slice(&1_i16.to_le_bytes());
    user[232..236].copy_from_slice(&128_i32.to_le_bytes());
    user[236..238].copy_from_slice(&1_i16.to_le_bytes());
    user[238..240].copy_from_slice(&2_i16.to_le_bytes());
    user[240..248].copy_from_slice(&1_u64.to_le_bytes());
    let decoded = envelope(&bytes);
    let header = decode_extdata_header(&decoded, 216, 16).unwrap();
    let items = decode_tracker_items(&decoded, header).unwrap();

    assert_eq!(items[0].vfid.file_id.get(), 128);
    assert_eq!(items[0].file_type.as_str(), "heap-reuse-slots");
    assert!(items[0].heap_marked_deleted);

    let mut invalid = bytes;
    invalid[32 + 240..32 + 248].copy_from_slice(&2_u64.to_le_bytes());
    let envelope = envelope(&invalid);
    let header = decode_extdata_header(&envelope, 216, 16).unwrap();
    assert_eq!(
        decode_tracker_items(&envelope, header).unwrap_err().rule(),
        "file.tracker.heap_metadata"
    );
}

#[test]
fn heap_file_descriptor_pins_the_header_page_role() {
    let mut bytes = file_page();
    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    user[140..144].copy_from_slice(&1_i32.to_le_bytes());
    user[40..44].copy_from_slice(&77_i32.to_le_bytes());
    user[44..46].copy_from_slice(&3_i16.to_le_bytes());
    user[46..48].copy_from_slice(&1_i16.to_le_bytes());
    user[48..52].copy_from_slice(&5_i32.to_le_bytes());
    user[52..54].copy_from_slice(&1_i16.to_le_bytes());
    user[56..60].copy_from_slice(&129_i32.to_le_bytes());
    let header = decode_file_header(&envelope(&bytes)).unwrap();
    assert_eq!(header.class_oid().unwrap().page_id.get(), 77);
    assert_eq!(header.class_oid().unwrap().slot_id.get(), 3);
    assert_eq!(header.heap_header_page().unwrap().page_id.get(), 129);

    let mut wrong_hfid = bytes;
    let user = &mut wrong_hfid[32..IO_PAGE_SIZE - 8];
    user[48..52].copy_from_slice(&6_i32.to_le_bytes());
    assert_eq!(
        decode_file_header(&envelope(&wrong_hfid))
            .unwrap_err()
            .rule(),
        "file.header.heap_hfid"
    );

    let mut partial_null = bytes;
    let user = &mut partial_null[32..IO_PAGE_SIZE - 8];
    user[40..44].copy_from_slice(&(-1_i32).to_le_bytes());
    user[44..46].copy_from_slice(&(-1_i16).to_le_bytes());
    assert_eq!(
        decode_file_header(&envelope(&partial_null))
            .unwrap_err()
            .rule(),
        "file.header.heap_class"
    );
}

#[test]
fn overflow_file_descriptor_pins_its_related_heap() {
    let mut bytes = file_page();
    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    user[140..144].copy_from_slice(&3_i32.to_le_bytes());
    user[40..44].copy_from_slice(&128_i32.to_le_bytes());
    user[44..46].copy_from_slice(&0_i16.to_le_bytes());
    user[48..52].copy_from_slice(&129_i32.to_le_bytes());
    let header = decode_file_header(&envelope(&bytes)).unwrap();
    let (heap, heap_header) = header.related_heap().unwrap();
    assert_eq!(heap.file_id.get(), 128);
    assert_eq!(heap_header.page_id.get(), 129);
}
