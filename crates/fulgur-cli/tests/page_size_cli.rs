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
    let tail = &text[idx..idx + 60.min(text.len() - idx)];
    let start = tail.find('[').unwrap();
    let end = tail.find(']').unwrap();
    tail[start + 1..end].trim().to_string()
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
    // A4: 210mm x 297mm = 595.28 x 841.89 pt
    let pdf = render_with_size("210x297mm");
    let mb = media_box(&pdf);
    assert!(mb.starts_with("0 0 595.2"), "got {mb}");
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
