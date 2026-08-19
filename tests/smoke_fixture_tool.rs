use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use volmap::inspection::{CancelToken, Inspection, OpenRequest, ResourcePolicy, RevisionSelector};
use volmap::source::InputSpec;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "volmap-smoke-fixture-tool-{}-{sequence}",
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

#[test]
fn distribution_smoke_fixture_is_sparse_and_inspectable() {
    let directory = TestDirectory::new();
    let binary = directory.path().join("create-smoke-fixture");
    let snapshot = directory.path().join("snapshot");
    std::fs::create_dir(&snapshot).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    assert!(
        Command::new(rustc)
            .args([
                "--edition=2024",
                "-Dwarnings",
                "tools/create-smoke-fixture.rs",
                "-o",
            ])
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(binary)
            .arg(&snapshot)
            .status()
            .unwrap()
            .success()
    );

    let volume = std::fs::metadata(snapshot.join("fixture")).unwrap();
    assert_eq!(volume.len(), 64 * 1024 * 1024);
    assert!(volume.blocks() * 512 < volume.len());
    let inspection = Inspection::open(
        &OpenRequest {
            input: InputSpec::Vinf {
                path: snapshot.join("fixture_vinf"),
                volume_root: Some(snapshot),
            },
            tde_keys_file: None,
            spill_directory: None,
        },
        ResourcePolicy::new(4 * 1024 * 1024, 1024 * 1024, 1, 32, 1024 * 1024).unwrap(),
        &CancelToken::new(),
        None,
    )
    .unwrap();
    let overview = inspection
        .view(RevisionSelector::Latest)
        .unwrap()
        .overview();
    assert_eq!(overview.physical_page_count, 4096);
    assert_eq!(overview.inspected_page_envelopes, 64);
    assert!(overview.diagnostics.is_empty());
}
