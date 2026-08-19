//! Manual, deterministic resource-policy benchmark matrix.
//!
//! Run the full profile with:
//! `VOLMAP_BENCH_SCALE=full VOLMAP_BENCH_SAMPLES=30 cargo test --release \
//!   --test resource_benchmark -- --ignored --nocapture`

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use volmap::diagnostics::InspectionOutcome;
use volmap::format::{IO_PAGE_SIZE, PageType};
use volmap::inspection::{
    CancelToken, FastScanResources, GraphView, Inspection, OpenRequest, ProgressObserver,
    ResourcePolicy, RevisionSelector, ScanPhase, ScanProgress,
};
use volmap::model::{Oid, PageId, SectorId, SlotId, VolId, Vpid};
use volmap::source::InputSpec;

const SECTOR_PAGES: u32 = 64;
const MIB: u64 = 1024 * 1024;
const DEFAULT_MEMORY: u64 = 256 * MIB;
const DEFAULT_SPILL: u64 = 2 * 1024 * MIB;
const DEFAULT_CHAIN_STEPS: u64 = 65_536;
const DEFAULT_DECODED: u64 = 256 * MIB;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct MatrixProfile {
    name: &'static str,
    total_sectors: u32,
    reserved_sectors: u32,
    corrupt_stride: Option<u32>,
}

const SMOKE_PROFILES: &[MatrixProfile] = &[
    MatrixProfile {
        name: "small",
        total_sectors: 64,
        reserved_sectors: 1,
        corrupt_stride: None,
    },
    MatrixProfile {
        name: "sparse",
        total_sectors: 512,
        reserved_sectors: 1,
        corrupt_stride: None,
    },
    MatrixProfile {
        name: "corrupt",
        total_sectors: 64,
        reserved_sectors: 8,
        corrupt_stride: Some(7),
    },
];

const FULL_PROFILES: &[MatrixProfile] = &[
    MatrixProfile {
        name: "small",
        total_sectors: 64,
        reserved_sectors: 1,
        corrupt_stride: None,
    },
    MatrixProfile {
        name: "medium",
        total_sectors: 256,
        reserved_sectors: 64,
        corrupt_stride: None,
    },
    MatrixProfile {
        name: "large",
        total_sectors: 1024,
        reserved_sectors: 256,
        corrupt_stride: None,
    },
    MatrixProfile {
        name: "sparse",
        total_sectors: 4096,
        reserved_sectors: 1,
        corrupt_stride: None,
    },
    MatrixProfile {
        name: "dense",
        total_sectors: 512,
        reserved_sectors: 512,
        corrupt_stride: None,
    },
    MatrixProfile {
        name: "corrupt",
        total_sectors: 128,
        reserved_sectors: 128,
        corrupt_stride: Some(7),
    },
];

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "volmap-resource-benchmark-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create benchmark directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove benchmark directory");
    }
}

struct Fixture {
    directory: TempDirectory,
    vinf: PathBuf,
    logical_bytes: u64,
    allocated_bytes: u64,
}

fn envelope_page(page_id: i32, page_type: PageType) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    let lsa = u64::try_from(page_id)
        .expect("nonnegative page id")
        .to_le_bytes();
    page[0..8].copy_from_slice(&lsa);
    page[8..12].copy_from_slice(&page_id.to_le_bytes());
    page[12..14].copy_from_slice(&0_i16.to_le_bytes());
    page[14] = page_type.ordinal();
    page[IO_PAGE_SIZE - 8..].copy_from_slice(&lsa);
    page
}

fn volume_header_page(total_sectors: u32) -> [u8; IO_PAGE_SIZE] {
    let mut page = envelope_page(0, PageType::VolumeHeader);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
    user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
    user[28..30].copy_from_slice(&0_i16.to_le_bytes());
    user[32..36].copy_from_slice(&0_i32.to_le_bytes());
    user[36..40].copy_from_slice(&0_i32.to_le_bytes());
    user[40..44].copy_from_slice(&64_i32.to_le_bytes());
    user[44..48].copy_from_slice(
        &i32::try_from(total_sectors)
            .expect("benchmark sectors fit i32")
            .to_le_bytes(),
    );
    user[48..52].copy_from_slice(
        &i32::try_from(total_sectors)
            .expect("benchmark sectors fit i32")
            .to_le_bytes(),
    );
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

fn bitmap_page(reserved_sectors: u32) -> [u8; IO_PAGE_SIZE] {
    let mut page = envelope_page(1, PageType::VolumeBitmap);
    for sector in 0..reserved_sectors {
        let word = usize::try_from(sector / 64).expect("bitmap word");
        let offset = 32 + word * 8;
        let mut value = u64::from_le_bytes(
            page[offset..offset + 8]
                .try_into()
                .expect("bitmap word range"),
        );
        value |= 1_u64 << (sector % 64);
        page[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    page
}

fn oos_chunk_page(
    page_id: i32,
    index: u32,
    chunk_count: u32,
    next_page: Option<i32>,
) -> [u8; IO_PAGE_SIZE] {
    let mut page = envelope_page(page_id, PageType::Oos);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[0..2].copy_from_slice(&1_i16.to_le_bytes());
    user[2..4].copy_from_slice(&1_i16.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    user[8..12].copy_from_slice(&16_280_i32.to_le_bytes());
    user[12..16].copy_from_slice(&16_280_i32.to_le_bytes());
    user[16..20].copy_from_slice(&56_i32.to_le_bytes());
    user[32..36].copy_from_slice(
        &i32::try_from(chunk_count * 4)
            .expect("OOS total fits i32")
            .to_le_bytes(),
    );
    user[36..40].copy_from_slice(
        &i32::try_from(index)
            .expect("OOS index fits i32")
            .to_le_bytes(),
    );
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

fn write_page(file: &File, page_id: u32, page: &[u8; IO_PAGE_SIZE]) {
    file.write_all_at(page, u64::from(page_id) * IO_PAGE_SIZE as u64)
        .expect("write benchmark page");
}

fn generate_profile(profile: MatrixProfile) -> Fixture {
    let directory = TempDirectory::new();
    let volume = directory.path().join(profile.name);
    let vinf = directory.path().join(format!("{}_vinf", profile.name));
    let logical_bytes =
        u64::from(profile.total_sectors) * u64::from(SECTOR_PAGES) * IO_PAGE_SIZE as u64;
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&volume)
        .expect("create benchmark volume");
    file.set_len(logical_bytes).expect("size benchmark volume");
    write_page(&file, 0, &volume_header_page(profile.total_sectors));
    write_page(&file, 1, &bitmap_page(profile.reserved_sectors));
    let eligible_pages = profile.reserved_sectors * SECTOR_PAGES;
    for page_id in 2..eligible_pages.max(SECTOR_PAGES) {
        let page = if profile
            .corrupt_stride
            .is_some_and(|stride| page_id % stride == 0)
        {
            [0_u8; IO_PAGE_SIZE]
        } else {
            envelope_page(
                i32::try_from(page_id).expect("benchmark page id fits i32"),
                PageType::Unknown,
            )
        };
        write_page(&file, page_id, &page);
    }
    drop(file);
    let mut manifest = File::create(&vinf).expect("create benchmark manifest");
    writeln!(manifest, "0 {}", volume.display()).expect("write benchmark manifest");
    drop(manifest);
    let allocated_bytes = std::fs::metadata(&volume)
        .expect("benchmark volume metadata")
        .blocks()
        * 512;
    Fixture {
        directory,
        vinf,
        logical_bytes,
        allocated_bytes,
    }
}

fn generate_oos_fixture(chunk_count: u32, cycle: bool) -> Fixture {
    let reserved_sectors = (64 + chunk_count).div_ceil(SECTOR_PAGES);
    let profile = MatrixProfile {
        name: if cycle { "oos-cycle" } else { "oos-boundary" },
        total_sectors: 64,
        reserved_sectors,
        corrupt_stride: None,
    };
    let fixture = generate_profile(profile);
    let volume = fixture.directory.path().join(profile.name);
    let file = OpenOptions::new()
        .write(true)
        .open(volume)
        .expect("open OOS benchmark volume");
    for index in 0..chunk_count {
        let page_id = 64 + index;
        let next = if index + 1 < chunk_count {
            Some(i32::try_from(page_id + 1).expect("next page fits i32"))
        } else if cycle {
            Some(64)
        } else {
            None
        };
        write_page(
            &file,
            page_id,
            &oos_chunk_page(
                i32::try_from(page_id).expect("OOS page fits i32"),
                index,
                chunk_count,
                next,
            ),
        );
    }
    drop(file);
    fixture
}

fn request(vinf: &Path) -> OpenRequest {
    OpenRequest {
        input: InputSpec::Vinf {
            path: vinf.to_path_buf(),
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    }
}

fn policy(memory: u64, spill: u64, workers: u32) -> ResourcePolicy {
    ResourcePolicy::new(memory, spill, workers, DEFAULT_CHAIN_STEPS, DEFAULT_DECODED)
        .expect("valid benchmark resource policy")
}

fn open_view(
    vinf: &Path,
    resource_policy: ResourcePolicy,
) -> (GraphView, Duration, FastScanResources) {
    let start = Instant::now();
    let inspection = Inspection::open(&request(vinf), resource_policy, &CancelToken::new(), None)
        .expect("benchmark inspection opens");
    let elapsed = start.elapsed();
    let view = inspection
        .view(RevisionSelector::Latest)
        .expect("benchmark latest revision");
    let resources = view.fast_scan_resources();
    (view, elapsed, resources)
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    let rank = samples
        .len()
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1);
    samples[rank.min(samples.len() - 1)]
}

fn duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn rss_bytes(field: &str) -> u64 {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    status
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name != field {
                return None;
            }
            value
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
                .and_then(|kib| kib.checked_mul(1024))
        })
        .unwrap_or(0)
}

#[derive(Serialize)]
struct ScanReport<'a> {
    kind: &'static str,
    profile: &'a str,
    samples: usize,
    workers_requested: u32,
    workers_admitted: u32,
    storage: &'static str,
    logical_bytes: u64,
    allocated_bytes: u64,
    physical_pages: u64,
    inspected_envelopes: u64,
    requested_envelope_bytes: u64,
    packed_fact_bytes: u64,
    admitted_resident_bytes: u64,
    spill_bytes: u64,
    elapsed_min_ns: u64,
    elapsed_p50_ns: u64,
    elapsed_p95_ns: u64,
    elapsed_p99_ns: u64,
    elapsed_max_ns: u64,
    process_rss_bytes: u64,
    process_peak_rss_bytes: u64,
}

fn measure_scans<'a>(
    profile: &'a MatrixProfile,
    fixture: &Fixture,
    resource_policy: ResourcePolicy,
    samples: usize,
) -> (ScanReport<'a>, GraphView) {
    let (baseline, _, resources) = open_view(&fixture.vinf, resource_policy);
    let overview = baseline.overview();
    let mut elapsed = Vec::with_capacity(samples);
    for _ in 0..samples {
        let (view, duration, measured) = open_view(&fixture.vinf, resource_policy);
        assert_eq!(
            view.overview(),
            overview,
            "scan output must be deterministic"
        );
        assert_eq!(
            measured, resources,
            "resource accounting must be deterministic"
        );
        elapsed.push(duration_ns(duration));
    }
    elapsed.sort_unstable();
    let storage = if resources.spill_bytes == 0 {
        "resident"
    } else {
        "spill"
    };
    (
        ScanReport {
            kind: "fast-scan",
            profile: profile.name,
            samples,
            workers_requested: resources.requested_workers,
            workers_admitted: resources.admitted_workers,
            storage,
            logical_bytes: fixture.logical_bytes,
            allocated_bytes: fixture.allocated_bytes,
            physical_pages: overview.physical_page_count,
            inspected_envelopes: overview.inspected_page_envelopes,
            requested_envelope_bytes: resources.envelope_requested_bytes,
            packed_fact_bytes: resources.packed_fact_bytes,
            admitted_resident_bytes: resources.admitted_resident_bytes,
            spill_bytes: resources.spill_bytes,
            elapsed_min_ns: elapsed[0],
            elapsed_p50_ns: percentile(&elapsed, 50),
            elapsed_p95_ns: percentile(&elapsed, 95),
            elapsed_p99_ns: percentile(&elapsed, 99),
            elapsed_max_ns: *elapsed.last().expect("nonempty benchmark samples"),
            process_rss_bytes: rss_bytes("VmRSS"),
            process_peak_rss_bytes: rss_bytes("VmHWM"),
        },
        baseline,
    )
}

#[derive(Serialize)]
struct QueryReport {
    kind: &'static str,
    operation: &'static str,
    samples: usize,
    p50_ns: u64,
    p95_ns: u64,
    p99_ns: u64,
    max_ns: u64,
}

fn measure_query(operation: &'static str, samples: usize, mut query: impl FnMut()) -> QueryReport {
    let mut elapsed = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        query();
        elapsed.push(duration_ns(start.elapsed()));
    }
    elapsed.sort_unstable();
    QueryReport {
        kind: "query",
        operation,
        samples,
        p50_ns: percentile(&elapsed, 50),
        p95_ns: percentile(&elapsed, 95),
        p99_ns: percentile(&elapsed, 99),
        max_ns: *elapsed.last().expect("nonempty query samples"),
    }
}

#[derive(Serialize)]
struct OosReport {
    kind: &'static str,
    boundary: &'static str,
    chunks: usize,
    complete: bool,
    diagnostic_rule: Option<&'static str>,
    elapsed_ns: u64,
}

fn measure_oos(
    view: &GraphView,
    head: Oid,
    boundary: &'static str,
    resource_policy: ResourcePolicy,
) -> OosReport {
    let start = Instant::now();
    let enriched = view
        .enrich_oos(head, resource_policy, &CancelToken::new())
        .expect("OOS benchmark enrichment");
    let elapsed_ns = duration_ns(start.elapsed());
    let chain = enriched
        .oos_chain(head)
        .expect("published OOS benchmark chain");
    OosReport {
        kind: "oos",
        boundary,
        chunks: chain.chunks.len(),
        complete: chain.complete,
        diagnostic_rule: chain.diagnostic_rule,
        elapsed_ns,
    }
}

struct CancelAt {
    token: CancelToken,
    completed: u64,
}

impl ProgressObserver for CancelAt {
    fn update(&mut self, progress: ScanProgress) {
        if progress.phase == ScanPhase::PageEnvelopes && progress.completed >= self.completed {
            self.token.cancel();
        }
    }
}

#[derive(Serialize)]
struct CancellationReport {
    kind: &'static str,
    requested_boundary: u64,
    inspected_envelopes: u64,
    read_attempts: u64,
    elapsed_ns: u64,
    outcome: &'static str,
}

fn emit(value: &impl Serialize) {
    println!(
        "{}",
        serde_json::to_string(value).expect("serialize benchmark report")
    );
}

#[test]
#[ignore = "manual resource benchmark matrix"]
#[allow(clippy::too_many_lines)]
fn representative_resource_matrix() {
    let full = std::env::var("VOLMAP_BENCH_SCALE").is_ok_and(|value| value == "full");
    let profiles = if full { FULL_PROFILES } else { SMOKE_PROFILES };
    let samples = std::env::var("VOLMAP_BENCH_SAMPLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(if full { 11 } else { 3 });
    let query_samples = if full { 20_000 } else { 1_000 };
    let mut dense_view = None;
    let mut dense_vinf = None;
    let mut dense_fixture = None;

    for profile in profiles {
        let fixture = generate_profile(*profile);
        let (serial_report, serial) = measure_scans(
            profile,
            &fixture,
            policy(DEFAULT_MEMORY, DEFAULT_SPILL, 1),
            samples,
        );
        emit(&serial_report);
        let (parallel_report, parallel) = measure_scans(
            profile,
            &fixture,
            policy(DEFAULT_MEMORY, DEFAULT_SPILL, 4),
            samples,
        );
        emit(&parallel_report);
        assert_eq!(serial.overview(), parallel.overview());

        if matches!(profile.name, "large" | "dense") {
            let (spill_report, spilled) = measure_scans(
                profile,
                &fixture,
                policy(64 * 1024, DEFAULT_SPILL, 4),
                samples,
            );
            emit(&spill_report);
            assert_eq!(serial.overview(), spilled.overview());
        }
        if profile.name == "dense" || (!full && profile.name == "small") {
            dense_vinf = Some(fixture.vinf.clone());
            dense_view = Some(serial);
            dense_fixture = Some(fixture);
        }
    }

    let view = dense_view.expect("query benchmark profile");
    emit(&measure_query("overview", query_samples, || {
        std::hint::black_box(view.overview());
    }));
    emit(&measure_query("sector", query_samples, || {
        std::hint::black_box(
            view.sector(
                VolId::new(0).expect("volid"),
                SectorId::new(0).expect("sector"),
            )
            .expect("benchmark sector"),
        );
    }));
    emit(&measure_query("page", query_samples, || {
        std::hint::black_box(
            view.page(Vpid::new(
                VolId::new(0).expect("volid"),
                PageId::new(32).expect("page"),
            ))
            .expect("benchmark page"),
        );
    }));

    let vinf = dense_vinf.expect("cancellation benchmark profile");
    let cancellation_boundary = if full { 1024 } else { 32 };
    let token = CancelToken::new();
    let mut observer = CancelAt {
        token: token.clone(),
        completed: cancellation_boundary,
    };
    let start = Instant::now();
    let cancelled = Inspection::open(
        &request(&vinf),
        policy(DEFAULT_MEMORY, DEFAULT_SPILL, 4),
        &token,
        Some(&mut observer),
    )
    .expect("cancelled benchmark publishes a prefix")
    .view(RevisionSelector::Latest)
    .expect("cancelled benchmark revision");
    assert_eq!(cancelled.overview().outcome, InspectionOutcome::Incomplete);
    emit(&CancellationReport {
        kind: "cancellation",
        requested_boundary: cancellation_boundary,
        inspected_envelopes: cancelled.overview().inspected_page_envelopes,
        read_attempts: cancelled.fast_scan_resources().envelope_read_attempts,
        elapsed_ns: duration_ns(start.elapsed()),
        outcome: "incomplete",
    });
    drop(dense_fixture);

    let chunks = if full { 512 } else { 32 };
    let oos = generate_oos_fixture(chunks, false);
    let (oos_view, _, _) = open_view(&oos.vinf, policy(DEFAULT_MEMORY, DEFAULT_SPILL, 4));
    let head = Oid::new(
        VolId::new(0).expect("OOS volid"),
        PageId::new(64).expect("OOS page"),
        SlotId::new(0).expect("OOS slot"),
    );
    emit(&measure_oos(
        &oos_view,
        head,
        "steps-limit-minus-one",
        ResourcePolicy::new(
            DEFAULT_MEMORY,
            DEFAULT_SPILL,
            4,
            u64::from(chunks - 1),
            DEFAULT_DECODED,
        )
        .expect("OOS step policy"),
    ));
    emit(&measure_oos(
        &oos_view,
        head,
        "steps-limit",
        ResourcePolicy::new(
            DEFAULT_MEMORY,
            DEFAULT_SPILL,
            4,
            u64::from(chunks),
            DEFAULT_DECODED,
        )
        .expect("OOS step policy"),
    ));
    emit(&measure_oos(
        &oos_view,
        head,
        "bytes-limit-minus-one-page",
        ResourcePolicy::new(
            DEFAULT_MEMORY,
            DEFAULT_SPILL,
            4,
            DEFAULT_CHAIN_STEPS,
            u64::from(chunks - 1) * IO_PAGE_SIZE as u64,
        )
        .expect("OOS byte policy"),
    ));
    emit(&measure_oos(
        &oos_view,
        head,
        "bytes-limit",
        ResourcePolicy::new(
            DEFAULT_MEMORY,
            DEFAULT_SPILL,
            4,
            DEFAULT_CHAIN_STEPS,
            u64::from(chunks) * IO_PAGE_SIZE as u64,
        )
        .expect("OOS byte policy"),
    ));

    let cyclic = generate_oos_fixture(chunks, true);
    let (cyclic_view, _, _) = open_view(&cyclic.vinf, policy(DEFAULT_MEMORY, DEFAULT_SPILL, 4));
    emit(&measure_oos(
        &cyclic_view,
        head,
        "cycle",
        ResourcePolicy::new(
            DEFAULT_MEMORY,
            DEFAULT_SPILL,
            4,
            u64::from(chunks) + 1,
            DEFAULT_DECODED,
        )
        .expect("OOS cycle policy"),
    ));
}
