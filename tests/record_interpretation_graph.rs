//! Record interpretation as inspection-graph evidence.
//!
//! The fixture places the pinned `e1e651de-records` corpus pages at the page
//! ids they were extracted from, across the two volumes they came from: the
//! class objects live on volume 0 and the rows on volume 1, and a record's
//! class OID names the former, so both are needed to resolve anything.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use volmap::format::{AttributeInterpretation, AttributeValue, IO_PAGE_SIZE, PageType};
use volmap::inspection::{CancelToken, Inspection, OpenRequest, ResourcePolicy, RevisionSelector};
use volmap::model::{Oid, PageId, SlotId, VolId, Vpid};
use volmap::source::InputSpec;

const CLASS_PAGE: i32 = 195;
const SCALARS_ROWS_PAGE: i32 = 641;
const ALTERED_ROWS_PAGE: i32 = 1345;
const ALTERED_CLASS_PAGE: i32 = 207;
const OOS_ROWS_PAGE: i32 = 1153;
const OOS_CLASS_PAGE: i32 = 199;

const TOTAL_SECTORS: u32 = 64;
const SECTOR_PAGES: u32 = 64;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "volmap-interpretation-test-{}-{sequence}",
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

fn envelope_page(vol_id: i16, page_id: i32, page_type: PageType) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    let lsa = u64::try_from(page_id).unwrap().to_le_bytes();
    page[0..8].copy_from_slice(&lsa);
    page[8..12].copy_from_slice(&page_id.to_le_bytes());
    page[12..14].copy_from_slice(&vol_id.to_le_bytes());
    page[14] = page_type.ordinal();
    page[IO_PAGE_SIZE - 8..].copy_from_slice(&lsa);
    page
}

fn volume_header_page(vol_id: i16) -> [u8; IO_PAGE_SIZE] {
    let mut page = envelope_page(vol_id, 0, PageType::VolumeHeader);
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
    user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
    user[28..30].copy_from_slice(&vol_id.to_le_bytes());
    user[32..36].copy_from_slice(&0_i32.to_le_bytes());
    user[36..40].copy_from_slice(&0_i32.to_le_bytes());
    user[40..44].copy_from_slice(&i32::try_from(SECTOR_PAGES).unwrap().to_le_bytes());
    user[44..48].copy_from_slice(&i32::try_from(TOTAL_SECTORS).unwrap().to_le_bytes());
    user[48..52].copy_from_slice(&i32::try_from(TOTAL_SECTORS).unwrap().to_le_bytes());
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

/// Writes one volume holding `pages` at their own page ids, reserving exactly
/// the sectors those pages fall in.
fn write_volume(path: &Path, vol_id: i16, pages: &[(i32, Vec<u8>)]) {
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap();
    file.set_len(u64::from(TOTAL_SECTORS * SECTOR_PAGES) * IO_PAGE_SIZE as u64)
        .unwrap();
    file.write_all_at(&volume_header_page(vol_id), 0).unwrap();

    let mut reserved: u64 = 1; // sector 0 holds the header and the bitmap
    for (page_id, _) in pages {
        reserved |= 1_u64 << (u32::try_from(*page_id).unwrap() / SECTOR_PAGES);
    }
    let mut bitmap = envelope_page(vol_id, 1, PageType::VolumeBitmap);
    bitmap[32..40].copy_from_slice(&reserved.to_le_bytes());
    file.write_all_at(&bitmap, IO_PAGE_SIZE as u64).unwrap();

    // Every other page of a reserved sector still needs a decodable envelope.
    for sector in 0..TOTAL_SECTORS {
        if reserved & (1_u64 << sector) == 0 {
            continue;
        }
        for page_id in (sector * SECTOR_PAGES)..((sector + 1) * SECTOR_PAGES) {
            if page_id < 2 {
                continue;
            }
            let page_id = i32::try_from(page_id).unwrap();
            let bytes = pages
                .iter()
                .find(|(candidate, _)| *candidate == page_id)
                .map_or_else(
                    || envelope_page(vol_id, page_id, PageType::Unknown).to_vec(),
                    |(_, bytes)| bytes.clone(),
                );
            file.write_all_at(
                &bytes,
                u64::try_from(page_id).unwrap() * IO_PAGE_SIZE as u64,
            )
            .unwrap();
        }
    }
    drop(file);
}

fn corpus(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/e1e651de-records/pages")
            .join(name),
    )
    .unwrap()
}

/// A two-volume snapshot carrying the class objects and the row pages.
fn fixture() -> (TestDirectory, PathBuf) {
    let directory = TestDirectory::new();
    let volume0 = directory.path().join("interp");
    let volume1 = directory.path().join("interp_x001");
    let vinf = directory.path().join("interp_vinf");

    write_volume(
        &volume0,
        0,
        &[
            (CLASS_PAGE, corpus("vol0-page195.bin")),
            (OOS_CLASS_PAGE, corpus("vol0-page199.bin")),
            (ALTERED_CLASS_PAGE, corpus("vol0-page207.bin")),
        ],
    );
    write_volume(
        &volume1,
        1,
        &[
            (SCALARS_ROWS_PAGE, corpus("vol1-page641.bin")),
            (OOS_ROWS_PAGE, corpus("vol1-page1153.bin")),
            (ALTERED_ROWS_PAGE, corpus("vol1-page1345.bin")),
        ],
    );

    let mut manifest = File::create(&vinf).unwrap();
    writeln!(manifest, "0 {}", volume0.display()).unwrap();
    writeln!(manifest, "1 {}", volume1.display()).unwrap();
    (directory, vinf)
}

fn policy() -> ResourcePolicy {
    ResourcePolicy::new(8 * 1024 * 1024, 1024 * 1024, 1, 64, 8 * 1024 * 1024).unwrap()
}

fn open(vinf: PathBuf) -> volmap::inspection::GraphView {
    let request = OpenRequest {
        input: InputSpec::Vinf {
            path: vinf,
            volume_root: None,
        },
        tde_keys_file: None,
        spill_directory: None,
    };
    Inspection::open(&request, policy(), &CancelToken::new(), None)
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap()
}

fn vpid(vol: i16, page: i32) -> Vpid {
    Vpid::new(VolId::new(vol).unwrap(), PageId::new(page).unwrap())
}

fn record_oid(vol: i16, page: i32, slot: i16) -> Oid {
    Oid::new(
        VolId::new(vol).unwrap(),
        PageId::new(page).unwrap(),
        SlotId::new(slot).unwrap(),
    )
}

fn value(view: &volmap::inspection::GraphView, oid: Oid, name: &str) -> AttributeInterpretation {
    view.record_interpretation(oid)
        .unwrap()
        .attributes
        .iter()
        .find(|attribute| attribute.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no attribute {name}"))
        .interpretation
        .clone()
}

#[test]
fn class_representation_enrichment_publishes_schema_evidence_once() {
    let (_directory, vinf) = fixture();
    let original = open(vinf);
    assert_eq!(original.overview().revision.get(), 0);

    let enriched = original
        .enrich_class_representation(vpid(1, SCALARS_ROWS_PAGE), policy(), &CancelToken::new())
        .unwrap();
    assert_eq!(enriched.overview().revision.get(), 1);

    let representations = enriched.class_representations();
    assert_eq!(representations.len(), 1);
    let view = &representations[0];
    let representation = view.representation.as_ref().unwrap();
    assert_eq!(representation.class_name, "dba.interp_scalars");
    assert_eq!(view.class_oid, record_oid(0, CLASS_PAGE, 2));
    assert_eq!(view.representation_id, representation.representation_id);
    assert!(view.diagnostic_rule.is_none());

    // Re-requesting facts already in this revision is a no-op, not a new
    // revision carrying the same evidence.
    let again = enriched
        .enrich_class_representation(vpid(1, SCALARS_ROWS_PAGE), policy(), &CancelToken::new())
        .unwrap();
    assert_eq!(again.overview().revision.get(), 1);
}

#[test]
fn a_page_enrichment_interprets_every_home_record_in_one_revision() {
    let (_directory, vinf) = fixture();
    let enriched = open(vinf)
        .enrich_record_page(vpid(1, SCALARS_ROWS_PAGE), policy(), &CancelToken::new())
        .unwrap();
    assert_eq!(enriched.overview().revision.get(), 1);

    // The page holds three rows; slot 0 is the page's own heap metadata.
    let interpretations = enriched.record_interpretations();
    assert_eq!(interpretations.len(), 3);

    let bound = record_oid(1, SCALARS_ROWS_PAGE, 1);
    assert_eq!(
        value(&enriched, bound, "id"),
        AttributeInterpretation::Decoded(AttributeValue::Integer(1))
    );
    assert_eq!(
        value(&enriched, bound, "c_numeric"),
        AttributeInterpretation::Decoded(AttributeValue::Numeric("-12345678.90".to_owned()))
    );

    // The all-NULL row reports NULL rather than absent attributes.
    let unset = record_oid(1, SCALARS_ROWS_PAGE, 2);
    assert_eq!(
        value(&enriched, unset, "c_short"),
        AttributeInterpretation::Null
    );
    assert_eq!(
        value(&enriched, unset, "c_varchar"),
        AttributeInterpretation::Null
    );

    // The class representation the records resolved through is published too,
    // so an export carries the schema alongside the values.
    let representation = enriched
        .class_representation(record_oid(0, CLASS_PAGE, 2), 1)
        .unwrap();
    assert_eq!(
        representation.representation.as_ref().unwrap().class_name,
        "dba.interp_scalars"
    );

    // Repeat clicks on the same page do not advance the revision.
    let again = enriched
        .enrich_record_page(vpid(1, SCALARS_ROWS_PAGE), policy(), &CancelToken::new())
        .unwrap();
    assert_eq!(again.overview().revision.get(), 1);
}

#[test]
fn a_second_page_of_the_same_sector_reuses_the_cached_class() {
    let (_directory, vinf) = fixture();
    let first = open(vinf)
        .enrich_record_page(vpid(1, SCALARS_ROWS_PAGE), policy(), &CancelToken::new())
        .unwrap();
    // Page 642 shares sector 10 with page 641, so its class needs no second
    // class-record read; it has no rows, so the enrichment publishes nothing new.
    let second = first
        .enrich_class_representation(
            vpid(1, SCALARS_ROWS_PAGE + 1),
            policy(),
            &CancelToken::new(),
        )
        .unwrap();
    assert_eq!(second.overview().revision.get(), 1);
    assert_eq!(second.class_representations().len(), 1);
}

#[test]
fn an_out_of_row_attribute_is_published_as_a_reference_not_a_value() {
    let (_directory, vinf) = fixture();
    let enriched = open(vinf)
        .enrich_record_page(vpid(1, OOS_ROWS_PAGE), policy(), &CancelToken::new())
        .unwrap();

    let demoted = record_oid(1, OOS_ROWS_PAGE, 2);
    let AttributeInterpretation::OutOfRow { head, total_length } =
        value(&enriched, demoted, "out_value")
    else {
        panic!("expected an out-of-row reference");
    };
    assert_eq!(total_length, 32_776);
    assert_eq!(head.vol_id.get(), 1);
    assert_eq!(
        value(&enriched, demoted, "label"),
        AttributeInterpretation::Decoded(AttributeValue::Text("out-of-row".to_owned()))
    );
}

#[test]
fn a_row_older_than_its_class_resolves_through_its_own_representation() {
    let (_directory, vinf) = fixture();
    let enriched = open(vinf)
        .enrich_record_page(vpid(1, ALTERED_ROWS_PAGE), policy(), &CancelToken::new())
        .unwrap();

    let before = enriched
        .record_interpretation(record_oid(1, ALTERED_ROWS_PAGE, 1))
        .unwrap();
    let after = enriched
        .record_interpretation(record_oid(1, ALTERED_ROWS_PAGE, 2))
        .unwrap();
    assert_eq!(
        (before.representation_id, after.representation_id),
        (1, 2),
        "the two rows were written under different representations"
    );
    assert!(before.diagnostic_rule.is_none());

    // Both representations are published, and only the newer one has the
    // column the ALTER added.
    assert_eq!(enriched.class_representations().len(), 2);
    assert_eq!(
        value(&enriched, record_oid(1, ALTERED_ROWS_PAGE, 1), "pre_alter"),
        AttributeInterpretation::Decoded(AttributeValue::Integer(11))
    );
    assert_eq!(
        value(&enriched, record_oid(1, ALTERED_ROWS_PAGE, 2), "post_alter"),
        AttributeInterpretation::Decoded(AttributeValue::Integer(222))
    );
    assert!(
        !before
            .attributes
            .iter()
            .any(|attribute| attribute.name.as_deref() == Some("post_alter"))
    );
}

#[test]
fn a_page_with_no_class_degrades_with_a_reason_and_keeps_structural_facts() {
    let (_directory, vinf) = fixture();
    let view = open(vinf);
    // Page 3 is a plain envelope with no heap content at all.
    let degraded = view
        .enrich_record_page(vpid(0, 3), policy(), &CancelToken::new())
        .unwrap();

    assert!(degraded.record_interpretations().is_empty());
    let overview = degraded.overview();
    let diagnostic = overview
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "record.interpretation.unavailable")
        .expect("a durable reason for the degradation");
    assert_eq!(diagnostic.severity, "warning");
    assert!(!diagnostic.rule.is_empty());
    // The page itself is still queryable: degradation is not an error page.
    assert!(degraded.page(vpid(0, 3)).is_ok());
}
