#![no_main]

use libfuzzer_sys::fuzz_target;
use seyal_protocol::pass8::BlockState;

fuzz_target!(|data: &[u8]| {
    let _ = BlockState::decode(data);
});
