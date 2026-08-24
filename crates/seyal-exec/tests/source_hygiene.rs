use std::{fs, path::Path};

fn visit(path: &Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).expect("read source directory") {
        let entry = entry.expect("source entry");
        let path = entry.path();
        if path.is_dir() {
            visit(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn production_source_contains_no_rill_identifiers_and_unsafe_is_platform_scoped() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    visit(&source_root, &mut paths);
    paths.sort();

    for path in paths {
        let content = fs::read_to_string(&path).expect("read source file");
        assert!(
            !content.contains("RILL_"),
            "legacy RILL environment identifier found in {}",
            path.display()
        );

        let is_macos_ffi = path.ends_with("platform/macos.rs");
        if !is_macos_ffi {
            assert!(
                !content.contains("unsafe {"),
                "unsafe block escaped audited macOS FFI module: {}",
                path.display()
            );
        }
    }
}

#[test]
fn window_size_rejects_zero_cell_dimensions() {
    use seyal_exec::WindowSize;

    assert!(WindowSize::cells(0, 24).is_err());
    assert!(WindowSize::cells(80, 0).is_err());
    assert_eq!(
        WindowSize::new(100, 40, 1000, 800)
            .expect("valid size")
            .columns(),
        100
    );
}
