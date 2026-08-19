#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;
use volmap::format::{
    IO_PAGE_SIZE, PAGE_PREFIX_SIZE, PAGE_WATERMARK_SIZE, decode_page_envelope,
    decode_page_envelope_parts,
};

fuzz_target!(|data: &[u8]| {
    let mut page = [0_u8; IO_PAGE_SIZE];
    let length = data.len().min(page.len());
    page[..length].copy_from_slice(&data[..length]);
    let expected = common::vpid();
    let _ = decode_page_envelope(data, expected);
    let _ = decode_page_envelope(page.as_slice(), expected);
    let _ = decode_page_envelope_parts(
        &page[..PAGE_PREFIX_SIZE],
        &page[IO_PAGE_SIZE - PAGE_WATERMARK_SIZE..],
        expected,
    );
});
