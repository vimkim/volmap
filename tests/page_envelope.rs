use volmap::format::{
    DecodeErrorKind, IO_PAGE_SIZE, PageContent, PageType, TdeAlgorithm, decode_page_envelope,
};
use volmap::model::{PageId, VolId, Vpid};

fn synthetic_page(expected: Vpid, page_type: u8, flags: u8) -> Vec<u8> {
    let mut page = vec![0_u8; IO_PAGE_SIZE];
    let lsa = 0x1234_5678_9abc_def0_u64.to_le_bytes();
    page[0..8].copy_from_slice(&lsa);
    page[8..12].copy_from_slice(&expected.page_id.get().to_le_bytes());
    page[12..14].copy_from_slice(&expected.vol_id.get().to_le_bytes());
    page[14] = page_type;
    page[15] = flags;
    page[IO_PAGE_SIZE - 8..].copy_from_slice(&lsa);
    page
}

fn vpid() -> Vpid {
    Vpid::new(VolId::new(2).unwrap(), PageId::new(7).unwrap())
}

#[test]
fn valid_plaintext_envelope_reports_plaintext_without_exposing_body_bytes() {
    let bytes = synthetic_page(vpid(), PageType::Heap.ordinal(), 0);

    let decoded = decode_page_envelope(&bytes, vpid()).unwrap();

    assert_eq!(decoded.id(), vpid());
    assert_eq!(decoded.page_type(), PageType::Heap);
    assert_eq!(decoded.tde_algorithm(), None);
    match decoded.content() {
        PageContent::Plaintext => {}
        PageContent::EncryptedOpaque { .. } => panic!("plaintext became opaque"),
    }
}

#[test]
fn encrypted_envelope_never_exposes_its_ciphertext_region() {
    let bytes = synthetic_page(vpid(), PageType::Heap.ordinal(), 0x01);

    let decoded = decode_page_envelope(&bytes, vpid()).unwrap();

    assert_eq!(decoded.tde_algorithm(), Some(TdeAlgorithm::Aes));
    assert!(matches!(
        decoded.content(),
        PageContent::EncryptedOpaque {
            algorithm: TdeAlgorithm::Aes
        }
    ));
}

#[test]
fn corrupt_envelopes_fail_closed_before_body_parsing() {
    let mut cases = Vec::new();

    let mut bad_identity = synthetic_page(vpid(), PageType::Heap.ordinal(), 0);
    bad_identity[8..12].copy_from_slice(&8_i32.to_le_bytes());
    cases.push((bad_identity, DecodeErrorKind::IdentityMismatch));

    let mut bad_lsa = synthetic_page(vpid(), PageType::Heap.ordinal(), 0);
    bad_lsa[IO_PAGE_SIZE - 1] ^= 1;
    cases.push((bad_lsa, DecodeErrorKind::LsaMismatch));

    cases.push((
        synthetic_page(vpid(), 0xff, 0),
        DecodeErrorKind::UnknownEnum,
    ));
    cases.push((
        synthetic_page(vpid(), PageType::Heap.ordinal(), 0x03),
        DecodeErrorKind::InvalidFlags,
    ));
    cases.push((
        synthetic_page(vpid(), PageType::Heap.ordinal(), 0x80),
        DecodeErrorKind::InvalidFlags,
    ));

    for (bytes, expected_kind) in cases {
        assert_eq!(
            decode_page_envelope(&bytes, vpid()).unwrap_err().kind(),
            expected_kind
        );
    }
}

#[test]
fn physical_page_size_is_exact() {
    let short = vec![0_u8; IO_PAGE_SIZE - 1];

    assert_eq!(
        decode_page_envelope(&short, vpid()).unwrap_err().kind(),
        DecodeErrorKind::InvalidLength
    );
}
