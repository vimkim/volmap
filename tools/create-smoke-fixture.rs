//! Create the deterministic sparse snapshot used by distribution smoke tests.

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::path::Path;

const IO_PAGE_SIZE: usize = 16_384;
const DB_PAGE_SIZE: usize = 16_344;
const PHYSICAL_PAGES: u64 = 64 * 64;
const PAGE_UNKNOWN: u8 = 0;
const PAGE_HEAP: u8 = 2;
const PAGE_VOLUME_HEADER: u8 = 3;
const PAGE_VOLUME_BITMAP: u8 = 4;
const PAGE_OOS: u8 = 8;

fn envelope_page(page_id: i32, page_type: u8) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    let lsa = u64::try_from(page_id)
        .expect("nonnegative page")
        .to_le_bytes();
    page[0..8].copy_from_slice(&lsa);
    page[8..12].copy_from_slice(&page_id.to_le_bytes());
    page[12..14].copy_from_slice(&0_i16.to_le_bytes());
    page[14] = page_type;
    page[IO_PAGE_SIZE - 8..].copy_from_slice(&lsa);
    page
}

fn volume_header_page() -> [u8; IO_PAGE_SIZE] {
    let mut page = envelope_page(0, PAGE_VOLUME_HEADER);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
    user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
    user[28..30].copy_from_slice(&0_i16.to_le_bytes());
    user[32..36].copy_from_slice(&0_i32.to_le_bytes());
    user[36..40].copy_from_slice(&0_i32.to_le_bytes());
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
    user[126..128].copy_from_slice(&0_i16.to_le_bytes());
    user[128..130].copy_from_slice(&1_i16.to_le_bytes());
    user[130..132].copy_from_slice(&2_i16.to_le_bytes());
    page
}

fn oos_chunk_page(page_id: i32, index: i32, next_page: Option<i32>) -> [u8; IO_PAGE_SIZE] {
    let mut page = envelope_page(page_id, PAGE_OOS);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    let distribution_fixture = page_id == 20;
    user[0..2].copy_from_slice(&(if distribution_fixture { 4_i16 } else { 1 }).to_le_bytes());
    user[2..4].copy_from_slice(&(if distribution_fixture { 2_i16 } else { 1 }).to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    user[8..12].copy_from_slice(
        &(if distribution_fixture { 16_240_i32 } else { 16_280 }).to_le_bytes(),
    );
    user[12..16].copy_from_slice(
        &(if distribution_fixture { 16_216_i32 } else { 16_280 }).to_le_bytes(),
    );
    user[16..20]
        .copy_from_slice(&(if distribution_fixture { 112_i32 } else { 56 }).to_le_bytes());
    user[32..36].copy_from_slice(&8_i32.to_le_bytes());
    user[36..40].copy_from_slice(&index.to_le_bytes());
    let (next_page, next_slot, next_volume): (i32, i16, i16) =
        next_page.map_or((-1, -1, -1), |page| (page, 0, 0));
    user[40..44].copy_from_slice(&next_page.to_le_bytes());
    user[44..46].copy_from_slice(&next_slot.to_le_bytes());
    user[46..48].copy_from_slice(&next_volume.to_le_bytes());
    user[48..52].copy_from_slice(b"safe");
    let slot = 32_u32 | (20_u32 << 14) | (2_u32 << 28);
    user[DB_PAGE_SIZE - 4..DB_PAGE_SIZE].copy_from_slice(&slot.to_le_bytes());
    if distribution_fixture {
        let second = 80_u32 | (32_u32 << 14) | (3_u32 << 28);
        let unallocated = 9_u32 << 28;
        let deleted = (48_u32 << 14) | (6_u32 << 28);
        user[DB_PAGE_SIZE - 8..DB_PAGE_SIZE - 4].copy_from_slice(&second.to_le_bytes());
        user[DB_PAGE_SIZE - 12..DB_PAGE_SIZE - 8].copy_from_slice(&unallocated.to_le_bytes());
        user[DB_PAGE_SIZE - 16..DB_PAGE_SIZE - 12].copy_from_slice(&deleted.to_le_bytes());
    }
    page
}

fn run(output: &Path) -> Result<(), String> {
    if !output.is_dir() {
        return Err("output must be an existing directory".to_owned());
    }
    let volume_path = output.join("fixture");
    let vinf_path = output.join("fixture_vinf");
    let volume = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&volume_path)
        .map_err(|error| format!("create volume: {error}"))?;
    volume
        .set_len(PHYSICAL_PAGES * IO_PAGE_SIZE as u64)
        .map_err(|error| format!("size volume: {error}"))?;
    volume
        .write_all_at(&volume_header_page(), 0)
        .map_err(|error| format!("write volume header: {error}"))?;
    let mut bitmap = envelope_page(1, PAGE_VOLUME_BITMAP);
    bitmap[32..40].copy_from_slice(&1_u64.to_le_bytes());
    volume
        .write_all_at(&bitmap, IO_PAGE_SIZE as u64)
        .map_err(|error| format!("write bitmap: {error}"))?;
    for page_id in 2_i32..64 {
        let page = match page_id {
            10 => {
                let mut page = envelope_page(page_id, PAGE_HEAP);
                let user = &mut page[32..IO_PAGE_SIZE - 8];
                user[4..6].copy_from_slice(&1_i16.to_le_bytes());
                user[6..8].copy_from_slice(&8_u16.to_le_bytes());
                user[8..12].copy_from_slice(&16_312_i32.to_le_bytes());
                user[12..16].copy_from_slice(&16_312_i32.to_le_bytes());
                user[16..20].copy_from_slice(&32_i32.to_le_bytes());
                page
            }
            20 => oos_chunk_page(20, 0, Some(21)),
            21 => oos_chunk_page(21, 1, None),
            _ => envelope_page(page_id, PAGE_UNKNOWN),
        };
        volume
            .write_all_at(&page, u64::try_from(page_id).unwrap() * IO_PAGE_SIZE as u64)
            .map_err(|error| format!("write page: {error}"))?;
    }
    volume
        .sync_all()
        .map_err(|error| format!("sync volume: {error}"))?;

    let mut vinf = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(vinf_path)
        .map_err(|error| format!("create manifest: {error}"))?;
    vinf.write_all(b"0 /snapshot/fixture\n")
        .map_err(|error| format!("write manifest: {error}"))?;
    vinf.sync_all()
        .map_err(|error| format!("sync manifest: {error}"))?;
    Ok(())
}

fn main() {
    let arguments = env::args_os().collect::<Vec<_>>();
    let [_, output] = arguments.as_slice() else {
        eprintln!("usage: create-smoke-fixture OUTPUT_DIRECTORY");
        std::process::exit(2);
    };
    if let Err(error) = run(Path::new(output)) {
        eprintln!("smoke fixture creation failed: {error}");
        std::process::exit(2);
    }
}
