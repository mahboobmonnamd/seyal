use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../../resources/terminfo/seyal-m001.src");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
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

    let entry = find_entry(&destination).expect("tic did not emit seyal-m001 entry");
    println!(
        "cargo:rustc-env=SEYAL_M001_TERMINFO_ENTRY={}",
        entry.display()
    );
}

fn find_entry(root: &PathBuf) -> Option<PathBuf> {
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for item in fs::read_dir(directory).ok()? {
            let item = item.ok()?;
            let path = item.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.file_name().is_some_and(|name| name == "seyal-m001") {
                return Some(path);
            }
        }
    }
    None
}
