use std::process::Command;

fn fulgur_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_fulgur"))
}

/// Render trivial HTML with `--size` and return the raw PDF bytes.
fn render_with_size(size: &str) -> Vec<u8> {
    use std::io::Write;
    let bin = fulgur_bin();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.pdf");
    let mut child = Command::new(&bin)
        .args([
            "render",
            "--stdin",
            "--size",
            size,
            "-o",
            out.to_str().unwrap(),
        ])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn fulgur render");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"<html><body><p>x</p></body></html>")
        .unwrap();
    assert!(
        child.wait().unwrap().success(),
        "render failed for --size {size}"
    );
    std::fs::read(&out).unwrap()
}

fn media_box(pdf: &[u8]) -> String {
    let text = String::from_utf8_lossy(pdf);
    let idx = text.find("/MediaBox").expect("no MediaBox");
    // `/MediaBox` is immediately followed by `[ ... ]`; search the remainder
    // for the bracket pair rather than slicing a fixed-size window (which
    // could split a multi-byte U+FFFD replacement char at its boundary).
    let rest = &text[idx..];
    let start = rest.find('[').expect("no '[' after MediaBox");
    let end = rest.find(']').expect("no ']' after MediaBox");
    rest[start + 1..end].trim().to_string()
}

#[test]
fn custom_pt_size_sets_media_box() {
    let bin = fulgur_bin();
    if !bin.exists() {
        eprintln!("fulgur binary not found, skipping");
        return;
    }
    let pdf = render_with_size("200ptx400pt");
    assert_eq!(media_box(&pdf), "0 0 200 400");
}

#[test]
fn custom_mm_size_sets_media_box() {
    let bin = fulgur_bin();
    if !bin.exists() {
        return;
    }
    // 100mm x 200mm = 283.46 x 566.93 pt (distinct from the A4 fallback)
    let pdf = render_with_size("100x200mm");
    let mb = media_box(&pdf);
    assert!(mb.starts_with("0 0 283.4"), "got {mb}");
}

#[test]
fn keyword_size_still_works() {
    let bin = fulgur_bin();
    if !bin.exists() {
        return;
    }
    let pdf = render_with_size("A4");
    assert!(media_box(&pdf).starts_with("0 0 595.2"));
}

#[test]
fn help_documents_custom_and_priority() {
    let bin = fulgur_bin();
    if !bin.exists() {
        return;
    }
    let out = Command::new(&bin)
        .args(["render", "--help"])
        .output()
        .expect("run --help");
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("WxH"), "help missing WxH: {help}");
    assert!(
        help.contains("@page"),
        "help missing @page priority note: {help}"
    );
}
