//! Terminfo honesty: advertise only implemented/tested capabilities.
#[cfg(target_os = "macos")]
use std::process::Command;
use std::{fs, path::PathBuf};

fn terminfo_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/terminfo/seyal-m001.src")
}

#[test]
fn seyal_m001_source_omits_unimplemented_capabilities() {
    let source = fs::read_to_string(terminfo_source()).expect("terminfo source");
    // Strong negative set required by #821. Mouse is implemented by #822.
    let required_absent = ["sixel", "Ms", "Smolx", "rmolx", "kitty", "RGB"];
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
        "csr=", "il=", "dl=", "ich=", "dch=", "ech=", "indn=", "rin=", "ri=", "kmous=", "XM=",
        "xm=",
    ] {
        assert!(
            source.contains(cap),
            "terminfo must advertise implemented capability prefix {cap}"
        );
    }
    assert!(
        source.contains("XM=\\E[?1000;1002;1006%?%p1%{1}%=%th%el%;"),
        "XM must be a parameterized enable/disable initializer"
    );
    assert!(
        source.contains("xm=\\E[<%p1%d;%p2%d;%p3%d%?%p4%tm%eM%;"),
        "xm must be a parameterized SGR mouse-event formatter"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn compiled_seyal_m001_resolves_and_advertises_mouse() {
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
    assert!(
        text.contains("kmous="),
        "compiled entry should include kmous"
    );
    assert!(
        !text.contains("sixel"),
        "compiled terminfo must not contain sixel"
    );
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
