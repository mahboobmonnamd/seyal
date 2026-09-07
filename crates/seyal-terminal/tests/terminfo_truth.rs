//! Terminfo honesty: advertise only implemented/tested capabilities.
use std::{fs, path::PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;

fn terminfo_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/terminfo/seyal-m001.src")
}

#[test]
fn seyal_m001_source_omits_unimplemented_capabilities() {
    let source = fs::read_to_string(terminfo_source()).expect("terminfo source");
    // Strong negative set required by #821 (and deferred #822 mouse / graphics).
    let required_absent = [
        "kmous", "XM", "xm", "sixel", "Ms", "Smolx", "rmolx", "kitty", "RGB",
    ];
    for cap in required_absent {
        let advertised = source
            .split(|c: char| c == ',' || c == '\n' || c.is_whitespace())
            .any(|token| token == cap || token.starts_with(&format!("{cap}=")));
        assert!(
            !advertised,
            "terminfo must not advertise unimplemented capability {cap}"
        );
    }
}

#[test]
fn seyal_m001_source_includes_implemented_scroll_and_edit_caps() {
    let source = fs::read_to_string(terminfo_source()).expect("terminfo source");
    for cap in [
        "csr=", "il=", "dl=", "ich=", "dch=", "ech=", "indn=", "rin=", "ri=",
    ] {
        assert!(
            source.contains(cap),
            "terminfo must advertise implemented capability prefix {cap}"
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn compiled_seyal_m001_resolves_and_omits_mouse() {
    let source = terminfo_source();
    let out = tempfile_dir();
    let status = Command::new("tic")
        .args(["-x", "-o"])
        .arg(&out)
        .arg(&source)
        .status()
        .expect("tic available on macOS");
    assert!(status.success(), "tic must compile seyal-m001");

    let output = Command::new("infocmp")
        .env("TERMINFO", &out)
        .arg("seyal-m001")
        .output()
        .expect("infocmp");
    assert!(output.status.success(), "infocmp seyal-m001 must succeed");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("csr="), "compiled entry should include csr");
    assert!(text.contains("il="), "compiled entry should include il");
    for banned in ["kmous", "XM=", "sixel"] {
        assert!(
            !text.contains(banned),
            "compiled terminfo must not contain {banned}"
        );
    }
}

#[cfg(target_os = "macos")]
fn tempfile_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "seyal-terminfo-truth-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).expect("temp terminfo dir");
    path
}
