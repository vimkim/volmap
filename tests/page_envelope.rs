use volmap::format::{
    DB_PAGE_SIZE, DecodeErrorKind, FormatProfile, IO_PAGE_SIZE, PAGE_PREFIX_SIZE,
    PAGE_WATERMARK_SIZE, PageContent, PageType, TdeAlgorithm, decode_decrypted_page_envelope,
    decode_page_envelope, decode_page_envelope_parts, decode_page_envelope_with_profile,
    decode_slotted_page,
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
fn selected_format_profile_decodes_ambiguous_page_ordinal() {
    let develop = [
        PageType::Area,
        PageType::Catalog,
        PageType::Btree,
        PageType::Log,
        PageType::DroppedFiles,
        PageType::VacuumData,
    ];
    let feat_oos = [
        PageType::Oos,
        PageType::Area,
        PageType::Catalog,
        PageType::Btree,
        PageType::Log,
        PageType::DroppedFiles,
    ];

    for (index, (develop, feat_oos)) in develop.into_iter().zip(feat_oos).enumerate() {
        let raw = u8::try_from(index + 8).unwrap();
        let bytes = synthetic_page(vpid(), raw, 0);
        assert_eq!(
            decode_page_envelope_with_profile(&bytes, vpid(), FormatProfile::Develop)
                .unwrap()
                .page_type(),
            develop
        );
        assert_eq!(
            decode_page_envelope_with_profile(&bytes, vpid(), FormatProfile::FeatOos)
                .unwrap()
                .page_type(),
            feat_oos
        );
    }

    let raw_fourteen = synthetic_page(vpid(), 14, 0);
    assert_eq!(
        decode_page_envelope_with_profile(&raw_fourteen, vpid(), FormatProfile::Develop)
            .unwrap_err()
            .rule(),
        "page.envelope.type_known"
    );
    assert_eq!(
        decode_page_envelope_with_profile(&raw_fourteen, vpid(), FormatProfile::FeatOos)
            .unwrap()
            .page_type(),
        PageType::VacuumData
    );
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
        PageContent::Decrypted { .. } | PageContent::EncryptedOpaque { .. } => {
            panic!("plaintext changed state")
        }
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
fn decrypted_envelope_preserves_algorithm_and_exposes_only_supplied_plaintext() {
    let expected = Vpid::new(VolId::new(0).unwrap(), PageId::new(14).unwrap());
    let mut page = synthetic_page(expected, PageType::Heap.ordinal(), 0x02);
    page[32..40].copy_from_slice(b"cipher!!");
    let mut plaintext = [0_u8; DB_PAGE_SIZE];
    plaintext[4..6].copy_from_slice(&1_i16.to_le_bytes());
    plaintext[6..8].copy_from_slice(&8_u16.to_le_bytes());
    plaintext[8..12].copy_from_slice(&16_312_i32.to_le_bytes());
    plaintext[12..16].copy_from_slice(&16_312_i32.to_le_bytes());
    plaintext[16..20].copy_from_slice(&32_i32.to_le_bytes());
    let decoded = decode_decrypted_page_envelope(&page, &plaintext, expected).unwrap();
    assert_eq!(decoded.tde_algorithm(), Some(TdeAlgorithm::Aria));
    assert!(matches!(
        decoded.content(),
        PageContent::Decrypted {
            algorithm: TdeAlgorithm::Aria
        }
    ));
    let slotted = decode_slotted_page(&decoded).unwrap();
    assert!(slotted.slots().is_empty());
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

#[test]
fn fast_envelope_decoder_needs_only_prefix_and_watermark() {
    let bytes = synthetic_page(vpid(), PageType::Oos.ordinal(), 0x02);
    let decoded = decode_page_envelope_parts(
        &bytes[..PAGE_PREFIX_SIZE],
        &bytes[IO_PAGE_SIZE - PAGE_WATERMARK_SIZE..],
        vpid(),
    )
    .unwrap();

    assert_eq!(decoded.id(), vpid());
    assert_eq!(decoded.page_type(), PageType::Oos);
    assert_eq!(
        decoded.content(),
        PageContent::EncryptedOpaque {
            algorithm: TdeAlgorithm::Aria
        }
    );
}
