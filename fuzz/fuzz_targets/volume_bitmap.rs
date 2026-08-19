#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;
use volmap::format::{
    IO_PAGE_SIZE, PageType, decode_page_envelope, decode_sector_bitmap, decode_volume_header,
};

fuzz_target!(|data: &[u8]| {
    let (header_data, bitmap_data) = data.split_at(data.len().min(IO_PAGE_SIZE));
    let header_page = common::normalized_page(header_data, &[PageType::VolumeHeader]);
    let bitmap_page = common::normalized_page(bitmap_data, &[PageType::VolumeBitmap]);
    let Some(expected) = common::page_vpid(&header_page) else {
        return;
    };
    let Ok(header_envelope) = decode_page_envelope(&header_page, expected) else {
        return;
    };
    let Ok(header) = decode_volume_header(&header_envelope, 64 * 1024 * 1024) else {
        return;
    };
    let bitmap_vpid =
        volmap::model::Vpid::new(expected.vol_id, volmap::model::PageId::new(1).unwrap());
    let mut bitmap_page = bitmap_page;
    bitmap_page[8..12].copy_from_slice(&1_i32.to_le_bytes());
    bitmap_page[12..14].copy_from_slice(&expected.vol_id.get().to_le_bytes());
    let Ok(bitmap_envelope) = decode_page_envelope(&bitmap_page, bitmap_vpid) else {
        return;
    };
    let _ = decode_sector_bitmap(&bitmap_envelope, &header, 0);
});
