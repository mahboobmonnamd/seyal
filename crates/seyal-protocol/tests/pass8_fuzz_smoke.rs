use std::{env, fs, path::PathBuf};

use seyal_protocol::pass8::BlockState;

#[test]
#[ignore = "executed by fuzz/targets/block-state-decode with retained seeds"]
fn block_state_decode_seed() {
    let path = PathBuf::from(
        env::var_os("SEYAL_FUZZ_INPUT").expect("SEYAL_FUZZ_INPUT is required"),
    );
    let bytes = fs::read(path).expect("read retained Pass 8 fuzz seed");
    let _ = BlockState::decode(&bytes);
}
