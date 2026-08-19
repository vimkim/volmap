#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;
use volmap::format::{
    PageType, decode_dropped_files_page, decode_extdata_header, decode_file_header,
    decode_full_sectors, decode_overflow_continuation, decode_overflow_head, decode_page_envelope,
    decode_partial_sectors, decode_tracker_items, decode_user_pages, decode_vacuum_page,
};

fuzz_target!(|data: &[u8]| {
    let page = common::normalized_page(
        data,
        &[
            PageType::FileTable,
            PageType::Overflow,
            PageType::VacuumData,
            PageType::DroppedFiles,
        ],
    );
    let Some(expected) = common::page_vpid(&page) else {
        return;
    };
    let Ok(envelope) = decode_page_envelope(&page, expected) else {
        return;
    };
    match envelope.page_type() {
        PageType::FileTable => {
            let _ = decode_file_header(&envelope);
            for item_size in [8_u16, 16] {
                for offset in [0_u16, 32, u16::MAX] {
                    if let Ok(header) = decode_extdata_header(&envelope, offset, item_size) {
                        let _ = decode_partial_sectors(&envelope, header);
                        let _ = decode_full_sectors(&envelope, header);
                        let _ = decode_user_pages(&envelope, header);
                        let _ = decode_tracker_items(&envelope, header);
                    }
                }
            }
        }
        PageType::Overflow => {
            let _ = decode_overflow_head(&envelope);
            for remaining in [0_u32, 1, 16_344, u32::MAX] {
                let _ = decode_overflow_continuation(&envelope, remaining);
            }
        }
        PageType::VacuumData => {
            let _ = decode_vacuum_page(&envelope);
        }
        PageType::DroppedFiles => {
            let _ = decode_dropped_files_page(&envelope);
        }
        _ => unreachable!(),
    }
});
