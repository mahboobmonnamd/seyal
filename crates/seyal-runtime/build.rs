use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../../resources/terminfo/seyal-m001.src");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        println!("cargo:rustc-env=SEYAL_M001_TERMINFO_DIR=");
        return;
    }

    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let source = manifest.join("../../resources/terminfo/seyal-m001.src");
    let destination = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("terminfo");
    fs::create_dir_all(&destination).expect("create bundled terminfo output directory");

    let status = Command::new("tic")
        .arg("-x")
        .arg("-o")
        .arg(&destination)
        .arg(&source)
        .status()
        .expect("M001 macOS build requires the system tic compiler");
    assert!(status.success(), "tic failed to compile seyal-m001 terminfo");
    println!(
        "cargo:rustc-env=SEYAL_M001_TERMINFO_DIR={}",
        destination.display()
    );
}
