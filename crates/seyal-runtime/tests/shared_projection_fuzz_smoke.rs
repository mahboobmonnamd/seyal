#![cfg(all(target_os = "macos", feature = "benchmark-shared-projection"))]
#![allow(unsafe_code)]

use std::{env, fs, path::PathBuf};

use seyal_runtime::projection::layout::{
    CELL_LEN, CellRecord, DAMAGE_LEN, DamageRecord, MAX_REGION_BYTES, REGION_HEADER_LEN,
    SLOT_HEADER_LEN, SlotHeader,
};
use seyal_runtime::projection::writer::{RegionMemory, read_latest, read_region_header};

fn input() -> Vec<u8> {
    let path =
        PathBuf::from(env::var_os("SEYAL_FUZZ_INPUT").expect("SEYAL_FUZZ_INPUT is required"));
    fs::read(path).expect("read retained fuzz seed")
}

#[test]
#[ignore = "executed only by the legacy Candidate-B comparator fuzz adapter"]
fn shared_projection_validation_seed() {
    let bytes = input();
    let bounded_len = bytes.len().min(MAX_REGION_BYTES as usize);
    let storage_bytes = bounded_len.max(REGION_HEADER_LEN).div_ceil(8) * 8;
    let mut storage = vec![0u64; storage_bytes / 8].into_boxed_slice();
    for (index, chunk) in bytes[..bounded_len].chunks(8).enumerate() {
        let mut word = [0u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        storage[index] = u64::from_le_bytes(word);
    }

    // SAFETY: boxed `u64` storage is 8-byte aligned and remains live for the
    // complete comparator reader exercise.
    let memory = unsafe { RegionMemory::new(storage.as_mut_ptr().cast(), storage_bytes) };
    if let Ok(region) = read_region_header(&memory) {
        let _ = read_latest(&memory, &region);
    }

    if bytes.len() >= SLOT_HEADER_LEN {
        let _ = SlotHeader::decode(&bytes[..SLOT_HEADER_LEN], 256, 512);
    }
    let (cell_chunks, _) = bytes.as_chunks::<CELL_LEN>();
    for chunk in cell_chunks {
        let _ = CellRecord::decode(chunk);
    }
    let (damage_chunks, _) = bytes.as_chunks::<DAMAGE_LEN>();
    for chunk in damage_chunks {
        let _ = DamageRecord::decode(chunk, 256);
    }
}
