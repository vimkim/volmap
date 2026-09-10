//! Independently verify the producer-owned oracle without network access.
use sha2::{Digest, Sha256};
use std::path::Path;

#[test]
fn vendored_producer_revision_and_every_corpus_byte_match_the_pin() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/pgbuf-inspector/v1");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let sums = std::fs::read(root.join("corpus/SHA256SUMS")).unwrap();
    assert_eq!(manifest["contract_version"], "1.0");
    assert_eq!(manifest["corpus_revision"], 2);
    let pinned = "11dbecc72e4b9dd78e22080f138c801c189c7e047c0db23af8233f44a804d5ce";
    assert_eq!(manifest["corpus_sha256"], pinned);
    assert_eq!(hex(&sums), pinned);
    let mut count = 0;
    for line in std::str::from_utf8(&sums).unwrap().lines() {
        let (expected, relative) = line.split_once("  ").unwrap();
        assert!(!relative.contains("..") && !relative.starts_with('/'));
        let bytes = std::fs::read(root.join("corpus").join(relative)).unwrap();
        assert_eq!(hex(&bytes), expected, "{relative}");
        count += 1;
    }
    assert!(count > 100, "the full corpus must be verified");
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}
