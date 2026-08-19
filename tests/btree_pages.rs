use volmap::format::{
    BtreePageFact, DB_PAGE_SIZE, IO_PAGE_SIZE, PageType, decode_btree_page, decode_page_envelope,
    decode_slotted_page,
};
use volmap::model::{PageId, VolId, Vpid};

fn btree_page(page_id: i32, records: &[(u16, u16, u8)]) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    page[8..12].copy_from_slice(&page_id.to_le_bytes());
    page[12..14].copy_from_slice(&0_i16.to_le_bytes());
    page[14] = PageType::Btree.ordinal();
    let user = &mut page[32..IO_PAGE_SIZE - 8];
    let slots = i16::try_from(records.len()).unwrap();
    user[0..2].copy_from_slice(&slots.to_le_bytes());
    user[2..4].copy_from_slice(&slots.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    let free_offset = records
        .iter()
        .map(|(offset, length, _)| i32::from(*offset) + i32::from(*length))
        .max()
        .unwrap();
    let free = i32::try_from(DB_PAGE_SIZE).unwrap() - i32::from(slots) * 4 - free_offset;
    user[8..12].copy_from_slice(&free.to_le_bytes());
    user[12..16].copy_from_slice(&free.to_le_bytes());
    user[16..20].copy_from_slice(&free_offset.to_le_bytes());
    for (slot_id, (offset, length, record_type)) in records.iter().enumerate() {
        let word =
            u32::from(*offset) | (u32::from(*length) << 14) | (u32::from(*record_type) << 28);
        let position = DB_PAGE_SIZE - (slot_id + 1) * 4;
        user[position..position + 4].copy_from_slice(&word.to_le_bytes());
    }
    page
}

fn decode(bytes: &[u8], page_id: i32, is_root: bool) -> BtreePageFact {
    let envelope = decode_page_envelope(
        bytes,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(page_id).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    decode_btree_page(&envelope, &slotted, is_root).unwrap()
}

fn put_null_vpid(user: &mut [u8], offset: usize) {
    user[offset..offset + 4].copy_from_slice(&(-1_i32).to_le_bytes());
    user[offset + 4..offset + 6].copy_from_slice(&(-1_i16).to_le_bytes());
}

fn put_node(user: &mut [u8], offset: usize, level: i16) {
    put_null_vpid(user, offset + 8);
    put_null_vpid(user, offset + 16);
    user[offset + 24..offset + 26].copy_from_slice(&level.to_le_bytes());
    user[offset + 26..offset + 28].copy_from_slice(&12_i16.to_le_bytes());
}

#[test]
fn btree_root_leaf_nonleaf_and_oid_overflow_roles_are_structural() {
    let mut root = btree_page(20, &[(32, 92, 2), (128, 16, 2)]);
    let user = &mut root[32..IO_PAGE_SIZE - 8];
    put_node(user, 32, 1);
    user[64..72].copy_from_slice(&(-1_i64).to_le_bytes());
    user[72..80].copy_from_slice(&(-1_i64).to_le_bytes());
    user[80..88].copy_from_slice(&(-1_i64).to_le_bytes());
    user[88..92].copy_from_slice(&7_i32.to_le_bytes());
    user[92..94].copy_from_slice(&2_i16.to_le_bytes());
    user[94..96].copy_from_slice(&0_i16.to_le_bytes());
    put_null_vpid(user, 104);
    user[120..124].copy_from_slice(&[1, 2, 3, 4]);
    let BtreePageFact::Root(root) = decode(&root, 20, true) else {
        panic!("expected root");
    };
    assert_eq!(root.node.level, 1);
    assert_eq!(root.node.common_prefix, None);
    assert_eq!(root.node.record_count, 1);
    assert_eq!(root.top_class.page_id.get(), 7);
    assert_eq!(root.domain_offset, 120);
    assert_eq!(root.domain_length, 4);

    let mut nonleaf = btree_page(21, &[(32, 32, 2), (64, 12, 2), (80, 16, 2)]);
    let user = &mut nonleaf[32..IO_PAGE_SIZE - 8];
    put_node(user, 32, 2);
    user[64..68].copy_from_slice(&30_i32.to_be_bytes());
    user[68..70].copy_from_slice(&0_i16.to_be_bytes());
    user[70..72].copy_from_slice(&4_i16.to_be_bytes());
    user[80..84].copy_from_slice(&31_i32.to_be_bytes());
    user[84..86].copy_from_slice(&0_i16.to_be_bytes());
    user[86..88].copy_from_slice(&(-1_i16).to_be_bytes());
    user[88..92].copy_from_slice(&50_i32.to_be_bytes());
    user[92..94].copy_from_slice(&0_i16.to_be_bytes());
    let BtreePageFact::NonLeaf(node) = decode(&nonleaf, 21, false) else {
        panic!("expected nonleaf");
    };
    assert_eq!(node.child_count, 2);
    assert_eq!(node.common_prefix, Some(0));
    assert_eq!(node.overflow_key_count, 1);

    let mut overflow = btree_page(22, &[(32, 8, 2), (40, 16, 2)]);
    put_null_vpid(&mut overflow[32..IO_PAGE_SIZE - 8], 32);
    let BtreePageFact::OidOverflow(overflow) = decode(&overflow, 22, false) else {
        panic!("expected OID overflow");
    };
    assert_eq!(overflow.next, None);
    assert_eq!(overflow.record_count, 1);
    assert_eq!(overflow.record_bytes, 16);
}

#[test]
fn btree_decoder_rejects_forged_root_flags_children_and_roles() {
    let mut root = btree_page(20, &[(32, 92, 2)]);
    let user = &mut root[32..IO_PAGE_SIZE - 8];
    put_node(user, 32, 1);
    user[64..72].copy_from_slice(&(-1_i64).to_le_bytes());
    user[72..80].copy_from_slice(&(-1_i64).to_le_bytes());
    user[80..88].copy_from_slice(&(-1_i64).to_le_bytes());
    user[88..92].copy_from_slice(&7_i32.to_le_bytes());
    user[92..94].copy_from_slice(&2_i16.to_le_bytes());
    user[94..96].copy_from_slice(&0_i16.to_le_bytes());
    user[96..100].copy_from_slice(&2_u32.to_le_bytes());
    put_null_vpid(user, 104);
    let envelope = decode_page_envelope(
        &root,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(20).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_btree_page(&envelope, &slotted, true)
            .unwrap_err()
            .rule(),
        "btree.root.constraint_flags"
    );

    let mut nonleaf = btree_page(21, &[(32, 32, 2), (64, 8, 2)]);
    let user = &mut nonleaf[32..IO_PAGE_SIZE - 8];
    put_node(user, 32, 2);
    user[64..68].copy_from_slice(&(-1_i32).to_be_bytes());
    user[68..70].copy_from_slice(&(-1_i16).to_be_bytes());
    let envelope = decode_page_envelope(
        &nonleaf,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(21).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_btree_page(&envelope, &slotted, false)
            .unwrap_err()
            .rule(),
        "btree.nonleaf.child"
    );

    let wrong_role = btree_page(22, &[(32, 24, 2)]);
    let envelope = decode_page_envelope(
        &wrong_role,
        Vpid::new(VolId::new(0).unwrap(), PageId::new(22).unwrap()),
    )
    .unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();
    assert_eq!(
        decode_btree_page(&envelope, &slotted, false)
            .unwrap_err()
            .rule(),
        "btree.page.role_length"
    );
}
