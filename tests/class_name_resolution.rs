use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use volmap::export::export_html;
use volmap::format::{DB_PAGE_SIZE, FileType, IO_PAGE_SIZE, PageType};
use volmap::inspection::{
    CancelToken, ClassAssociation, ClassAssociationNotApplicableReason,
    ClassAssociationUnresolvedReason, ClassNameResolution, ClassNameUnresolvedReason, Inspection,
    OpenRequest, PageFileAssociation, ResourcePolicy, RevisionSelector,
};
use volmap::model::{FileId, Oid, PageAllocationClass, PageId, SlotId, Vfid, VolId, Vpid};
use volmap::projection::page_projection;
use volmap::source::InputSpec;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
const CLASS_PAGE: i32 = 6;
const CLASS_SLOT: i16 = 1;
const TARGET_PAGE: i32 = 10;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "volmap-class-name-test-{}-{sequence}",
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum ClassRecord {
    Home,
    NewHome,
    Relocation,
    BigOne,
    BigOneCycle,
    BigOneWrongOwner,
    BrokenTerminator,
    Dead,
    Encrypted,
    MissingPage,
    MissingSlot,
    RelocationChain,
    RelocationCycle,
    WrongPage,
}

impl ClassRecord {
    fn class_oid(self) -> Oid {
        let (page_id, slot_id) = match self {
            Self::MissingPage => (4_096, CLASS_SLOT),
            Self::MissingSlot => (CLASS_PAGE, 7),
            Self::Home
            | Self::NewHome
            | Self::Relocation
            | Self::BigOne
            | Self::BigOneCycle
            | Self::BigOneWrongOwner
            | Self::BrokenTerminator
            | Self::Dead
            | Self::Encrypted
            | Self::RelocationChain
            | Self::RelocationCycle
            | Self::WrongPage => (CLASS_PAGE, CLASS_SLOT),
        };
        Oid::new(
            VolId::new(0).unwrap(),
            PageId::new(page_id).unwrap(),
            SlotId::new(slot_id).unwrap(),
        )
    }
}

#[derive(Clone, Copy)]
struct ResolutionFailureCase {
    codeset: u8,
    name: &'static [u8],
    record: ClassRecord,
    reason: ClassNameUnresolvedReason,
    reason_code: &'static str,
    message: &'static str,
}

const RESOLUTION_FAILURE_CASES: &[ResolutionFailureCase] = &[
    ResolutionFailureCase {
        codeset: 42,
        name: b"table",
        record: ClassRecord::Home,
        reason: ClassNameUnresolvedReason::DatabaseCodeset(
            volmap::inspection::DatabaseCodesetFailure::Unsupported(42),
        ),
        reason_code: "class-name.codeset-unsupported",
        message: "database codeset is unsupported",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: &[0x80],
        record: ClassRecord::Home,
        reason: ClassNameUnresolvedReason::InvalidIdentifier(
            volmap::inspection::ClassIdentifierFailure::NonAscii,
        ),
        reason_code: "class-name.identifier-non-ascii",
        message: "ASCII database contains a non-ASCII class name",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: b"table",
        record: ClassRecord::Encrypted,
        reason: ClassNameUnresolvedReason::EncryptedPage,
        reason_code: "class-name.page-encrypted-opaque",
        message: "class record page is encrypted and unavailable",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: b"table",
        record: ClassRecord::MissingPage,
        reason: ClassNameUnresolvedReason::PageUnavailable,
        reason_code: "class-name.page-unavailable",
        message: "class record page could not be read",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: b"table",
        record: ClassRecord::MissingSlot,
        reason: ClassNameUnresolvedReason::MissingSlot,
        reason_code: "class-name.slot-missing",
        message: "class record slot does not exist",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: b"table",
        record: ClassRecord::Dead,
        reason: ClassNameUnresolvedReason::DeadRecord(volmap::format::RecordType::MarkDeleted),
        reason_code: "class-name.record-not-live",
        message: "class record slot is not live",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: b"table",
        record: ClassRecord::RelocationChain,
        reason: ClassNameUnresolvedReason::InvalidFormat("heap.relocation.target_slot_role"),
        reason_code: "heap.relocation.target_slot_role",
        message: "class record format validation failed",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: b"table",
        record: ClassRecord::RelocationCycle,
        reason: ClassNameUnresolvedReason::RelocationCycle,
        reason_code: "class-name.relocation-cycle",
        message: "class record relocation cycle",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: b"table",
        record: ClassRecord::BigOneCycle,
        reason: ClassNameUnresolvedReason::InvalidFormat("overflow.chain.acyclic"),
        reason_code: "overflow.chain.acyclic",
        message: "class record format validation failed",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: b"table",
        record: ClassRecord::BigOneWrongOwner,
        reason: ClassNameUnresolvedReason::InvalidOwnership("class.name.bigone.overflow_owner"),
        reason_code: "class.name.bigone.overflow_owner",
        message: "class record ownership validation failed",
    },
    ResolutionFailureCase {
        codeset: 0,
        name: b"table",
        record: ClassRecord::WrongPage,
        reason: ClassNameUnresolvedReason::InvalidFormat("slotted.page.type"),
        reason_code: "slotted.page.type",
        message: "class record format validation failed",
    },
];

#[derive(Clone, Copy, Eq, PartialEq)]
enum InventoryFixture {
    Complete,
    HeaderMismatch,
    MissingHeader,
    OwnerConflict,
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

fn volume_header_page(codeset: u8) -> [u8; IO_PAGE_SIZE] {
    let mut page = page(0, PageType::VolumeHeader);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
    user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
    user[30] = codeset;
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

fn put_oid(bytes: &mut [u8], offset: usize, page_id: i32, slot_id: i16) {
    bytes[offset..offset + 4].copy_from_slice(&page_id.to_le_bytes());
    bytes[offset + 4..offset + 6].copy_from_slice(&slot_id.to_le_bytes());
    bytes[offset + 6..offset + 8].copy_from_slice(&0_i16.to_le_bytes());
}

fn put_null_oid(bytes: &mut [u8], offset: usize) {
    bytes[offset..offset + 4].copy_from_slice(&(-1_i32).to_le_bytes());
    bytes[offset + 4..offset + 6].copy_from_slice(&(-1_i16).to_le_bytes());
    bytes[offset + 6..offset + 8].copy_from_slice(&(-1_i16).to_le_bytes());
}

fn file_header_page(
    file_id: i32,
    file_type: FileType,
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
    user[140..144].copy_from_slice(&file_type.ordinal().to_le_bytes());
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

fn configure_heap_descriptor(
    page: &mut [u8; IO_PAGE_SIZE],
    file_id: i32,
    heap_header_page: i32,
    class_oid: Option<Oid>,
) {
    let descriptor = &mut page[32 + 40..32 + 104];
    match class_oid {
        Some(oid) => put_oid(descriptor, 0, oid.page_id.get(), oid.slot_id.get()),
        None => put_null_oid(descriptor, 0),
    }
    descriptor[8..12].copy_from_slice(&file_id.to_le_bytes());
    descriptor[12..14].copy_from_slice(&0_i16.to_le_bytes());
    descriptor[16..20].copy_from_slice(&heap_header_page.to_le_bytes());
    descriptor[20..22].copy_from_slice(&0_i16.to_le_bytes());
}

fn configure_overflow_descriptor(page: &mut [u8; IO_PAGE_SIZE]) {
    let descriptor = &mut page[32 + 40..32 + 104];
    descriptor[0..4].copy_from_slice(&4_i32.to_le_bytes());
    descriptor[4..6].copy_from_slice(&0_i16.to_le_bytes());
    descriptor[8..12].copy_from_slice(&5_i32.to_le_bytes());
    descriptor[12..14].copy_from_slice(&0_i16.to_le_bytes());
    put_null_oid(descriptor, 12);
}

fn tracker_items_page(items: &[(i32, FileType)]) -> [u8; IO_PAGE_SIZE] {
    let mut page = page(3, PageType::FileTable);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    put_null_vpid(user, 0);
    let item_bytes = i16::try_from(items.len() * 16).unwrap();
    user[8..10].copy_from_slice(&item_bytes.to_le_bytes());
    user[10..12].copy_from_slice(&16_i16.to_le_bytes());
    user[12..14].copy_from_slice(&i16::try_from(items.len()).unwrap().to_le_bytes());
    for (index, (file_id, file_type)) in items.iter().copied().enumerate() {
        let offset = 16 + index * 16;
        user[offset..offset + 4].copy_from_slice(&file_id.to_le_bytes());
        user[offset + 4..offset + 6].copy_from_slice(&0_i16.to_le_bytes());
        user[offset + 6..offset + 8]
            .copy_from_slice(&i16::try_from(file_type.ordinal()).unwrap().to_le_bytes());
    }
    page
}

fn configure_class_descriptor(page: &mut [u8; IO_PAGE_SIZE], file_type: FileType, class_oid: Oid) {
    let descriptor = &mut page[32 + 40..32 + 104];
    match file_type {
        FileType::MultipageObjectHeap => {
            descriptor[0..4].copy_from_slice(&7_i32.to_le_bytes());
            descriptor[4..6].copy_from_slice(&0_i16.to_le_bytes());
            descriptor[8..12].copy_from_slice(&8_i32.to_le_bytes());
            put_oid(
                descriptor,
                12,
                class_oid.page_id.get(),
                class_oid.slot_id.get(),
            );
        }
        FileType::Btree | FileType::ExtensibleHash | FileType::HashDirectory => {
            put_oid(
                descriptor,
                0,
                class_oid.page_id.get(),
                class_oid.slot_id.get(),
            );
            descriptor[8..12].copy_from_slice(&42_i32.to_le_bytes());
        }
        FileType::BtreeOverflowKey => {
            descriptor[0..4].copy_from_slice(&14_i32.to_le_bytes());
            descriptor[4..6].copy_from_slice(&0_i16.to_le_bytes());
            descriptor[8..12].copy_from_slice(&14_i32.to_le_bytes());
            put_oid(
                descriptor,
                12,
                class_oid.page_id.get(),
                class_oid.slot_id.get(),
            );
        }
        _ => panic!("not a class-associated non-heap fixture type"),
    }
}

fn configure_oos_descriptor(page: &mut [u8; IO_PAGE_SIZE]) {
    let descriptor = &mut page[32 + 40..32 + 104];
    descriptor[0..4].copy_from_slice(&7_i32.to_le_bytes());
    descriptor[4..6].copy_from_slice(&0_i16.to_le_bytes());
    descriptor[8..12].copy_from_slice(&8_i32.to_le_bytes());
}

fn slotted_header(user: &mut [u8], slots: i16, records: i16, free_offset: i32) {
    user[0..2].copy_from_slice(&slots.to_le_bytes());
    user[2..4].copy_from_slice(&records.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    let free = i32::try_from(DB_PAGE_SIZE).unwrap() - i32::from(slots) * 4 - free_offset;
    user[8..12].copy_from_slice(&free.to_le_bytes());
    user[12..16].copy_from_slice(&free.to_le_bytes());
    user[16..20].copy_from_slice(&free_offset.to_le_bytes());
}

fn put_slot(user: &mut [u8], slot: usize, offset: u16, length: u16, kind: u8) {
    let word = u32::from(offset) | (u32::from(length) << 14) | (u32::from(kind) << 28);
    let at = DB_PAGE_SIZE - (slot + 1) * 4;
    user[at..at + 4].copy_from_slice(&word.to_le_bytes());
}

fn class_body(name: &[u8], valid_terminator: bool) -> Vec<u8> {
    const TABLE: usize = 18 * 4;
    const FIXED: usize = 88;
    const VARIABLE: usize = TABLE + FIXED;
    let attribute_length = 1 + name.len() + 1;
    let end = (VARIABLE + attribute_length + 3) & !3;
    let mut body = vec![0_u8; end];
    body[0..4].copy_from_slice(&u32::try_from(VARIABLE).unwrap().to_be_bytes());
    for entry in 1..=17 {
        let at = entry * 4;
        body[at..at + 4].copy_from_slice(&u32::try_from(end).unwrap().to_be_bytes());
    }
    body[VARIABLE] = u8::try_from(name.len()).unwrap();
    body[VARIABLE + 1..VARIABLE + 1 + name.len()].copy_from_slice(name);
    body[VARIABLE + 1 + name.len()] = if valid_terminator { 0 } else { b'X' };
    body
}

fn class_record_page(page_id: i32, kind: u8, record: &[u8]) -> [u8; IO_PAGE_SIZE] {
    let mut page = page(page_id, PageType::Heap);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    let record_offset = 72_usize;
    let free_offset = (record_offset + record.len() + 7) & !7;
    slotted_header(user, 2, 2, i32::try_from(free_offset).unwrap());
    user[32..72].fill(0);
    user[record_offset..record_offset + record.len()].copy_from_slice(record);
    put_slot(user, 0, 32, 40, 2);
    put_slot(
        user,
        1,
        u16::try_from(record_offset).unwrap(),
        u16::try_from(record.len()).unwrap(),
        kind,
    );
    page
}

fn inline_class_page(page_id: i32, kind: u8, name: &[u8], terminator: bool) -> [u8; IO_PAGE_SIZE] {
    let mut record = vec![0_u8; 8];
    record.extend_from_slice(&class_body(name, terminator));
    class_record_page(page_id, kind, &record)
}

fn relocation_page_at(page_id: i32, target_page: i32) -> [u8; IO_PAGE_SIZE] {
    let mut target = [0_u8; 8];
    put_oid(&mut target, 0, target_page, CLASS_SLOT);
    class_record_page(page_id, 4, &target)
}

fn relocation_page(target_page: i32) -> [u8; IO_PAGE_SIZE] {
    relocation_page_at(CLASS_PAGE, target_page)
}

fn bigone_page(head_page: i32) -> [u8; IO_PAGE_SIZE] {
    let mut target = [0_u8; 8];
    target[0..4].copy_from_slice(&head_page.to_le_bytes());
    target[4..6].copy_from_slice(&(-1_i16).to_le_bytes());
    target[6..8].copy_from_slice(&0_i16.to_le_bytes());
    class_record_page(CLASS_PAGE, 5, &target)
}

fn overflow_pages(payload: &[u8]) -> ([u8; IO_PAGE_SIZE], [u8; IO_PAGE_SIZE]) {
    let head_capacity = DB_PAGE_SIZE - 12;
    let mut head = page(11, PageType::Overflow);
    let user = &mut head[32..IO_PAGE_SIZE - 8];
    put_vpid(user, 0, 12);
    user[8..12].copy_from_slice(&i32::try_from(payload.len()).unwrap().to_le_bytes());
    user[12..].copy_from_slice(&payload[..head_capacity]);

    let mut tail = page(12, PageType::Overflow);
    let user = &mut tail[32..IO_PAGE_SIZE - 8];
    put_null_vpid(user, 0);
    user[8..8 + payload.len() - head_capacity].copy_from_slice(&payload[head_capacity..]);
    (head, tail)
}

fn heap_header_page(page_id: i32) -> [u8; IO_PAGE_SIZE] {
    let mut page = page(page_id, PageType::Heap);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    slotted_header(user, 1, 1, 1_192);
    user[32..40].fill(0xff);
    put_null_vpid(user, 40);
    put_vpid(user, 48, page_id + 1);
    put_vpid(user, 56, page_id + 1);
    put_null_vpid(user, 64);
    user[72..76].copy_from_slice(&64_i32.to_le_bytes());
    user[76..80].copy_from_slice(&1_i32.to_le_bytes());
    put_slot(user, 0, 32, 1_160, 2);
    page
}

fn fixture(
    codeset: u8,
    name: &[u8],
    record: ClassRecord,
) -> (TestDirectory, volmap::inspection::GraphView) {
    fixture_with_inventory(codeset, name, record, InventoryFixture::Complete)
}

const TRACKED_FILES: &[(i32, FileType)] = &[
    (4, FileType::Heap),
    (7, FileType::Heap),
    (9, FileType::MultipageObjectHeap),
    (13, FileType::MultipageObjectHeap),
    (14, FileType::Btree),
    (15, FileType::BtreeOverflowKey),
    (16, FileType::ExtensibleHash),
    (17, FileType::HashDirectory),
    (18, FileType::Catalog),
    (19, FileType::QueryArea),
    (20, FileType::Oos),
    (21, FileType::Heap),
    (23, FileType::HeapReuseSlots),
    (64, FileType::Heap),
];

fn core_inventory_pages(
    codeset: u8,
    inventory: InventoryFixture,
    class_oid: Oid,
) -> Vec<(i32, [u8; IO_PAGE_SIZE])> {
    let mut bitmap = page(1, PageType::VolumeBitmap);
    bitmap[32..40].copy_from_slice(&3_u64.to_le_bytes());
    let mut tracker = file_header_page(2, FileType::Tracker, 1, (1 << 2) | (1 << 3), Some(3));
    tracker[32 + 40..32 + 52].fill(0);
    let mut root = file_header_page(
        4,
        FileType::Heap,
        3,
        (1 << 4) | (1 << 5) | (1 << 6) | (1 << 10),
        None,
    );
    configure_heap_descriptor(&mut root, 4, 5, None);
    let mut user_heap = file_header_page(7, FileType::Heap, 1, (1 << 7) | (1 << 8), None);
    configure_heap_descriptor(&mut user_heap, 7, 8, Some(class_oid));
    let overflow_type = if inventory == InventoryFixture::HeaderMismatch {
        FileType::Btree
    } else {
        FileType::MultipageObjectHeap
    };
    let mut overflow =
        file_header_page(9, overflow_type, 2, (1 << 9) | (1 << 11) | (1 << 12), None);
    configure_overflow_descriptor(&mut overflow);
    vec![
        (0, volume_header_page(codeset)),
        (1, bitmap),
        (2, tracker),
        (3, tracker_items_page(TRACKED_FILES)),
        (4, root),
        (5, heap_header_page(5)),
        (7, user_heap),
        (8, heap_header_page(8)),
        (9, overflow),
    ]
}

fn associated_file_pages(
    inventory: InventoryFixture,
    class_oid: Oid,
) -> Vec<(i32, [u8; IO_PAGE_SIZE])> {
    let (multipage_user_pages, multipage_bitmap) = if inventory == InventoryFixture::OwnerConflict {
        (1, (1 << 7) | (1 << 13))
    } else {
        (0, 1 << 13)
    };
    let mut multipage = file_header_page(
        13,
        FileType::MultipageObjectHeap,
        multipage_user_pages,
        multipage_bitmap,
        None,
    );
    configure_class_descriptor(&mut multipage, FileType::MultipageObjectHeap, class_oid);
    let mut btree = file_header_page(14, FileType::Btree, 0, 1 << 14, None);
    configure_class_descriptor(&mut btree, FileType::Btree, class_oid);
    let mut btree_overflow = file_header_page(15, FileType::BtreeOverflowKey, 0, 1 << 15, None);
    configure_class_descriptor(&mut btree_overflow, FileType::BtreeOverflowKey, class_oid);
    let mut ehash = file_header_page(16, FileType::ExtensibleHash, 0, 1 << 16, None);
    configure_class_descriptor(&mut ehash, FileType::ExtensibleHash, class_oid);
    let mut hash_directory = file_header_page(17, FileType::HashDirectory, 0, 1 << 17, None);
    configure_class_descriptor(&mut hash_directory, FileType::HashDirectory, class_oid);
    let mut oos = file_header_page(20, FileType::Oos, 0, 1 << 20, None);
    configure_oos_descriptor(&mut oos);
    let mut null_heap = file_header_page(21, FileType::Heap, 1, (1 << 21) | (1 << 22), None);
    configure_heap_descriptor(&mut null_heap, 21, 22, None);
    let mut reuse_heap =
        file_header_page(23, FileType::HeapReuseSlots, 1, (1 << 23) | (1 << 24), None);
    configure_heap_descriptor(&mut reuse_heap, 23, 24, Some(class_oid));
    let mut reserved_heap = file_header_page(64, FileType::Heap, 1, 0b11, None);
    reserved_heap[32 + 232..32 + 236].copy_from_slice(&1_i32.to_le_bytes());
    configure_heap_descriptor(&mut reserved_heap, 64, 65, Some(class_oid));
    let mut pages = vec![
        (13, multipage),
        (14, btree),
        (15, btree_overflow),
        (16, ehash),
        (17, hash_directory),
        (
            18,
            file_header_page(18, FileType::Catalog, 0, 1 << 18, None),
        ),
        (
            19,
            file_header_page(19, FileType::QueryArea, 0, 1 << 19, None),
        ),
        (20, oos),
        (21, null_heap),
        (22, heap_header_page(22)),
        (23, reuse_heap),
        (24, heap_header_page(24)),
        (64, reserved_heap),
        (65, heap_header_page(65)),
    ];
    if inventory == InventoryFixture::MissingHeader {
        pages.retain(|(page_id, _)| *page_id != 14);
    }
    pages
}

fn class_storage_pages(name: &[u8], record: ClassRecord) -> Vec<(i32, [u8; IO_PAGE_SIZE])> {
    let mut overflow_record = vec![0_u8; 8];
    overflow_record.extend_from_slice(&class_body(name, true));
    overflow_record.resize(17_000, 0);
    let (mut overflow_head, mut overflow_tail) = overflow_pages(&overflow_record);
    if record == ClassRecord::BigOneCycle {
        let cycle_total = (DB_PAGE_SIZE - 12) + (DB_PAGE_SIZE - 8) + 1;
        overflow_head[40..44].copy_from_slice(&i32::try_from(cycle_total).unwrap().to_le_bytes());
        put_vpid(&mut overflow_tail[32..IO_PAGE_SIZE - 8], 0, 11);
    }
    let source = match record {
        ClassRecord::Home => inline_class_page(CLASS_PAGE, 2, name, true),
        ClassRecord::NewHome => inline_class_page(CLASS_PAGE, 3, name, true),
        ClassRecord::Relocation | ClassRecord::RelocationChain => relocation_page(TARGET_PAGE),
        ClassRecord::BigOne | ClassRecord::BigOneCycle => bigone_page(11),
        ClassRecord::BigOneWrongOwner => bigone_page(TARGET_PAGE),
        ClassRecord::BrokenTerminator => inline_class_page(CLASS_PAGE, 2, name, false),
        ClassRecord::Dead => class_record_page(CLASS_PAGE, 6, &[0_u8; 8]),
        ClassRecord::Encrypted => {
            let mut page = inline_class_page(CLASS_PAGE, 2, name, true);
            page[15] = 1;
            page
        }
        ClassRecord::MissingPage | ClassRecord::MissingSlot => {
            inline_class_page(CLASS_PAGE, 2, name, true)
        }
        ClassRecord::RelocationCycle => relocation_page(CLASS_PAGE),
        ClassRecord::WrongPage => page(CLASS_PAGE, PageType::Unknown),
    };
    let target = if record == ClassRecord::RelocationChain {
        relocation_page_at(TARGET_PAGE, 11)
    } else {
        inline_class_page(TARGET_PAGE, 3, name, true)
    };
    vec![
        (CLASS_PAGE, source),
        (TARGET_PAGE, target),
        (11, overflow_head),
        (12, overflow_tail),
    ]
}

fn fixture_with_inventory(
    codeset: u8,
    name: &[u8],
    record: ClassRecord,
    inventory: InventoryFixture,
) -> (TestDirectory, volmap::inspection::GraphView) {
    let directory = TestDirectory::new();
    let volume = directory.path().join("fixture");
    let vinf = directory.path().join("fixture_vinf");
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&volume)
        .unwrap();
    file.set_len(64 * 64 * IO_PAGE_SIZE as u64).unwrap();
    let class_oid = record.class_oid();
    let pages = core_inventory_pages(codeset, inventory, class_oid)
        .into_iter()
        .chain(class_storage_pages(name, record))
        .chain(associated_file_pages(inventory, class_oid));
    for (page_id, bytes) in pages {
        file.write_all_at(
            &bytes,
            u64::try_from(page_id).unwrap() * IO_PAGE_SIZE as u64,
        )
        .unwrap();
    }
    drop(file);
    let mut manifest = File::create(&vinf).unwrap();
    writeln!(manifest, "0 {}", volume.display()).unwrap();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let policy = ResourcePolicy::new(4 << 20, 1 << 20, 1, 32, 1 << 20).unwrap();
    let view = Inspection::open(&request, policy, &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
    (directory, view)
}

#[test]
fn selective_file_enrichment_does_not_promote_class_evidence_from_incomplete_inventory() {
    let (_directory, view) = fixture_with_inventory(
        0,
        b"owner.table",
        ClassRecord::Home,
        InventoryFixture::HeaderMismatch,
    );
    let inventory = view
        .overview()
        .coverage
        .into_iter()
        .find(|coverage| coverage.facet == "file-inventory")
        .unwrap();
    assert_eq!(inventory.coverage, volmap::model::Coverage::Partial);

    let user_heap = Vfid::new(VolId::new(0).unwrap(), FileId::new(7).unwrap());
    let enriched = view
        .enrich_file(
            user_heap,
            ResourcePolicy::new(4 << 20, 1 << 20, 1, 32, 1 << 20).unwrap(),
            &CancelToken::new(),
        )
        .unwrap();
    let page = enriched
        .page(Vpid::new(VolId::new(0).unwrap(), PageId::new(8).unwrap()))
        .unwrap();
    let PageFileAssociation::Allocated(file) = &page.file_association else {
        panic!("selectively enriched page lost its allocating file")
    };
    assert_eq!(
        file.class,
        ClassAssociation::Unresolved(ClassAssociationUnresolvedReason::IncompleteInventory)
    );
    let projected = serde_json::to_value(page_projection(page)).unwrap();

    assert_eq!(projected["file_association"]["state"], "allocated");
    assert_eq!(
        projected["file_association"]["file"]["file_type"]["value"],
        "heap"
    );
    assert_eq!(
        projected["file_association"]["file"]["class_oid"]["state"],
        "absent"
    );
    assert_eq!(
        projected["file_association"]["file"]["class_name"]["state"],
        "unresolved"
    );
    assert_eq!(
        projected["file_association"]["file"]["class_name"]["reason"],
        "complete file inventory is required for class attribution"
    );
    assert_eq!(
        projected["file_association"]["file"]["class_name"]["reason_code"],
        "class-association.inventory-incomplete"
    );
}

#[test]
fn conflicting_page_owners_publish_no_partial_file_or_class_association() {
    let (_directory, view) = fixture_with_inventory(
        0,
        b"owner.table",
        ClassRecord::Home,
        InventoryFixture::OwnerConflict,
    );
    let inventory = view
        .overview()
        .coverage
        .into_iter()
        .find(|coverage| coverage.facet == "file-inventory")
        .unwrap();
    assert_eq!(inventory.coverage, volmap::model::Coverage::Partial);
    assert_eq!(inventory.stop_reason, Some("structural"));
    for page_id in [7, 13] {
        let page = view
            .page(Vpid::new(
                VolId::new(0).unwrap(),
                PageId::new(page_id).unwrap(),
            ))
            .unwrap();
        assert_eq!(page.file_association, PageFileAssociation::None);
    }
}

#[test]
fn absent_file_header_publishes_no_partial_file_or_class_association() {
    let (_directory, view) = fixture_with_inventory(
        0,
        b"owner.table",
        ClassRecord::Home,
        InventoryFixture::MissingHeader,
    );
    let inventory = view
        .overview()
        .coverage
        .into_iter()
        .find(|coverage| coverage.facet == "file-inventory")
        .unwrap();
    assert_eq!(inventory.coverage, volmap::model::Coverage::Partial);
    assert_eq!(inventory.stop_reason, Some("structural"));
    for page_id in [7, 14] {
        let page = view
            .page(Vpid::new(
                VolId::new(0).unwrap(),
                PageId::new(page_id).unwrap(),
            ))
            .unwrap();
        assert_eq!(page.file_association, PageFileAssociation::None);
    }
}

fn class_association(view: &volmap::inspection::GraphView) -> (Oid, ClassNameResolution) {
    let page = view
        .page(Vpid::new(VolId::new(0).unwrap(), PageId::new(8).unwrap()))
        .unwrap();
    let PageFileAssociation::Allocated(file) = page.file_association else {
        panic!("user heap page did not retain its allocating file")
    };
    let ClassAssociation::Class { oid, name } = file.class else {
        panic!("user heap file did not retain its class association")
    };
    (oid, name)
}

fn exported_page(document: &serde_json::Value, page_id: i32) -> &serde_json::Value {
    document["data"]["sectors"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|sector| sector["pages"].as_array().unwrap())
        .find(|page| page["vol_id"] == 0 && page["page_id"] == page_id)
        .unwrap_or_else(|| panic!("export omitted page 0:{page_id}"))
}

#[test]
fn complete_inventory_joins_every_scoped_file_family_through_one_shared_class_resolution() {
    let (_directory, view) = fixture(0, b"owner.table", ClassRecord::Home);
    let mut shared_name: Option<Arc<str>> = None;
    for (page_id, expected_type) in [
        (7, FileType::Heap),
        (13, FileType::MultipageObjectHeap),
        (14, FileType::Btree),
        (15, FileType::BtreeOverflowKey),
        (16, FileType::ExtensibleHash),
        (17, FileType::HashDirectory),
        (23, FileType::HeapReuseSlots),
    ] {
        let page = view
            .page(Vpid::new(
                VolId::new(0).unwrap(),
                PageId::new(page_id).unwrap(),
            ))
            .unwrap();
        assert_eq!(page.allocation, PageAllocationClass::Allocated);
        let PageFileAssociation::Allocated(file) = page.file_association else {
            panic!("{expected_type:?} page did not retain its allocating file")
        };
        assert_eq!(file.vfid.file_id.get(), page_id);
        assert_eq!(file.file_type, Some(expected_type));
        let ClassAssociation::Class { oid, name } = file.class else {
            panic!("{expected_type:?} page did not retain its class association")
        };
        assert_eq!(oid.page_id.get(), CLASS_PAGE);
        assert_eq!(oid.slot_id.get(), CLASS_SLOT);
        let ClassNameResolution::Resolved(name) = name else {
            panic!("{expected_type:?} class name was not resolved")
        };
        assert_eq!(name.as_ref(), "owner.table");
        if let Some(shared) = &shared_name {
            assert!(Arc::ptr_eq(shared, &name));
        } else {
            shared_name = Some(name);
        }
    }
}

#[test]
fn page_association_keeps_nonapplicability_reservation_and_ownership_distinct() {
    let (_directory, view) = fixture(0, b"owner.table", ClassRecord::Home);
    let system = view
        .page(Vpid::new(VolId::new(0).unwrap(), PageId::new(0).unwrap()))
        .unwrap();
    assert_eq!(system.allocation, PageAllocationClass::SystemMetadata);
    // This one-sector hostile fixture has conflicting reservations, which is
    // disclosed without promoting any one file or class onto system metadata.
    assert_eq!(system.file_association, PageFileAssociation::MixedClaims);

    for (page_id, expected, expected_code) in [
        (
            18,
            ClassAssociationNotApplicableReason::InternalFile,
            "class-association.internal-file",
        ),
        (
            19,
            ClassAssociationNotApplicableReason::NoSingleClass,
            "class-association.no-single-class",
        ),
        (
            20,
            ClassAssociationNotApplicableReason::DeferredOos,
            "class-association.oos-deferred",
        ),
        (
            21,
            ClassAssociationNotApplicableReason::NullOid,
            "class-association.null-oid",
        ),
    ] {
        let page = view
            .page(Vpid::new(
                VolId::new(0).unwrap(),
                PageId::new(page_id).unwrap(),
            ))
            .unwrap();
        let projected = serde_json::to_value(page_projection(page.clone())).unwrap();
        let PageFileAssociation::Allocated(file) = &page.file_association else {
            panic!("page {page_id} lost its allocating file")
        };
        assert_eq!(file.class, ClassAssociation::NotApplicable(expected));
        assert_eq!(
            projected["file_association"]["file"]["class_name"]["state"],
            "not-applicable"
        );
        assert_eq!(
            projected["file_association"]["file"]["class_name"]["reason"],
            expected.message()
        );
        assert_eq!(
            projected["file_association"]["file"]["class_name"]["reason_code"],
            expected_code
        );
    }

    let reserved = view
        .page(Vpid::new(VolId::new(0).unwrap(), PageId::new(66).unwrap()))
        .unwrap();
    assert_eq!(
        reserved.allocation,
        PageAllocationClass::ReservedUnallocated
    );
    let PageFileAssociation::ReservedFor(file) = reserved.file_association else {
        panic!("reserved-unallocated page was not kept distinct from allocation")
    };
    assert_eq!(file.vfid.file_id.get(), 64);

    let mixed = view
        .page(Vpid::new(VolId::new(0).unwrap(), PageId::new(25).unwrap()))
        .unwrap();
    assert_eq!(mixed.allocation, PageAllocationClass::ReservedUnallocated);
    assert_eq!(mixed.file_association, PageFileAssociation::MixedClaims);

    let unreserved = view
        .page(Vpid::new(VolId::new(0).unwrap(), PageId::new(128).unwrap()))
        .unwrap();
    assert_eq!(unreserved.allocation, PageAllocationClass::Unreserved);
    assert_eq!(unreserved.file_association, PageFileAssociation::None);
}

#[test]
fn html_export_escapes_a_resolved_class_name_without_changing_its_json_value() {
    const DATA_START: &str = "<script id=\"volmap-data\" type=\"application/json\">";
    const DATA_END: &str = "</script><script>";
    let stored_name = format!(r#"dba.고객</script><tag>&"quoted"\path{}"#, "길".repeat(48));

    let (directory, view) = fixture(5, stored_name.as_bytes(), ClassRecord::Home);
    let output = directory.path().join("association.html");
    export_html(&view, &output, 8 * 1024 * 1024).unwrap();
    let html = std::fs::read_to_string(output).unwrap();
    let json = html
        .split_once(DATA_START)
        .unwrap()
        .1
        .split_once(DATA_END)
        .unwrap()
        .0;

    assert!(!json.contains(&stored_name));
    assert!(json.contains(r"dba.고객\u003c/script\u003e\u003ctag\u003e\u0026"));
    for label in ["File", "File role", "Class OID", "Class/table"] {
        assert!(html.contains(label), "export renderer omitted {label}");
    }
    assert!(html.contains("overflowWrap='anywhere'"));
    assert!(html.contains("fileAssociationRows(p.file_association)"));
    assert!(html.contains("textContent=text"));
    let document: serde_json::Value = serde_json::from_str(json).unwrap();
    let page = exported_page(&document, 7);
    assert_eq!(
        page["file_association"]["file"]["class_name"]["value"],
        stored_name
    );

    for (page_id, reason_code) in [
        (18, "class-association.internal-file"),
        (19, "class-association.no-single-class"),
        (20, "class-association.oos-deferred"),
        (21, "class-association.null-oid"),
    ] {
        let association = &exported_page(&document, page_id)["file_association"];
        assert_eq!(association["state"], "allocated");
        assert_eq!(association["file"]["class_oid"]["state"], "absent");
        assert_eq!(association["file"]["class_name"]["state"], "not-applicable");
        assert_eq!(
            association["file"]["class_name"]["reason_code"],
            reason_code
        );
    }
    assert_eq!(
        exported_page(&document, 25)["file_association"]["state"],
        "mixed-claims"
    );
    assert_eq!(
        exported_page(&document, 128)["file_association"]["state"],
        "none"
    );
}

#[test]
fn html_export_retains_numeric_class_identity_when_name_resolution_fails() {
    const DATA_START: &str = "<script id=\"volmap-data\" type=\"application/json\">";
    const DATA_END: &str = "</script><script>";
    let (directory, view) = fixture(0, b"broken", ClassRecord::BrokenTerminator);
    let output = directory.path().join("unresolved-association.html");
    export_html(&view, &output, 8 * 1024 * 1024).unwrap();
    let html = std::fs::read_to_string(output).unwrap();
    let document: serde_json::Value = serde_json::from_str(
        html.split_once(DATA_START)
            .unwrap()
            .1
            .split_once(DATA_END)
            .unwrap()
            .0,
    )
    .unwrap();
    let file = &exported_page(&document, 7)["file_association"]["file"];

    assert_eq!(file["class_oid"]["state"], "present");
    assert_eq!(file["class_oid"]["oid"]["vol_id"], 0);
    assert_eq!(file["class_oid"]["oid"]["page_id"], CLASS_PAGE);
    assert_eq!(file["class_oid"]["oid"]["slot_id"], CLASS_SLOT);
    assert_eq!(file["class_name"]["state"], "unresolved");
    assert_eq!(file["class_name"]["reason_code"], "class.name.terminator");
}

#[test]
fn inspection_resolves_home_newhome_and_relocated_class_records() {
    for record in [
        ClassRecord::Home,
        ClassRecord::NewHome,
        ClassRecord::Relocation,
    ] {
        let (_directory, view) = fixture(0, b"owner.table", record);
        let (oid, resolution) = class_association(&view);
        assert_eq!(oid.page_id.get(), CLASS_PAGE);
        assert_eq!(oid.slot_id.get(), CLASS_SLOT);
        assert_eq!(
            resolution,
            ClassNameResolution::Resolved("owner.table".into())
        );
    }
}

#[test]
fn inspection_resolves_a_bigone_class_record_from_its_owned_overflow_file() {
    let (_directory, view) = fixture(0, b"owner.large_table", ClassRecord::BigOne);
    let (oid, resolution) = class_association(&view);
    assert_eq!(oid.page_id.get(), CLASS_PAGE);
    assert_eq!(oid.slot_id.get(), CLASS_SLOT);
    assert_eq!(
        resolution,
        ClassNameResolution::Resolved("owner.large_table".into())
    );
}

#[test]
fn inspection_decodes_every_supported_database_codeset() {
    for (codeset, stored, expected) in [
        (0, b"table_name".as_slice(), "table_name"),
        (3, b"caf\xe9".as_slice(), "café"),
        (4, [0xc5, 0xd7, 0xc0, 0xcc, 0xba, 0xed].as_slice(), "테이블"),
        (5, "테이블".as_bytes(), "테이블"),
    ] {
        let (_directory, view) = fixture(codeset, stored, ClassRecord::Home);
        assert_eq!(
            class_association(&view).1,
            ClassNameResolution::Resolved(expected.into())
        );
    }
}

#[test]
fn unresolved_class_name_retains_the_original_numeric_oid() {
    let (_directory, view) = fixture(0, b"broken", ClassRecord::BrokenTerminator);
    let (oid, resolution) = class_association(&view);
    assert_eq!(oid.page_id.get(), CLASS_PAGE);
    assert_eq!(oid.slot_id.get(), CLASS_SLOT);
    assert_eq!(
        resolution,
        ClassNameResolution::Unresolved(ClassNameUnresolvedReason::InvalidFormat(
            "class.name.terminator"
        ))
    );
}

#[test]
fn every_failed_resolution_keeps_the_exact_descriptor_oid() {
    for case in RESOLUTION_FAILURE_CASES {
        let (_directory, view) = fixture(case.codeset, case.name, case.record);
        let (oid, resolution) = class_association(&view);
        let expected_oid = case.record.class_oid();
        assert_eq!(oid, expected_oid);
        assert_eq!(resolution, ClassNameResolution::Unresolved(case.reason));
        let page = view
            .page(Vpid::new(VolId::new(0).unwrap(), PageId::new(7).unwrap()))
            .unwrap();
        let projected = serde_json::to_value(page_projection(page)).unwrap();
        assert_eq!(
            projected["file_association"]["file"]["class_name"]["reason_code"],
            case.reason_code
        );
        assert_eq!(
            projected["file_association"]["file"]["class_name"]["reason"],
            case.message
        );
    }
}
