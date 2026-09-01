use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use volmap::format::{IO_PAGE_SIZE, PageType};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "volmap-cli-contract-{}-{sequence}",
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

fn fixture() -> (TestDirectory, PathBuf) {
    let directory = TestDirectory::new();
    let volume = directory.path().join("synthetic-volume");
    let vinf = directory.path().join("synthetic_vinf");
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
        file.write_all_at(
            &envelope_page(page_id, PageType::Unknown),
            u64::try_from(page_id).unwrap() * IO_PAGE_SIZE as u64,
        )
        .unwrap();
    }
    drop(file);
    let mut manifest = File::create(&vinf).unwrap();
    writeln!(manifest, "0 {}", volume.display()).unwrap();
    (directory, vinf)
}

fn volmap(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_volmap"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn help_and_usage_are_stable_and_do_not_scan() {
    let help = volmap(&["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("Read-only CUBRID volume inspector"));
    assert!(help.stderr.is_empty());

    let serve_help = volmap(&["serve", "--help"]);
    assert!(serve_help.status.success());
    let serve_help = String::from_utf8(serve_help.stdout).unwrap();
    assert!(serve_help.contains("--listen"));
    assert!(!serve_help.contains("--allow-remote-http"));
    assert!(!serve_help.contains("--external-origin"));
    assert!(!serve_help.contains("--token-file"));

    let bare = volmap(&[]);
    assert_eq!(bare.status.code(), Some(2));
    assert!(bare.stdout.is_empty());
    assert!(String::from_utf8_lossy(&bare.stderr).contains("Usage:"));

    let invalid = volmap(&[
        "inspect",
        "--vinf",
        "/path/that/must/not/be/opened",
        "page:0:01",
    ]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid canonical page identifier"));
}

#[test]
fn json_document_is_revision_pinned_and_path_free() {
    let (_directory, vinf) = fixture();
    let output = volmap(&[
        "summary",
        "--vinf",
        vinf.to_str().unwrap(),
        "--format",
        "json",
        "--progress",
        "never",
    ]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["schema"], "volmap.inspection");
    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["document_type"], "result");
    assert_eq!(document["snapshot"]["revision"], "0");
    assert_eq!(document["outcome"], "success-limited");
    assert!(document["coverage"].as_array().unwrap().len() >= 4);
    let output_text = String::from_utf8(output.stdout).unwrap();
    assert!(!output_text.contains(vinf.to_str().unwrap()));
    assert!(!output_text.contains("synthetic-volume"));
}

#[test]
fn schema_one_page_json_adds_the_tagged_association_without_changing_prior_fields() {
    let (_directory, vinf) = fixture();
    let arguments = [
        "inspect",
        "--vinf",
        vinf.to_str().unwrap(),
        "page:0:2",
        "--format",
        "json",
        "--progress",
        "never",
    ];
    let first = volmap(&arguments);
    let second = volmap(&arguments);

    assert_eq!(first.status.code(), Some(0));
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    let document: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["data"]["kind"], "inspect-page");
    let page = document["data"]["page"].as_object().unwrap();
    assert_eq!(
        page["file_association"],
        serde_json::json!({ "state": "none" })
    );
    assert_eq!(
        page.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "allocation",
            "availability",
            "bytes",
            "detail_support",
            "diagnostic",
            "file_association",
            "lsa_word",
            "occupancy",
            "page_id",
            "page_type",
            "sector_id",
            "tde_state",
            "vol_id",
        ]
    );
}

#[test]
fn jsonl_has_typed_ordered_records_and_a_completion_frame() {
    let (_directory, vinf) = fixture();
    let output = volmap(&[
        "map",
        "--vinf",
        vinf.to_str().unwrap(),
        "sector:0:0",
        "--format",
        "jsonl",
        "--progress",
        "never",
    ]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let records = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records.first().unwrap()["record_type"], "header");
    assert_eq!(records.last().unwrap()["record_type"], "completion");
    assert!(
        records
            .iter()
            .any(|record| record["record_type"] == "volume")
    );
    assert!(
        records
            .iter()
            .any(|record| record["record_type"] == "sector")
    );
    assert!(
        records
            .iter()
            .any(|record| record["record_type"] == "coverage")
    );
    let snapshot = records[0]["snapshot_id"].clone();
    for (sequence, record) in records.iter().enumerate() {
        assert_eq!(record["schema"], "volmap.inspection");
        assert_eq!(record["sequence"], sequence.to_string());
        assert_eq!(record["snapshot_id"], snapshot);
        assert_eq!(record["revision"], "0");
    }
    let output_text = String::from_utf8(output.stdout).unwrap();
    assert!(!output_text.contains(vinf.to_str().unwrap()));
}

#[test]
fn absent_entities_use_machine_command_error_documents() {
    let (_directory, vinf) = fixture();
    for format in ["json", "jsonl"] {
        let output = volmap(&[
            "inspect",
            "--vinf",
            vinf.to_str().unwrap(),
            "page:0:4096",
            "--format",
            format,
            "--progress",
            "never",
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stderr.is_empty());
        if format == "json" {
            let document = serde_json::from_slice::<Value>(&output.stdout).unwrap();
            assert_eq!(document["document_type"], "command-error");
            assert_eq!(document["error"]["code"], "entity-not-found");
            assert_eq!(document["error"]["selector"], "page:0:4096");
        } else {
            let records = output
                .stdout
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .map(|line| serde_json::from_slice::<Value>(line).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(records.first().unwrap()["record_type"], "header");
            assert_eq!(records[1]["record_type"], "command-error");
            assert_eq!(records[1]["data"]["code"], "entity-not-found");
            assert_eq!(records.last().unwrap()["record_type"], "completion");
        }
    }
}

#[test]
fn broken_stdout_pipe_is_quiet_and_uses_shell_status() {
    let (_directory, vinf) = fixture();
    let mut child = Command::new(env!("CARGO_BIN_EXE_volmap"))
        .args([
            "map",
            "--vinf",
            vinf.to_str().unwrap(),
            "--format",
            "jsonl",
            "--progress",
            "never",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(141));
    assert!(output.stderr.is_empty());
}
