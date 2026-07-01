# Transform Affine2D + Point2 を typed units に移行 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** `Affine2D.{e,f}`（transform matrix の translate 成分）と `Point2.{x,y}`（transform-origin）を bare `f32` / legacy `type Pt = f32` alias から `units::Pt` newtype に移行し、現挙動を 1 byte も変えない（byte-neutral）。

**Architecture:** transform pipeline は実体として全部 pt 空間で self-consistent に動作している（`compute_transform` は pt dims を渡され、render は ×0.75 せず pt として直接消費）。「fold は存在しない」と認め、translate 成分と origin を `Pt` で型付けする。`a,b,c,d` は無次元のため f32 据え置き。ripple は `transform_point`/`to_krilla`/FFI 境界で `.to_f32()` untag、producer 境界で `.pt()` tag に封じ込める。

**Tech Stack:** Rust, `crate::units::{Pt, F32Units}`（`#[repr(transparent)]` newtype, 演算子 impl, `Pt::ZERO`/`Pt::abs`）, krilla, Stylo computed values。

---

## 不変条件（全 Task で厳守）

1. **byte-neutral**: `.in_pt()` / ×0.75 を一切入れない。`compute_transform` の dims 入力（`size_in_pt` = pt）を変えない。Stylo の `.resolve(..).px()`（f32 抽出）に `.pt()` を足すだけ（tag であって変換ではない）。
2. **Mul / 式構造を 1:1 保持**: `Affine2D::Mul` の式を再結合しない（f32 丸めが変わる）。`f32 * Pt → Pt`（commutative impl）、`Pt + Pt → Pt` に自然に乗る。
3. **「goldens pass ≠ behavior 不変」**: 絶対長 translate(例 `translate(20px)`)は golden 未カバーの可能性が高い。`.in_pt()` を 1 箇所でも誤って入れると goldens 緑のまま behavior が変わる。**証明は VRT golden の byte-identical**（`FULGUR_VRT_UPDATE` なし）。
4. 潜在 4/3 バグ（絶対長 translate の過大シフト）は**この issue では是正しない**。Task 4 で別 issue 起票。

---

## Task 1: `Point2` を `units::Pt` に移行

`Point2` は transform-origin 専用。legacy `type Pt = f32` alias から `units::Pt` に切替。`Affine2D` はこの Task では f32 のまま（独立にコンパイル可能）。

**Files:**
- Modify: `crates/fulgur/src/draw_primitives.rs:229-239`（`Point2` struct + `Point2::new`）
- Modify: `crates/fulgur/src/blitz_adapter.rs:2913-2920`（`compute_transform` の origin 構築）
- Modify: `crates/fulgur/src/render.rs:1325-1326`（`draw_under_transform` の `tx.origin` 読み出し）
- Modify: `crates/fulgur/src/render.rs:4621`（test default `Point2 { x:0.0, y:0.0 }`）

**Step 1: `Point2` を units::Pt に**

`draw_primitives.rs` の `Point2`（現在 `pub x: Pt` = legacy alias）を:

```rust
/// A 2D point in user-space coordinates (PDF pt).
///
/// Used only for `transform-origin` (`drawables::TransformEntry.origin`).
/// The value is the box-local origin already in pt space — `compute_transform`
/// is fed pt-valued box dims, so no px→pt fold happens. **Do not add
/// `.in_pt()`**: that would scale an already-pt value by 0.75.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point2 {
    pub x: crate::units::Pt,
    pub y: crate::units::Pt,
}

impl Point2 {
    pub const fn new(x: crate::units::Pt, y: crate::units::Pt) -> Self {
        Self { x, y }
    }
}
```

**Step 2: `compute_transform` の origin を tag**

`blitz_adapter.rs:2913-2920` — `.px()`（Stylo の f32 抽出）に `.pt()` を足す。`use crate::units::F32Units;` を関数スコープか module 先頭に追加:

```rust
    let origin = styles.clone_transform_origin();
    let origin_x = origin.horizontal.resolve(Length::new(border_box_width)).px().pt();
    let origin_y = origin.vertical.resolve(Length::new(border_box_height)).px().pt();

    Some((m, Point2::new(origin_x, origin_y)))
```

（`use crate::units::F32Units;` を `compute_transform` 冒頭に追加。`.px()` は Stylo `Length` の inherent method、`.pt()` は f32 の trait method で衝突しない。）

**Step 3: `draw_under_transform` の origin 読み出しを untag**

`render.rs:1325-1326`:

```rust
    let ox = x_pt + tx.origin.x.to_f32();
    let oy = y_pt + tx.origin.y.to_f32();
```

**Step 4: test default を修正**

`render.rs:4621`（private field のため struct literal 不可）:

```rust
                origin: crate::draw_primitives::Point2::new(
                    crate::units::Pt::ZERO,
                    crate::units::Pt::ZERO,
                ),
```

**Step 5: ビルド**

Run: `cargo build -p fulgur`
Expected: コンパイル成功（他の Point2 利用箇所が無いことは grep 済み）

**Step 6: transform テスト**

Run: `cargo test -p fulgur --test transform_integration`
Expected: 全 PASS（origin の値は不変なので byte-neutral）

**Step 7: コミット**

```bash
git add crates/fulgur/src/draw_primitives.rs crates/fulgur/src/blitz_adapter.rs crates/fulgur/src/render.rs
git commit -m "refactor(transform): type Point2 (transform-origin) with units::Pt (byte-neutral)"
```

---

## Task 2: `Affine2D.{e,f}` を `units::Pt` に移行

`a,b,c,d` は無次元 f32 据え置き、`e,f`（translate）を `units::Pt` に。

**Files:**
- Modify: `crates/fulgur/src/draw_primitives.rs:98-223`（`Affine2D` struct + impl + Mul）
- Modify: `crates/fulgur/src/blitz_adapter.rs:2923-2968`（`op_to_matrix`: translation/Matrix arm）
- Modify: `crates/fulgur/src/render.rs:1328`（`draw_under_transform` の translation 構築）
- Modify: `crates/fulgur/src/paragraph.rs:839`（image link の translation）
- Modify: `crates/fulgur/src/draw_primitives.rs:1920-1926`（既存 Affine2D 単体テスト）
- Verify-only: `crates/fulgur/src/link.rs`（LinkCollector の Affine2D 合成は Mul 経由で自動対応、変更不要のはず）

**Step 1: `Affine2D` struct + impl を型付け**

`draw_primitives.rs`:

```rust
pub struct Affine2D {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    /// Translate-x. **pt 空間の値**（px→pt fold の結果ではなく、pt-basis hack の
    /// 出力を pt として扱っているだけ）。**`.in_pt()` を足すな** — byte が変わる。
    pub e: crate::units::Pt,
    pub f: crate::units::Pt,
}

impl Affine2D {
    pub const IDENTITY: Self = Self {
        a: 1.0, b: 0.0, c: 0.0, d: 1.0,
        e: crate::units::Pt::ZERO,
        f: crate::units::Pt::ZERO,
    };

    const IDENTITY_EPS: f32 = 1e-5;

    pub fn is_identity(&self) -> bool {
        (self.a - 1.0).abs() < Self::IDENTITY_EPS
            && self.b.abs() < Self::IDENTITY_EPS
            && self.c.abs() < Self::IDENTITY_EPS
            && (self.d - 1.0).abs() < Self::IDENTITY_EPS
            && self.e.to_f32().abs() < Self::IDENTITY_EPS
            && self.f.to_f32().abs() < Self::IDENTITY_EPS
    }

    pub fn translation(tx: crate::units::Pt, ty: crate::units::Pt) -> Self {
        Self { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: tx, f: ty }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self { a: sx, b: 0.0, c: 0.0, d: sy, e: crate::units::Pt::ZERO, f: crate::units::Pt::ZERO }
    }

    pub fn rotation(theta_rad: f32) -> Self {
        let (s, c) = theta_rad.sin_cos();
        Self { a: c, b: s, c: -s, d: c, e: crate::units::Pt::ZERO, f: crate::units::Pt::ZERO }
    }

    pub fn skew(ax_rad: f32, ay_rad: f32) -> Self {
        Self { a: 1.0, b: ay_rad.tan(), c: ax_rad.tan(), d: 1.0, e: crate::units::Pt::ZERO, f: crate::units::Pt::ZERO }
    }

    pub fn to_krilla(&self) -> krilla::geom::Transform {
        krilla::geom::Transform::from_row(self.a, self.b, self.c, self.d, self.e.to_f32(), self.f.to_f32())
    }

    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.e.to_f32(),
            self.b * x + self.d * y + self.f.to_f32(),
        )
    }
    // transform_rect は transform_point を呼ぶだけ → 変更不要
}
```

**Step 2: `Mul` impl — 式構造 1:1 保持**

`draw_primitives.rs:213-222`。`a,c` は f32, `rhs.e,rhs.f,self.e,self.f` は Pt。`f32 * Pt → Pt`、`Pt + Pt → Pt` に自然に乗る。**式を一切いじらない**:

```rust
            e: self.a * rhs.e + self.c * rhs.f + self.e,
            f: self.b * rhs.e + self.d * rhs.f + self.f,
```

（a,b,c,d の行はそのまま f32 演算。)

**Step 3: `op_to_matrix` を tag**

`blitz_adapter.rs:2940-2945`。`use crate::units::F32Units;`（Task 1 で追加済みなら不要）。`.px()`（Stylo f32 抽出）に `.pt()`:

```rust
        Translate(x, y) => Affine2D::translation(
            x.resolve(Length::new(w)).px().pt(),
            y.resolve(Length::new(h)).px().pt(),
        ),
        TranslateX(x) => Affine2D::translation(x.resolve(Length::new(w)).px().pt(), 0.0_f32.pt()),
        TranslateY(y) => Affine2D::translation(0.0_f32.pt(), y.resolve(Length::new(h)).px().pt()),
```

`Matrix(m)` arm（`m.e`,`m.f` は f32）:

```rust
        Matrix(m) => Affine2D { a: m.a, b: m.b, c: m.c, d: m.d, e: m.e.pt(), f: m.f.pt() },
```

**Step 4: `draw_under_transform` の translation を tag**

`render.rs:1328`（`ox`,`oy` は f32 local のまま、引数を tag）:

```rust
    let full = Affine2D::translation(ox.pt(), oy.pt()) * tx.matrix * Affine2D::translation((-ox).pt(), (-oy).pt());
```

**Step 5: paragraph.rs の image translation を tag**

`paragraph.rs:839`:

```rust
                        let link_affine =
                            crate::draw_primitives::Affine2D::translation(off_x.pt(), off_y.pt());
```

**Step 6: 既存 Affine2D 単体テストを typed API に更新**

`draw_primitives.rs:1925-1926`（`composed.e - 4.0` → untag）:

```rust
        assert!((composed.e.to_f32() - 4.0).abs() < 1e-5);
        assert!((composed.f.to_f32() - 2.0).abs() < 1e-5);
```

その他の Affine2D テスト（`translation(..)` 呼び出し、`.e`/`.f` 比較）も同様に `0.0_f32.pt()` で構築、`.to_f32()` で untag。`cargo build --tests` のエラーに沿って機械的に修正。

**Step 7: ビルド（lib + tests）**

Run: `cargo build -p fulgur --tests`
Expected: コンパイル成功。`link.rs` が変更不要でコンパイルできることを確認（LinkCollector の Affine2D 合成は Mul で自動対応）。エラーが出たら untag(`.to_f32()`)/tag(`.pt()`)を該当箇所に追加。

**Step 8: lib + integration テスト**

Run: `cargo test -p fulgur`
Expected: 全 PASS（Affine2D の math は repr(transparent) + inline で f32 と同一機械語、値不変）

**Step 9: コミット**

```bash
git add crates/fulgur/src/draw_primitives.rs crates/fulgur/src/blitz_adapter.rs crates/fulgur/src/render.rs crates/fulgur/src/paragraph.rs
git commit -m "refactor(transform): type Affine2D translate components (e,f) with units::Pt (byte-neutral)"
```

---

## Task 3: docs 訂正 + footgun note + byte-neutral 証明

**Files:**
- Modify: `.claude/rules/coordinate-system.md`（`compute_transform` exception 節 — stale）

**Step 1: coordinate-system.md の stale 記述を訂正**

`compute_transform` の節は「px-in / px-out、convert で後段 fold」と書いてあるが、実体は「pt dims を渡し、render は ×0.75 せず pt 消費」。実態に合わせて訂正し、translate `e,f` と origin が pt-basis hack の出力（fold は存在しない）であること、`.in_pt()` を足すと 4/3 になることを明記。絶対長 translate の 4/3 潜在バグを別 issue として参照。

**Step 2: fmt + clippy**

Run: `cargo fmt -p fulgur` then `cargo fmt --check`
Run: `cargo clippy -p fulgur --all-targets`
Expected: フォーマット整合、clippy 警告なし

**Step 3: codecov patch coverage 確認**

`transform_integration.rs`（通常の integration test、codecov 対象）が `compute_transform` / `draw_under_transform` の typed 経路を踏むことを確認。新たに公開された typed surface（`Affine2D::translation(Pt,Pt)` 等）が lib 側で未カバーなら、`draw_primitives.rs` の `#[cfg(test)]` に unit test を追加。`.to_f32()` を assert 引数に直書きすると patch coverage の region アーティファクトになる（`project_units_migration_patch_coverage.md`）ので、値を変数に束縛してから assert する。

**Step 4: byte-neutral 証明（load-bearing）**

Run: `FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt`
Expected: 全 PASS（**`FULGUR_VRT_UPDATE` は付けない**）。golden 差分が出たら byte-neutral 違反 = どこかで `.in_pt()` を誤って入れた or Mul を再結合した。

Run: `cargo test -p fulgur-cli --test examples_determinism`（存在すれば）
Expected: PASS（run-twice 自己比較。nondeterminism 検出用）

**Step 5: コミット**

```bash
git add .claude/rules/coordinate-system.md crates/fulgur/src/draw_primitives.rs
git commit -m "docs(coordinate-system): correct compute_transform px/pt note; transform fold is pt-basis hack not a fold"
```

---

## Task 4: 4/3 絶対長 translate 是正の follow-up issue 起票

byte-neutral スコープ外の behavior 修正を別 issue に切り出す。

**Step 1: issue 起票**

```bash
bd create --title="Fix 4/3 over-translation for absolute-length CSS transform translate" \
  --type=bug --priority=2 \
  --description="compute_transform is fed pt-valued box dims and Stylo's .px() extraction is consumed as pt without x0.75. % translate and 50% origin round-trip correctly, but absolute-length translate (e.g. translate(20px)) shifts 20pt = 26.67px instead of the correct 20px = 15pt — a 4/3 over-shift. Fixing requires feeding px dims and folding px->pt, which changes PDF bytes (golden/VRT updates). Spun out of fulgur-1ino (byte-neutral typing migration)."
```

（`--description` は eval 経由なので backtick を含めない / 必要ならファイル経由。）

**Step 2: 確認**

Run: `bd show <new-id>`
Expected: issue 作成確認

---

## 完了条件（受け入れ基準）

- VRT golden が `FULGUR_VRT_UPDATE` なしで byte-identical
- `cargo test -p fulgur` 全 PASS、`cargo clippy` / `cargo fmt --check` clean
- `draw_primitives::Pt`(legacy alias) の `Point2` 利用が消えた（`Affine2D` も alias 非依存）
- `coordinate-system.md` の compute_transform 記述が実態と一致
- 4/3 是正の follow-up issue が起票済み
