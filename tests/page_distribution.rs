use volmap::format::{
    DB_PAGE_SIZE, IO_PAGE_SIZE, PageType, decode_page_envelope, decode_slotted_page,
};
use volmap::model::{PageId, VolId, Vpid};
use volmap::projection::{
    PageDistributionProjection, RecordTypeProjection, page_distribution_projection,
};

fn representative_slotted_page() -> [u8; IO_PAGE_SIZE] {
    let mut bytes = [0_u8; IO_PAGE_SIZE];
    bytes[8..12].copy_from_slice(&7_i32.to_le_bytes());
    bytes[12..14].copy_from_slice(&1_i16.to_le_bytes());
    bytes[14] = PageType::Heap.ordinal();

    let user = &mut bytes[32..IO_PAGE_SIZE - 8];
    user[0..2].copy_from_slice(&4_i16.to_le_bytes());
    user[2..4].copy_from_slice(&3_i16.to_le_bytes());
    user[4..6].copy_from_slice(&1_i16.to_le_bytes());
    user[6..8].copy_from_slice(&8_u16.to_le_bytes());
    user[8..12].copy_from_slice(&16_256_i32.to_le_bytes());
    user[12..16].copy_from_slice(&16_200_i32.to_le_bytes());
    user[16..20].copy_from_slice(&128_i32.to_le_bytes());
    for (slot, offset, length, kind) in [
        (0_usize, 32_u16, 24_u16, 2_u8),
        (1, 0, 0, 9),
        // Retained tombstone geometry is validated but is not a live record.
        (2, 104, 16, 6),
        (3, 80, 16, 3),
    ] {
        let word = u32::from(offset) | (u32::from(length) << 14) | (u32::from(kind) << 28);
        let start = DB_PAGE_SIZE - 4 * (slot + 1);
        user[start..start + 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

#[test]
fn shared_distribution_exhaustively_projects_validated_slotted_geometry() {
    let bytes = representative_slotted_page();
    let vpid = Vpid::new(VolId::new(1).unwrap(), PageId::new(7).unwrap());
    let envelope = decode_page_envelope(&bytes, vpid).unwrap();
    let slotted = decode_slotted_page(&envelope).unwrap();

    let PageDistributionProjection::Available {
        content_size,
        header,
        record_extents,
        free_regions,
        slot_directory,
        slot_entries,
        allocated_record_bytes,
        unoccupied_bytes,
    } = page_distribution_projection(&slotted)
    else {
        panic!("a validated slotted page must have a distribution");
    };

    assert_eq!(content_size, 16_344);
    assert_eq!((header.offset, header.length), (0, 32));
    assert_eq!(
        record_extents
            .iter()
            .map(|record| (
                record.slot_id,
                record.offset,
                record.length,
                record.record_type
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, 32, 24, RecordTypeProjection::Home),
            (3, 80, 16, RecordTypeProjection::NewHome)
        ]
    );
    assert_eq!(
        free_regions
            .iter()
            .map(|region| (region.offset, region.length, region.kind.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (56, 24, "fragmented-free"),
            (96, 32, "fragmented-free"),
            (128, 16_200, "contiguous-free"),
        ]
    );
    assert_eq!((slot_directory.offset, slot_directory.length), (16_328, 16));
    assert_eq!(
        slot_entries
            .iter()
            .map(|entry| {
                (
                    entry.slot_id,
                    entry.offset,
                    entry.length,
                    entry.state.as_str(),
                    entry.record_type,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (0, 16_340, 4, "allocated", "home"),
            (1, 16_336, 4, "unallocated", "reserved"),
            (2, 16_332, 4, "deleted", "marked-deleted"),
            (3, 16_328, 4, "allocated", "new-home"),
        ]
    );
    assert_eq!(allocated_record_bytes, 40);
    assert_eq!(unoccupied_bytes, 16_256);
    assert_eq!(
        header.length + allocated_record_bytes + unoccupied_bytes + slot_directory.length,
        content_size
    );
}
