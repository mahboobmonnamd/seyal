#![no_main]

use libfuzzer_sys::fuzz_target;
use seyal_runtime::display::decode_chunk;

fuzz_target!(|data: &[u8]| {
    let _ = decode_chunk(data);
});
