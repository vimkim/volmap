#![no_main]

use libfuzzer_sys::fuzz_target;
use volmap::tde::decode_key_info_record;

fuzz_target!(|data: &[u8]| {
    let _ = decode_key_info_record(data);
    let mut exact = [0_u8; 92];
    let length = data.len().min(exact.len());
    exact[..length].copy_from_slice(&data[..length]);
    let _ = decode_key_info_record(&exact);
});
