#![no_main]

use libfuzzer_sys::fuzz_target;
use volmap::bytes::ByteView;

fuzz_target!(|data: &[u8]| {
    let mut words = [0_u8; 24];
    let prefix = data.len().min(words.len());
    words[..prefix].copy_from_slice(&data[..prefix]);
    let offset =
        usize::try_from(u64::from_le_bytes(words[0..8].try_into().unwrap())).unwrap_or(usize::MAX);
    let length =
        usize::try_from(u64::from_le_bytes(words[8..16].try_into().unwrap())).unwrap_or(usize::MAX);
    let origin = u64::from_le_bytes(words[16..24].try_into().unwrap());
    let bytes = data.get(prefix..).unwrap_or_default();
    let view = ByteView::new(bytes, origin);
    let _ = view.range(offset, length, "fuzz range");
    let _ = view.subview(offset, length, "fuzz subview");
    let _ = view.read_u16_le(offset, "fuzz u16");
    let _ = view.read_i32_be(offset, "fuzz i32");
    let _ = view.read_u64_le(offset, "fuzz u64");
});
