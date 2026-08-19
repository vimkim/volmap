use volmap::bytes::{ByteAccessErrorKind, ByteView};
use volmap::format::{
    DB_PAGE_SIZE, IO_PAGE_SIZE, PAGE_PREFIX_SIZE, PAGE_WATERMARK_SIZE, PageType,
    decode_page_envelope, decode_page_envelope_parts, decode_slotted_page,
};
use volmap::inspection::ResourcePolicy;
use volmap::model::{PageId, VolId, Vpid};

#[derive(Clone, Copy)]
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn usize(&mut self, limit: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(limit).unwrap()).unwrap()
    }
}

fn vpid() -> Vpid {
    Vpid::new(VolId::new(3).unwrap(), PageId::new(41).unwrap())
}

fn page(page_type: PageType) -> [u8; IO_PAGE_SIZE] {
    let mut bytes = [0_u8; IO_PAGE_SIZE];
    let lsa = 0x0102_0304_0506_0708_u64.to_le_bytes();
    bytes[..8].copy_from_slice(&lsa);
    bytes[8..12].copy_from_slice(&vpid().page_id.get().to_le_bytes());
    bytes[12..14].copy_from_slice(&vpid().vol_id.get().to_le_bytes());
    bytes[14] = page_type.ordinal();
    bytes[IO_PAGE_SIZE - PAGE_WATERMARK_SIZE..].copy_from_slice(&lsa);
    bytes
}

fn put_slotted_header(bytes: &mut [u8; IO_PAGE_SIZE], slots: i16, records: i16) {
    let user = &mut bytes[PAGE_PREFIX_SIZE..IO_PAGE_SIZE - PAGE_WATERMARK_SIZE];
    user[0..2].copy_from_slice(&slots.to_le_bytes());
    user[2..4].copy_from_slice(&records.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    user[8..12].copy_from_slice(&16_000_i32.to_le_bytes());
    user[12..16].copy_from_slice(&15_000_i32.to_le_bytes());
    user[16..20].copy_from_slice(&64_i32.to_le_bytes());
}

fn put_slot(bytes: &mut [u8; IO_PAGE_SIZE], slot: usize, offset: u16, length: u16) {
    let word = u32::from(offset) | (u32::from(length) << 14) | (2_u32 << 28);
    let start = PAGE_PREFIX_SIZE + DB_PAGE_SIZE - 4 * (slot + 1);
    bytes[start..start + 4].copy_from_slice(&word.to_le_bytes());
}

#[test]
fn property_byte_views_match_checked_slice_geometry() {
    let bytes = (0_u8..=255).collect::<Vec<_>>();
    let mut random = Lcg(0x2d5a_8c91_76e4_301b);

    for _ in 0..10_000 {
        let container_len = random.usize(bytes.len() + 1);
        let offset = random.usize(400);
        let length = random.usize(400);
        let origin = random.next() & 0x0000_ffff_ffff_ffff;
        let view = ByteView::new(&bytes[..container_len], origin);
        let expected = offset
            .checked_add(length)
            .filter(|end| *end <= container_len)
            .map(|end| &bytes[offset..end]);

        match (view.range(offset, length, "generated range"), expected) {
            (Ok(actual), Some(expected)) => assert_eq!(actual, expected),
            (Err(error), None) => assert!(matches!(
                error.kind(),
                ByteAccessErrorKind::OutOfBounds | ByteAccessErrorKind::ArithmeticOverflow
            )),
            pair => panic!("range model disagreed: {pair:?}"),
        }

        if let Ok(subview) = view.subview(offset, length, "generated subview") {
            assert_eq!(subview.origin(), origin + u64::try_from(offset).unwrap());
            assert_eq!(subview.len(), length);
        }
    }

    let overflow = ByteView::new(&bytes[..1], u64::MAX);
    assert_eq!(
        overflow
            .range(0, 1, "absolute overflow")
            .unwrap_err()
            .kind(),
        ByteAccessErrorKind::ArithmeticOverflow
    );
}

#[test]
fn property_envelope_decoder_is_deterministic_for_hostile_parts() {
    let mut random = Lcg(0x83d2_5f06_a14b_c799);

    for _ in 0..4_096 {
        let mut prefix = [0_u8; PAGE_PREFIX_SIZE];
        let mut watermark = [0_u8; PAGE_WATERMARK_SIZE];
        for byte in prefix.iter_mut().chain(watermark.iter_mut()) {
            *byte = random.next().to_le_bytes()[0];
        }
        let first = decode_page_envelope_parts(&prefix, &watermark, vpid());
        let second = decode_page_envelope_parts(&prefix, &watermark, vpid());
        assert_eq!(first, second);
    }
}

#[test]
fn property_slotted_decoder_is_deterministic_and_bounded_for_hostile_pages() {
    let mut random = Lcg(0x90bd_7441_ec17_2ca5);

    for _ in 0..1_024 {
        let mut bytes = page(PageType::Heap);
        for byte in &mut bytes[PAGE_PREFIX_SIZE..IO_PAGE_SIZE - PAGE_WATERMARK_SIZE] {
            *byte = random.next().to_le_bytes()[0];
        }
        let envelope = decode_page_envelope(&bytes, vpid()).unwrap();
        let first = decode_slotted_page(&envelope);
        let second = decode_slotted_page(&envelope);
        assert_eq!(first, second);
        if let Ok(decoded) = first {
            assert!(decoded.slots().len() <= DB_PAGE_SIZE / 4);
            assert!(decoded.slots().iter().all(|slot| {
                usize::from(slot.offset())
                    .checked_add(usize::from(slot.length()))
                    .is_some_and(|end| slot.is_empty() || end <= DB_PAGE_SIZE)
            }));
        }
    }
}

#[test]
fn metamorphic_record_payload_never_changes_or_exposes_structural_facts() {
    let mut before = page(PageType::Heap);
    put_slotted_header(&mut before, 1, 1);
    put_slot(&mut before, 0, 32, 32);
    before[PAGE_PREFIX_SIZE + 32..PAGE_PREFIX_SIZE + 64]
        .copy_from_slice(b"secret-A-secret-A-secret-A-00000");
    let mut after = before;
    after[PAGE_PREFIX_SIZE + 32..PAGE_PREFIX_SIZE + 64]
        .copy_from_slice(b"secret-B-secret-B-secret-B-11111");

    let before = decode_slotted_page(&decode_page_envelope(&before, vpid()).unwrap()).unwrap();
    let after = decode_slotted_page(&decode_page_envelope(&after, vpid()).unwrap()).unwrap();
    assert_eq!(before, after);
    let rendered = format!("{before:?}{after:?}");
    assert!(!rendered.contains("secret-A"));
    assert!(!rendered.contains("secret-B"));
}

#[test]
fn property_resource_policy_accepts_exactly_all_nonzero_components() {
    for bits in 0_u8..32 {
        let values = (0..5)
            .map(|index| u64::from((bits >> index) & 1))
            .collect::<Vec<_>>();
        let policy = ResourcePolicy::new(
            values[0],
            values[1],
            u32::try_from(values[2]).unwrap(),
            values[3],
            values[4],
        );
        assert_eq!(policy.is_ok(), bits == 31);
    }
}
