use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use volmap::diagnostics::InspectionOutcome;
use volmap::export::{ExportError, export_html};
use volmap::format::{IO_PAGE_SIZE, PageType};
use volmap::inspection::{CancelToken, Inspection, OpenRequest, ResourcePolicy, RevisionSelector};
use volmap::model::{Availability, Oid, PageId, SectorId, SlotId, VolId, Vpid};
use volmap::projection::{DataProjection, result_document, summary_projection};
use volmap::source::InputSpec;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "volmap-inspection-test-{}-{sequence}",
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
    user[132..135].copy_from_slice(b"\0\0\0");
    page
}

fn oos_chunk_page(page_id: i32, index: i32, next_page: Option<i32>) -> [u8; IO_PAGE_SIZE] {
    let mut page = envelope_page(page_id, PageType::Oos);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[0..2].copy_from_slice(&1_i16.to_le_bytes());
    user[2..4].copy_from_slice(&1_i16.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    user[8..12].copy_from_slice(&16_280_i32.to_le_bytes());
    user[12..16].copy_from_slice(&16_280_i32.to_le_bytes());
    user[16..20].copy_from_slice(&56_i32.to_le_bytes());
    user[32..36].copy_from_slice(&8_i32.to_le_bytes());
    user[36..40].copy_from_slice(&index.to_le_bytes());
    let (next_page, next_slot, next_volume): (i32, i16, i16) =
        next_page.map_or((-1, -1, -1), |page| (page, 0, 0));
    user[40..44].copy_from_slice(&next_page.to_le_bytes());
    user[44..46].copy_from_slice(&next_slot.to_le_bytes());
    user[46..48].copy_from_slice(&next_volume.to_le_bytes());
    user[48..52].copy_from_slice(b"hide");
    let slot = 32_u32 | (20_u32 << 14) | (2_u32 << 28);
    user[IO_PAGE_SIZE - 40 - 4..IO_PAGE_SIZE - 40].copy_from_slice(&slot.to_le_bytes());
    page
}

fn fixture() -> (TestDirectory, PathBuf, PathBuf) {
    let directory = TestDirectory::new();
    let volume = directory.path().join("fixture");
    let vinf = directory.path().join("fixture_vinf");
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
        let page_type = match page_id {
            10 => PageType::Heap,
            20 | 21 => PageType::Oos,
            _ => PageType::Unknown,
        };
        let mut page = match page_id {
            20 => oos_chunk_page(20, 0, Some(21)),
            21 => oos_chunk_page(21, 1, None),
            _ => envelope_page(page_id, page_type),
        };
        if page_id == 10 {
            let user = &mut page[32..IO_PAGE_SIZE - 8];
            user[4..6].copy_from_slice(&1_i16.to_le_bytes());
            user[6..8].copy_from_slice(&8_u16.to_le_bytes());
            user[8..12].copy_from_slice(&16_312_i32.to_le_bytes());
            user[12..16].copy_from_slice(&16_312_i32.to_le_bytes());
            user[16..20].copy_from_slice(&32_i32.to_le_bytes());
        }
        file.write_all_at(&page, u64::try_from(page_id).unwrap() * IO_PAGE_SIZE as u64)
            .unwrap();
    }
    drop(file);
    let mut manifest = File::create(&vinf).unwrap();
    writeln!(manifest, "0 {}", volume.display()).unwrap();
    (directory, volume, vinf)
}

fn policy(memory_limit: u64) -> ResourcePolicy {
    ResourcePolicy::new(memory_limit, 1024 * 1024, 1, 32, 1024 * 1024).unwrap()
}

#[test]
fn inspection_opens_sparse_volume_and_scans_only_reserved_sector_envelopes() {
    let (_directory, _volume, vinf) = fixture();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let inspection =
        Inspection::open(&request, policy(4 * 1024 * 1024), &CancelToken::new(), None).unwrap();
    let view = inspection.view(RevisionSelector::Latest).unwrap();
    let overview = view.overview();

    assert_eq!(overview.outcome, InspectionOutcome::SuccessLimited);
    assert_eq!(overview.physical_page_count, 4096);
    assert_eq!(overview.inspected_page_envelopes, 64);
    assert_eq!(overview.reserved_sector_count, 1);
    assert!(overview.diagnostics.is_empty());

    let heap = view
        .page(Vpid::new(VolId::new(0).unwrap(), PageId::new(10).unwrap()))
        .unwrap();
    assert_eq!(heap.page_type, Some(PageType::Heap));
    assert_eq!(heap.availability, Availability::Available);

    let unreserved = view
        .sector(VolId::new(0).unwrap(), SectorId::new(1).unwrap())
        .unwrap();
    assert!(!unreserved.reserved);
    assert!(
        unreserved
            .pages
            .iter()
            .all(|page| page.availability == Availability::Unsupported)
    );
}

#[test]
fn resource_refusal_publishes_partial_coverage_without_silent_sampling() {
    let (_directory, _volume, vinf) = fixture();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let inspection = Inspection::open(&request, policy(1), &CancelToken::new(), None).unwrap();
    let overview = inspection
        .view(RevisionSelector::Latest)
        .unwrap()
        .overview();

    assert_eq!(overview.outcome, InspectionOutcome::Incomplete);
    assert!(
        overview
            .coverage
            .iter()
            .any(|coverage| coverage.facet == "page-envelopes"
                && coverage.stop_reason == Some("resource-limit"))
    );
    assert!(
        overview
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "inspection.resource_limit")
    );
}

#[test]
fn packed_facts_spill_privately_and_preserve_the_graph_projection() {
    let (_directory, _volume, vinf) = fixture();
    let spill_parent = TestDirectory::new();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: Some(spill_parent.path().to_path_buf()),
    };
    let resident = Inspection::open(
        &OpenRequest {
            spill_directory: None,
            ..request.clone()
        },
        policy(4 * 1024 * 1024),
        &CancelToken::new(),
        None,
    )
    .unwrap()
    .view(RevisionSelector::Latest)
    .unwrap();
    let spilled = Inspection::open(
        &request,
        ResourcePolicy::new(768, 4 * 1024, 1, 32, 1024 * 1024).unwrap(),
        &CancelToken::new(),
        None,
    )
    .unwrap()
    .view(RevisionSelector::Latest)
    .unwrap();

    assert_eq!(spilled.overview(), resident.overview());
    let heap = Vpid::new(VolId::new(0).unwrap(), PageId::new(10).unwrap());
    assert_eq!(spilled.page(heap).unwrap(), resident.page(heap).unwrap());
    assert_eq!(
        std::fs::read_dir(spill_parent.path()).unwrap().count(),
        0,
        "the private spill inode and directory must be unlinked while open"
    );
}

#[test]
fn exhausted_spill_budget_publishes_an_explicit_partial_prefix() {
    let (_directory, _volume, vinf) = fixture();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let overview = Inspection::open(
        &request,
        ResourcePolicy::new(768, 16, 1, 32, 1024 * 1024).unwrap(),
        &CancelToken::new(),
        None,
    )
    .unwrap()
    .view(RevisionSelector::Latest)
    .unwrap()
    .overview();

    assert_eq!(overview.inspected_page_envelopes, 1);
    assert_eq!(overview.outcome, InspectionOutcome::Incomplete);
    assert!(overview.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "inspection.resource_limit"
            && diagnostic.rule == "inspection.resource_policy.spill"
    }));
}

#[test]
fn worker_counts_merge_to_the_same_canonical_snapshot() {
    let (_directory, _volume, vinf) = fixture();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let serial = Inspection::open(
        &request,
        ResourcePolicy::new(4 * 1024 * 1024, 1024 * 1024, 1, 32, 1024 * 1024).unwrap(),
        &CancelToken::new(),
        None,
    )
    .unwrap()
    .view(RevisionSelector::Latest)
    .unwrap();
    let parallel = Inspection::open(
        &request,
        ResourcePolicy::new(4 * 1024 * 1024, 1024 * 1024, 4, 32, 1024 * 1024).unwrap(),
        &CancelToken::new(),
        None,
    )
    .unwrap()
    .view(RevisionSelector::Latest)
    .unwrap();

    assert_eq!(parallel.overview(), serial.overview());
    assert_eq!(
        parallel
            .sector(VolId::new(0).unwrap(), SectorId::new(0).unwrap())
            .unwrap(),
        serial
            .sector(VolId::new(0).unwrap(), SectorId::new(0).unwrap())
            .unwrap()
    );
}

#[test]
fn corruption_heavy_scan_stops_at_the_resident_diagnostic_boundary() {
    let (_directory, volume, vinf) = fixture();
    let file = OpenOptions::new().write(true).open(volume).unwrap();
    let corrupt = [0_u8; IO_PAGE_SIZE];
    for page_id in 2_u64..64 {
        file.write_all_at(&corrupt, page_id * IO_PAGE_SIZE as u64)
            .unwrap();
    }
    drop(file);
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let parallel = Inspection::open(
        &request,
        ResourcePolicy::new(1024, 1024 * 1024, 4, 32, 1024 * 1024).unwrap(),
        &CancelToken::new(),
        None,
    )
    .unwrap()
    .view(RevisionSelector::Latest)
    .unwrap()
    .overview();
    let serial = Inspection::open(
        &request,
        ResourcePolicy::new(1024, 1024 * 1024, 1, 32, 1024 * 1024).unwrap(),
        &CancelToken::new(),
        None,
    )
    .unwrap()
    .view(RevisionSelector::Latest)
    .unwrap()
    .overview();

    assert_eq!(parallel, serial);
    assert!(parallel.inspected_page_envelopes < 64);
    assert_eq!(parallel.outcome, InspectionOutcome::Incomplete);
    assert!(parallel.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "inspection.resource_limit"
            && diagnostic.rule == "inspection.resource_policy.diagnostics"
    }));
    assert!(parallel.coverage.iter().any(|coverage| {
        coverage.facet == "page-envelopes" && coverage.stop_reason == Some("resource-limit")
    }));
}

#[test]
fn stable_projection_omits_manifest_and_volume_paths() {
    let (_directory, volume, vinf) = fixture();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf.clone(),
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let view = Inspection::open(&request, policy(4 * 1024 * 1024), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
    let overview = view.overview();
    let document = result_document(
        "summary",
        None,
        &overview,
        DataProjection::Summary {
            overview: summary_projection(&overview),
        },
    );
    let serialized = serde_json::to_string(&document).unwrap();

    assert!(!serialized.contains(&vinf.display().to_string()));
    assert!(!serialized.contains(&volume.display().to_string()));
}

#[test]
fn html_export_is_deterministic_private_atomic_and_path_free() {
    let (directory, volume, vinf) = fixture();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf.clone(),
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let view = Inspection::open(&request, policy(4 * 1024 * 1024), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
    let first = directory.path().join("first.html");
    let second = directory.path().join("second.html");

    export_html(&view, &first, 8 * 1024 * 1024).unwrap();
    export_html(&view, &second, 8 * 1024 * 1024).unwrap();
    let first_bytes = std::fs::read(&first).unwrap();
    assert_eq!(first_bytes, std::fs::read(&second).unwrap());
    assert_eq!(
        std::fs::metadata(&first).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let text = String::from_utf8(first_bytes).unwrap();
    assert!(text.contains("Content-Security-Policy"));
    assert!(text.contains("connect-src 'none'"));
    assert!(text.contains("bytes withheld"));
    assert!(text.contains("THIRD-PARTY COMPONENTS"));
    assert!(text.contains("aes 0.9.2"));
    assert!(!text.contains(&vinf.display().to_string()));
    assert!(!text.contains(&volume.display().to_string()));

    assert!(matches!(
        export_html(&view, &first, 8 * 1024 * 1024),
        Err(ExportError::DestinationExists)
    ));
    let limited = directory.path().join("limited.html");
    assert!(matches!(
        export_html(&view, &limited, 100),
        Err(ExportError::LimitExceeded { .. })
    ));
    assert!(!limited.exists());
}

#[test]
fn deep_page_enrichment_publishes_a_new_revision_without_rewriting_the_old_view() {
    let (_directory, _volume, vinf) = fixture();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let original = Inspection::open(&request, policy(4 * 1024 * 1024), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
    let vpid = Vpid::new(VolId::new(0).unwrap(), PageId::new(10).unwrap());
    let enriched = original
        .enrich_page(vpid, policy(4 * 1024 * 1024), &CancelToken::new())
        .unwrap();

    assert_eq!(original.overview().revision.get(), 0);
    assert!(original.deep_page(vpid).is_none());
    assert_eq!(enriched.overview().revision.get(), 1);
    let deep = enriched.deep_page(vpid).unwrap();
    assert_eq!(deep.revision.get(), 1);
    assert!(deep.diagnostic_rule.is_none());
    assert!(deep.slotted.unwrap().slots().is_empty());
    assert!(
        enriched
            .overview()
            .coverage
            .iter()
            .any(|coverage| coverage.facet == "deep-pages"
                && coverage.stop_reason == Some("selective-enrichment"))
    );
}

#[test]
fn oos_enrichment_validates_a_chain_and_retains_no_payload_bytes() {
    let (_directory, _volume, vinf) = fixture();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let original = Inspection::open(&request, policy(4 * 1024 * 1024), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
    let head = Oid::new(
        VolId::new(0).unwrap(),
        PageId::new(20).unwrap(),
        SlotId::new(0).unwrap(),
    );
    let enriched = original
        .enrich_oos(head, policy(4 * 1024 * 1024), &CancelToken::new())
        .unwrap();
    let chain = enriched.oos_chain(head).unwrap();

    assert_eq!(enriched.overview().revision.get(), 1);
    assert!(chain.complete);
    assert_eq!(chain.total_data_length, Some(8));
    assert_eq!(chain.validated_payload_bytes, 8);
    assert_eq!(chain.chunks.len(), 2);
    assert!(!format!("{chain:?}").contains("hide"));
}

#[test]
fn oos_enrichment_publishes_a_validated_prefix_at_the_chain_budget() {
    let (_directory, _volume, vinf) = fixture();
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    let original = Inspection::open(&request, policy(4 * 1024 * 1024), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
    let head = Oid::new(
        VolId::new(0).unwrap(),
        PageId::new(20).unwrap(),
        SlotId::new(0).unwrap(),
    );
    let limited = ResourcePolicy::new(4 * 1024 * 1024, 1024, 1, 1, 1024 * 1024).unwrap();
    let enriched = original
        .enrich_oos(head, limited, &CancelToken::new())
        .unwrap();
    let chain = enriched.oos_chain(head).unwrap();

    assert!(!chain.complete);
    assert_eq!(chain.chunks.len(), 1);
    assert_eq!(chain.validated_payload_bytes, 4);
    assert_eq!(chain.diagnostic_rule, Some("resource-limit"));
    assert_eq!(enriched.overview().outcome, InspectionOutcome::Incomplete);
}
