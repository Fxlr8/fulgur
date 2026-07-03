# CLI custom page size (`--size WxH`) Implementation Plan

**Goal:** `fulgur render --size` にキーワード (A4/Letter/A3) だけでなく `WxH[unit]` 形式のカスタム寸法を受け付けさせ、ヘルプに優先順位を明記する。

**Architecture:** `crates/fulgur-cli/src/main.rs` の `parse_page_size` を拡張する。キーワードにマッチしなければ新規 `parse_custom_size` にフォールバックし、成功すれば `PageSize { width, height }`(pt) を返す。パーサは追加依存なしの手書きスキャナ（単位 `px` が区切り `x` を含む曖昧性を greedy な既知単位マッチで解消）。単位変換係数は `gcpm::parser::css_unit_to_pt` と同一だが private のため CLI 内にローカル `unit_to_pt` を複製（public API を広げない YAGNI 判断）。

**Tech Stack:** Rust, clap 4 (derive), 標準ライブラリのみ。テストは `#[cfg(test)] mod tests`（unit）+ `std::process::Command` + `env!("CARGO_BIN_EXE_fulgur")`（CLI smoke, `tempfile` dev-dep 既存）。

**優先順位モデル（不変）:** `CLI --size > CSS @page { size } > Config デフォルト`。`--size` を渡すと `EngineBuilder::page_size` 経由で `overrides.page_size = true` が立ち、CSS `@page` を抑制する（`crates/fulgur/src/gcpm/page_settings.rs:87`）。本変更は「`--size` にカスタム寸法も書けるようにする」ものであり、この優先順位自体は変えない。

**確定した入力仕様:**

- 区切りは `x`/`X` または空白（空白版はシェルで引用符が必要）。
- 単位は `mm`/`cm`/`in`/`pt`/`px`。片側だけ単位を書いた場合はもう片側にも適用（例 `210x297mm` = 210mm × 297mm）。
- 両側とも単位なし（例 `800x1200`）は不正 → 従来通り警告して A4 フォールバック。
- 値は正の有限数のみ。

---

## Task 1: `parse_custom_size` とヘルパーを TDD で実装

**Files:**

- Modify: `crates/fulgur-cli/src/main.rs`（`parse_page_size` 直後 = 現 `main.rs:287` 付近に追加）
- Test: `crates/fulgur-cli/src/main.rs` 末尾に `#[cfg(test)] mod tests`

### Step 1: 失敗するユニットテストを書く

`crates/fulgur-cli/src/main.rs` の末尾に追加する（`use fulgur::config::PageSize;` などの import は既存の `parse_page_size` が使っているものを流用。テスト内は `super::*` で参照）:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn keyword_still_works() {
        assert!(approx(parse_page_size("A4").width, PageSize::A4.width));
        assert!(approx(parse_page_size("letter").height, PageSize::LETTER.height));
        assert!(approx(parse_page_size("A3").width, PageSize::A3.width));
    }

    #[test]
    fn custom_pt_both_units() {
        let s = parse_page_size("2352.6ptx3481.39pt");
        assert!(approx(s.width, 2352.6));
        assert!(approx(s.height, 3481.39));
    }

    #[test]
    fn custom_mm_trailing_unit_applies_to_both() {
        // 210mm = 595.28pt, 297mm = 841.89pt (A4)
        let s = parse_page_size("210x297mm");
        assert!(approx(s.width, 210.0 * 72.0 / 25.4));
        assert!(approx(s.height, 297.0 * 72.0 / 25.4));
    }

    #[test]
    fn custom_leading_unit_applies_to_both() {
        let s = parse_page_size("210mmx297");
        assert!(approx(s.width, 210.0 * 72.0 / 25.4));
        assert!(approx(s.height, 297.0 * 72.0 / 25.4));
    }

    #[test]
    fn custom_px_separator_ambiguity() {
        // px は x を含む。素朴な split では壊れるケース。800px=600pt, 1200px=900pt
        let s = parse_page_size("800pxx1200px");
        assert!(approx(s.width, 600.0));
        assert!(approx(s.height, 900.0));
    }

    #[test]
    fn custom_whitespace_separator() {
        let s = parse_page_size("210mm 297mm");
        assert!(approx(s.width, 210.0 * 72.0 / 25.4));
        assert!(approx(s.height, 297.0 * 72.0 / 25.4));
    }

    #[test]
    fn custom_mixed_units() {
        // 100mm x 2in = 283.46pt x 144pt
        let s = parse_page_size("100mmx2in");
        assert!(approx(s.width, 100.0 * 72.0 / 25.4));
        assert!(approx(s.height, 144.0));
    }

    #[test]
    fn unitless_falls_back_to_a4() {
        // 単位必須: 両側単位なしは不正 → A4
        assert!(approx(parse_page_size("800x1200").width, PageSize::A4.width));
    }

    #[test]
    fn garbage_falls_back_to_a4() {
        assert!(approx(parse_page_size("banana").width, PageSize::A4.width));
        assert!(approx(parse_page_size("100mm").width, PageSize::A4.width)); // 単一値
        assert!(approx(parse_page_size("100foox200bar").width, PageSize::A4.width)); // 未知単位
        assert!(approx(parse_page_size("0ptx100pt").width, PageSize::A4.width)); // 非正値
    }
}
```

### Step 2: テストを実行して失敗を確認

Run: `cargo test -p fulgur-cli --bin fulgur custom_ 2>&1 | tail -20`
Expected: コンパイルエラー（`parse_custom_size` 未定義 / 新テストが FAIL）。

### Step 3: 実装を書く

`crates/fulgur-cli/src/main.rs` の `parse_page_size` を差し替え、直後にヘルパーを追加する。

`parse_page_size` を以下に置き換える:

```rust
fn parse_page_size(s: &str) -> PageSize {
    match s.to_uppercase().as_str() {
        "A4" => PageSize::A4,
        "A3" => PageSize::A3,
        "LETTER" => PageSize::LETTER,
        _ => parse_custom_size(s).unwrap_or_else(|| {
            eprintln!(
                "Unknown page size '{}', defaulting to A4. \
                 Use a keyword (A4, Letter, A3) or custom WxH with units \
                 (e.g. 210x297mm, 2352.6ptx3481.39pt; units mm/cm/in/pt/px, \
                 'x' or space separator), or set the size via CSS \
                 @page {{ size }} and omit --size.",
                s
            );
            PageSize::A4
        }),
    }
}

/// Known absolute CSS length units accepted for `--size`, longest checked
/// per position. All are two ASCII bytes, which is what lets the scanner
/// disambiguate the `px` unit from the `x` separator.
const PAGE_UNITS: [&str; 5] = ["mm", "cm", "in", "pt", "px"];

/// Convert an absolute length value + unit to PDF points.
/// Mirrors `fulgur::gcpm::parser::css_unit_to_pt` (private there; the
/// five-line table is duplicated locally rather than widening fulgur's
/// public API).
fn unit_to_pt(value: f32, unit: &str) -> Option<f32> {
    let factor = match () {
        _ if unit.eq_ignore_ascii_case("mm") => 72.0 / 25.4,
        _ if unit.eq_ignore_ascii_case("cm") => 72.0 / 2.54,
        _ if unit.eq_ignore_ascii_case("in") => 72.0,
        _ if unit.eq_ignore_ascii_case("pt") => 1.0,
        _ if unit.eq_ignore_ascii_case("px") => 72.0 / 96.0,
        _ => return None,
    };
    Some(value * factor)
}

/// Consume a leading `[0-9.]+` run and parse it as a positive f32.
fn take_number(rest: &str) -> Option<(f32, &str)> {
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    let (num, tail) = rest.split_at(end);
    let v: f32 = num.parse().ok()?;
    Some((v, tail))
}

/// Greedily consume a known page unit at the start of `rest`.
/// Returns `(Some(unit), tail)` or `(None, rest)` when none matches.
fn take_unit(rest: &str) -> (Option<&str>, &str) {
    for u in PAGE_UNITS {
        if let Some(head) = rest.get(..u.len()) {
            if head.eq_ignore_ascii_case(u) {
                return (Some(head), &rest[u.len()..]);
            }
        }
    }
    (None, rest)
}

/// Parse `WxH` custom page dimensions, e.g. `210x297mm` or `2352.6ptx3481.39pt`.
/// Separator is a single `x`/`X` or a run of whitespace. A unit on one side
/// applies to both; both sides unitless is rejected (unit required).
fn parse_custom_size(s: &str) -> Option<PageSize> {
    let s = s.trim();

    let (wv, rest) = take_number(s)?;
    let (wunit, rest) = take_unit(rest);

    // Separator: exactly one 'x'/'X', or a run of whitespace.
    let rest = if let Some(after) = rest.strip_prefix(['x', 'X']) {
        after
    } else {
        let trimmed = rest.trim_start();
        if trimmed.len() == rest.len() {
            return None; // no separator
        }
        trimmed
    };

    let (hv, rest) = take_number(rest)?;
    let (hunit, rest) = take_unit(rest);
    if !rest.trim().is_empty() {
        return None; // trailing garbage
    }

    // A unit on one side applies to both; both missing is invalid.
    let (wu, hu) = match (wunit, hunit) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, a),
        (None, Some(b)) => (b, b),
        (None, None) => return None,
    };

    let width = unit_to_pt(wv, wu)?;
    let height = unit_to_pt(hv, hu)?;
    if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        Some(PageSize { width, height })
    } else {
        None
    }
}
```

### Step 4: テストを実行して pass を確認

Run: `cargo test -p fulgur-cli --bin fulgur 2>&1 | tail -20`
Expected: 追加した全テストが PASS、既存も維持。

### Step 5: clippy / fmt

Run: `cargo clippy -p fulgur-cli 2>&1 | tail -20 && cargo fmt -p fulgur-cli`
Expected: 警告なし、fmt 差分なし。

### Step 6: コミット

```bash
git add crates/fulgur-cli/src/main.rs
git commit -m "feat(cli): accept custom WxH dimensions in --size"
```

---

## Task 2: `--size` ヘルプ文言に受理フォーマットと優先順位を明記

**Files:**

- Modify: `crates/fulgur-cli/src/main.rs:162`（`size` フィールドの doc comment）

### Step 1: doc comment を書き換える

現状（`main.rs:162-164`）:

```rust
        /// Page size (A4, Letter, A3)
        #[arg(short, long)]
        size: Option<String>,
```

を以下に置き換える:

```rust
        /// Page size: keyword (A4, Letter, A3) or custom WxH with units
        /// (units mm/cm/in/pt/px; 'x' or space separator),
        /// e.g. 210x297mm or 2352.6ptx3481.39pt.
        /// Takes priority over CSS @page { size }. Omit --size to let
        /// CSS @page { size } take effect (falls back to A4 if neither set).
        #[arg(short, long)]
        size: Option<String>,
```

### Step 2: ヘルプ出力を確認

Run: `cargo run -q -p fulgur-cli --bin fulgur -- render --help 2>&1 | grep -A6 -- "--size"`
Expected: 上記の文言（keyword / WxH / units / priority over CSS @page）が表示される。

### Step 3: コミット

```bash
git add crates/fulgur-cli/src/main.rs
git commit -m "docs(cli): document --size custom format and CSS @page priority"
```

---

## Task 3: CLI end-to-end smoke test（MediaBox 実測 + ヘルプ検証）

**Files:**

- Create: `crates/fulgur-cli/tests/page_size_cli.rs`

### Step 1: テストを書く

既存の `inspect_test.rs` の流儀（`env!("CARGO_BIN_EXE_fulgur")` + `tempfile`）に合わせる:

```rust
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
        .args(["render", "--stdin", "--size", size, "-o", out.to_str().unwrap()])
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
    assert!(child.wait().unwrap().success(), "render failed for --size {size}");
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
    assert!(help.contains("@page"), "help missing @page priority note: {help}");
}
```

### Step 2: テストを実行

Run: `cargo test -p fulgur-cli --test page_size_cli 2>&1 | tail -20`
Expected: 4 テストすべて PASS。

### Step 3: fmt

Run: `cargo fmt -p fulgur-cli`
Expected: 差分なし。

### Step 4: コミット

```bash
git add crates/fulgur-cli/tests/page_size_cli.rs
git commit -m "test(cli): end-to-end custom --size MediaBox + help coverage"
```

---

## Task 4: 最終検証

### Step 1: fulgur-cli 全テスト

Run: `cargo test -p fulgur-cli 2>&1 | tail -15`
Expected: 全 PASS（unit + 既存 integration + 新規 page_size_cli）。

### Step 2: 手動確認（障害寸法での回帰確認）

Run:

```bash
echo '<html><body>x</body></html>' | \
  cargo run -q -p fulgur-cli --bin fulgur -- render --stdin --size 2352.6ptx3481.39pt -o /tmp/cs.pdf
grep -a MediaBox /tmp/cs.pdf | head -1
```

Expected: `/MediaBox [0 0 2352.6 3481.39]`

### Step 3: clippy / fmt 最終

Run: `cargo clippy -p fulgur-cli 2>&1 | tail -5 && cargo fmt --check -p fulgur-cli`
Expected: 警告なし、fmt 差分なし。

---

## Notes

- `parse_page_size` は private のため unit test は `main.rs` 内 `#[cfg(test)] mod tests` に置く（integration test からは見えない）。CLI パース経路は coverage が VRT に乗らないため lib(bin) 側と CLI integration 側の両方に置く方針（CLAUDE.md の coverage scope 注記）。
- `unit_to_pt` の係数は `gcpm::parser::css_unit_to_pt` と完全一致。将来共通化するなら fulgur 側を `pub(crate)` ではなく crate 公開 API に昇格させる必要があり、5 行のためにそれをするのは過剰と判断（YAGNI）。乖離防止のためコメントで参照元を明記済み。
- 優先順位（CLI > CSS > default）は変更しない。`--size` を渡した時点で CSS `@page` が抑制されるのは既存挙動であり、ヘルプにその旨を明記することで discoverability を担保する。
