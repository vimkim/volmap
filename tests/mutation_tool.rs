use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "volmap-mutation-tool-{}-{sequence}",
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

fn build_tool(directory: &TestDirectory) -> PathBuf {
    let binary = directory.path().join("fixture-mutate");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let status = Command::new(rustc)
        .args([
            "--edition=2024",
            "-Dwarnings",
            "tools/fixture-mutate.rs",
            "-o",
        ])
        .arg(&binary)
        .status()
        .unwrap();
    assert!(status.success());
    binary
}

#[test]
fn mutation_tool_changes_exactly_one_verified_range_and_refuses_overwrite() {
    let directory = TestDirectory::new();
    let tool = build_tool(&directory);
    let input = directory.path().join("input.bin");
    let output = directory.path().join("output.bin");
    let mut file = File::create(&input).unwrap();
    file.write_all(&[0, 1, 2, 3, 4, 5]).unwrap();
    drop(file);

    let result = Command::new(&tool)
        .args([&input, &output])
        .args(["2", "0203", "aabb"])
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(std::fs::read(&output).unwrap(), [0, 1, 0xaa, 0xbb, 4, 5]);
    assert_eq!(
        std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let before = std::fs::read(&output).unwrap();
    let overwrite = Command::new(&tool)
        .args([&input, &output])
        .args(["2", "0203", "ccdd"])
        .output()
        .unwrap();
    assert_eq!(overwrite.status.code(), Some(2));
    assert_eq!(std::fs::read(&output).unwrap(), before);
}

#[test]
fn mutation_tool_fails_closed_on_wrong_base_or_range() {
    let directory = TestDirectory::new();
    let tool = build_tool(&directory);
    let input = directory.path().join("input.bin");
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&input)
        .unwrap()
        .write_all(&[0, 1, 2, 3])
        .unwrap();

    for (name, arguments) in [
        ("wrong.bin", ["1", "ff", "aa"]),
        ("unequal.bin", ["1", "01", "aabb"]),
        ("outside.bin", ["4", "00", "aa"]),
    ] {
        let output = directory.path().join(name);
        let result = Command::new(&tool)
            .args([&input, &output])
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(!output.exists());
    }
}
