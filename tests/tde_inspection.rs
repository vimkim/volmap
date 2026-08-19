use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{FileExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aes::Aes256;
use cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;
use sha2::{Digest, Sha256};
use volmap::format::{DB_PAGE_SIZE, IO_PAGE_SIZE, PageType};
use volmap::inspection::{CancelToken, Inspection, OpenRequest, ResourcePolicy, RevisionSelector};
use volmap::model::{Availability, PageId, TdeInspectionState, VolId, Vpid};
use volmap::source::InputSpec;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
const MASTER_KEY: [u8; 32] = [0x35; 32];
const PERMANENT_KEY: [u8; 32] = [0xa7; 32];
const CREATED_TIME: i64 = 123;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "volmap-tde-inspection-test-{}-{sequence}",
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

fn envelope_page(page_id: i32, page_type: PageType) -> [u8; IO_PAGE_SIZE] {
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
    let mut page = envelope_page(0, PageType::VolumeHeader);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
    user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
    user[28..30].copy_from_slice(&0_i16.to_le_bytes());
    user[40..44].copy_from_slice(&64_i32.to_le_bytes());
    user[44..48].copy_from_slice(&64_i32.to_le_bytes());
    user[48..52].copy_from_slice(&64_i32.to_le_bytes());
    user[52..56].copy_from_slice(&(-1_i32).to_le_bytes());
    user[56..60].copy_from_slice(&1_i32.to_le_bytes());
    user[60..64].copy_from_slice(&1_i32.to_le_bytes());
    user[64..68].copy_from_slice(&1_i32.to_le_bytes());
    // Volume zero identifies the special boot heap: VFID 2|0, header page 2.
    user[96..100].copy_from_slice(&2_i32.to_le_bytes());
    user[100..102].copy_from_slice(&0_i16.to_le_bytes());
    user[104..108].copy_from_slice(&2_i32.to_le_bytes());
    user[124..126].copy_from_slice(&(-1_i16).to_le_bytes());
    user[126..128].copy_from_slice(&0_i16.to_le_bytes());
    user[128..130].copy_from_slice(&1_i16.to_le_bytes());
    user[130..132].copy_from_slice(&2_i16.to_le_bytes());
    user[132..135].copy_from_slice(b"\0\0\0");
    page
}

fn hfid(record: &mut [u8], offset: usize, file_id: i32, header_page_id: i32) {
    record[offset..offset + 4].copy_from_slice(&file_id.to_le_bytes());
    record[offset + 4..offset + 6].copy_from_slice(&0_i16.to_le_bytes());
    record[offset + 8..offset + 12].copy_from_slice(&header_page_id.to_le_bytes());
}

fn boot_record() -> [u8; 136] {
    let mut record = [0_u8; 136];
    hfid(&mut record, 8, 2, 2);
    hfid(&mut record, 124, 3, 3);
    record
}

fn apply_aes_ctr(key: &[u8; 32], nonce: &[u8; 16], bytes: &mut [u8]) {
    let mut cipher = Ctr128BE::<Aes256>::new_from_slices(key, nonce).unwrap();
    cipher.apply_keystream(bytes);
}

fn key_info_record() -> [u8; 156] {
    let mut record = [0_u8; 156];
    record[4..8].copy_from_slice(&0_i32.to_le_bytes());
    record[12..20].copy_from_slice(&CREATED_TIME.to_le_bytes());
    record[28..60].copy_from_slice(&Sha256::digest(MASTER_KEY));
    let mut encrypted = PERMANENT_KEY;
    apply_aes_ctr(&MASTER_KEY, &[0_u8; 16], &mut encrypted);
    record[60..92].copy_from_slice(&encrypted);
    record
}

fn special_heap_page<const N: usize>(page_id: i32, record: &[u8; N]) -> [u8; IO_PAGE_SIZE] {
    const HEAP_HEADER_SIZE: usize = 1_160;
    const RECORD_OFFSET: usize = 1_200;
    let mut page = envelope_page(page_id, PageType::Heap);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[0..2].copy_from_slice(&2_i16.to_le_bytes());
    user[2..4].copy_from_slice(&2_i16.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    let free_area = RECORD_OFFSET + N;
    let free = DB_PAGE_SIZE - 8 - free_area;
    user[8..12].copy_from_slice(&i32::try_from(free).unwrap().to_le_bytes());
    user[12..16].copy_from_slice(&i32::try_from(free).unwrap().to_le_bytes());
    user[16..20].copy_from_slice(&i32::try_from(free_area).unwrap().to_le_bytes());
    let header = &mut user[32..32 + HEAP_HEADER_SIZE];
    header[0..4].copy_from_slice(&(-1_i32).to_le_bytes());
    header[4..6].copy_from_slice(&(-1_i16).to_le_bytes());
    header[6..8].copy_from_slice(&(-1_i16).to_le_bytes());
    header[8..12].copy_from_slice(&(-1_i32).to_le_bytes());
    header[12..14].copy_from_slice(&(-1_i16).to_le_bytes());
    header[16..20].copy_from_slice(&(-1_i32).to_le_bytes());
    header[20..22].copy_from_slice(&(-1_i16).to_le_bytes());
    header[24..28].copy_from_slice(&page_id.to_le_bytes());
    header[28..30].copy_from_slice(&0_i16.to_le_bytes());
    header[32..36].copy_from_slice(&(-1_i32).to_le_bytes());
    header[36..38].copy_from_slice(&(-1_i16).to_le_bytes());
    user[RECORD_OFFSET..RECORD_OFFSET + N].copy_from_slice(record);
    let slot_zero = 32_u32 | (1_160_u32 << 14) | (2_u32 << 28);
    let slot_one =
        u32::try_from(RECORD_OFFSET).unwrap() | (u32::try_from(N).unwrap() << 14) | (2_u32 << 28);
    user[DB_PAGE_SIZE - 4..DB_PAGE_SIZE].copy_from_slice(&slot_zero.to_le_bytes());
    user[DB_PAGE_SIZE - 8..DB_PAGE_SIZE - 4].copy_from_slice(&slot_one.to_le_bytes());
    page
}

fn encrypted_heap_page(page_id: i32, valid: bool) -> [u8; IO_PAGE_SIZE] {
    let mut page = envelope_page(page_id, PageType::Heap);
    page[15] = 0x01;
    page[24..32].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    let mut nonce = [0_u8; 16];
    nonce[..8].copy_from_slice(&page[24..32]);
    let user = &mut page[32..32 + DB_PAGE_SIZE];
    user[4..6].copy_from_slice(&i16::from(valid).to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    user[8..12].copy_from_slice(&16_312_i32.to_le_bytes());
    user[12..16].copy_from_slice(&16_312_i32.to_le_bytes());
    user[16..20].copy_from_slice(&32_i32.to_le_bytes());
    apply_aes_ctr(&PERMANENT_KEY, &nonce, user);
    page
}

fn fixture(valid_encrypted_page: bool) -> (TestDirectory, PathBuf, PathBuf) {
    let directory = TestDirectory::new();
    let volume = directory.path().join("fixture");
    let vinf = directory.path().join("fixture_vinf");
    let key_file = directory.path().join("fixture_keys");
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&volume)
        .unwrap();
    file.set_len(64 * 64 * IO_PAGE_SIZE as u64).unwrap();
    file.write_all_at(&volume_header_page(), 0).unwrap();
    let mut bitmap = envelope_page(1, PageType::VolumeBitmap);
    bitmap[32..40].copy_from_slice(&1_u64.to_le_bytes());
    file.write_all_at(&bitmap, IO_PAGE_SIZE as u64).unwrap();
    for page_id in 2_i32..64 {
        let page = match page_id {
            2 => special_heap_page(page_id, &boot_record()),
            3 => special_heap_page(page_id, &key_info_record()),
            10 => encrypted_heap_page(page_id, valid_encrypted_page),
            _ => envelope_page(page_id, PageType::Unknown),
        };
        file.write_all_at(&page, u64::try_from(page_id).unwrap() * IO_PAGE_SIZE as u64)
            .unwrap();
    }
    drop(file);
    let mut manifest = File::create(&vinf).unwrap();
    writeln!(manifest, "0 {}", volume.display()).unwrap();
    let mut keys = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&key_file)
        .unwrap();
    keys.write_all(b"CUBRID/Keys\0\0\0\0\0\0\0\0\0\0\0\0\0\0")
        .unwrap();
    keys.write_all(&CREATED_TIME.to_le_bytes()).unwrap();
    keys.write_all(&MASTER_KEY).unwrap();
    (directory, vinf, key_file)
}

fn policy() -> ResourcePolicy {
    ResourcePolicy::new(4 * 1024 * 1024, 1024 * 1024, 1, 32, 1024 * 1024).unwrap()
}

#[test]
fn explicit_key_file_bootstraps_and_enriches_an_encrypted_page() {
    let (_directory, vinf, key_file) = fixture(true);
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: Some(key_file),
        spill_directory: None,
    };
    let view = Inspection::open(&request, policy(), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
    let encrypted = Vpid::new(VolId::new(0).unwrap(), PageId::new(10).unwrap());
    let page = view.page(encrypted).unwrap();
    assert_eq!(page.availability, Availability::Available);
    assert_eq!(page.tde_state, TdeInspectionState::Decrypted);
    let enriched = view
        .enrich_page(encrypted, policy(), &CancelToken::new())
        .unwrap();
    let deep = enriched.deep_page(encrypted).unwrap();
    assert!(deep.diagnostic_rule.is_none());
    assert!(deep.slotted.unwrap().slots().is_empty());
}

#[test]
fn invalid_decrypted_structure_is_not_retried_as_ciphertext() {
    let (_directory, vinf, key_file) = fixture(false);
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: Some(key_file),
        spill_directory: None,
    };
    let view = Inspection::open(&request, policy(), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
    let encrypted = Vpid::new(VolId::new(0).unwrap(), PageId::new(10).unwrap());
    let failed = view
        .enrich_page(encrypted, policy(), &CancelToken::new())
        .unwrap();
    assert_eq!(
        failed.page(encrypted).unwrap().tde_state,
        TdeInspectionState::DecryptedInvalid
    );
    assert!(failed.overview().diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "tde.decrypted_invalid" && diagnostic.rule == "slotted.header.anchor"
    }));
}

#[test]
fn insecure_key_permissions_are_reported_without_disclosing_the_path() {
    let (_directory, vinf, key_file) = fixture(true);
    std::fs::set_permissions(&key_file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: Some(key_file.clone()),
        spill_directory: None,
    };
    let overview = Inspection::open(&request, policy(), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap()
        .overview();
    let warning = overview
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "tde.key_file.insecure_permissions")
        .unwrap();
    assert_eq!(warning.severity, "warning");
    assert!(
        !warning
            .subject
            .contains(key_file.to_string_lossy().as_ref())
    );
}

#[test]
fn key_failures_do_not_disclose_the_explicit_path() {
    let (_directory, vinf, key_file) = fixture(true);
    let mut file = OpenOptions::new().write(true).open(&key_file).unwrap();
    file.write_all(b"not-a-key").unwrap();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: Some(key_file.clone()),
        spill_directory: None,
    };
    let error = Inspection::open(&request, policy(), &CancelToken::new(), None).unwrap_err();
    assert!(
        !error
            .to_string()
            .contains(key_file.to_string_lossy().as_ref())
    );
}
