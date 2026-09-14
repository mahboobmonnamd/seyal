#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::Command;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn c_header_layout_compiles_and_matches_rust_sizes() {
    let root = crate_root();
    let source = root.join("tests/c/seyal_app_layout.c");
    let header_dir = root.join("include");
    let out = std::env::temp_dir().join(format!("seyal-app-layout-{}", std::process::id()));
    let status = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Werror", "-I"])
        .arg(&header_dir)
        .arg(&source)
        .arg("-o")
        .arg(&out)
        .status()
        .expect("clang");
    assert!(status.success(), "C consumer failed to compile");
    let output = Command::new(&out).output().expect("run layout check");
    assert!(output.status.success(), "{:?}", output);
    let _ = std::fs::remove_file(out);
}
