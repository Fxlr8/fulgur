# P4g: Type GCPM margin-box measure pipeline to `Pt` (fulgur-sipv.7)

**Goal:** Eliminate the 1 `px_to_pt` + 3 `pt_to_px` calls in the GCPM
margin-box measure/render pipeline by typing `MarginBoxRect`, the measure
caches, and the edge-distribution math to `units::Pt`. Byte-neutral.

**Architecture:** `get_body_child_dimension` returns a measured dimension
(`Pt`); it flows through `MeasureCache`/`height_cache` and `edge_defined`
into `compute_edge_layout`, which builds `MarginBoxRect`s. Because
`compute_edge_layout` adds positions to sizes (`o_center = o_first + cs`),
positions and sizes must share a unit — so **all four** `MarginBoxRect`
fields become `Pt` (not just width/height). The struct is genuinely
page-space pt; no framing correction (unlike sipv.6 which was actually
`Px`). Full design is on beads issue `fulgur-sipv.7` (`bd show`).

**Tech Stack:** Rust, `units::{Px, Pt, F32Units}` newtypes, VRT byte-wise
PDF golden comparison.

**Scope boundary:** The measure-stage viewport feeds
(`pt_to_px(content_width)` / `pt_to_px(page_size.height)` /
`pt_to_px(fixed_width)`) read config values, not `MarginBoxRect`, and stay
untouched — they belong to P4h (fulgur-sipv.8). Only the render-stage feed
that reads `rect.width` / `rect.height` is in scope here.

**Byte-neutrality invariant:** `px_to_pt(v) = v * PX_TO_PT` is identical to
`Px::in_pt`; `pt_to_px(v) = v / PX_TO_PT` is identical to `Pt::in_px`.
Preserve operand order everywhere: keep `2.0 * third_w` (do not flip to
`third_w * 2.0`), keep left-associative subtraction chains, keep `/ 3.0`.
Never run `FULGUR_VRT_UPDATE=1`.

---

## Task 1: Type `margin_box.rs` + `render.rs` together

Both files must change in one commit: retyping `MarginBoxRect` breaks
`render.rs` simultaneously, so they only compile together.

**Files:**

- Modify: `crates/fulgur/src/gcpm/margin_box.rs`
- Modify: `crates/fulgur/src/render.rs`

**Step 1 — `margin_box.rs`:**

- Add `use crate::units::{F32Units, Pt};` at the top.
- `MarginBoxRect`: `x`/`y`/`width`/`height` from `f32` to `Pt`.
- `bounding_rect`: tag every `page_size.*` / `margin.*` read with
  `.as_pt()`; replace `0.0` field literals with `Pt::ZERO`. All 16 arms.
  `content_width` / `content_height` / `third_w` / `third_h` become `Pt`.
- `flex_distribute`: params `a_max`/`b_max`/`available` and return to `Pt`;
  `total == 0.0` to `total == Pt::ZERO`. `a_factor = a_max / total` stays
  `f32` (Pt/Pt ratio).
- `distribute_sizes`: `Option<f32>` to `Option<Pt>`, `available` to `Pt`,
  `unwrap_or(0.0)` to `unwrap_or(Pt::ZERO)`.
- `compute_edge_layout`: `defined: &BTreeMap<MarginBoxPosition, Pt>`; the
  `(available, fixed_origin, cross_origin, cross_extent)` tuple all `Pt`
  (`0.0` cross-origins to `Pt::ZERO`); `make_rect` closure params
  `|offset: Pt, size: Pt|`.

**Step 2 — `render.rs`:**

- `type MeasureCache = HashMap<(String, u32, u32), Pt>;`
- `MarginBoxRenderer.height_cache: HashMap<(String, u32, u32), Pt>`.
- `get_body_child_dimension(...) -> Pt`; body
  `crate::convert::px_to_pt(px)` to `px.as_px().in_pt()` (local `px: f32`
  stays f32). Update the doc comment that says "Returned value is in PDF pt"
  to reflect the `Pt` type.
- `edge_defined: BTreeMap<Edge, BTreeMap<MarginBoxPosition, Pt>>`.
- Consumers:
  - `width_key(rect.width.to_f32())`, `width_key(rect.height.to_f32())`.
  - render-stage feed: `pt_to_px(rect.width)` to `rect.width.in_px().to_f32()`
    and both `pt_to_px(rect.height)` to `rect.height.in_px().to_f32()`.
  - `paint_root_block_v2(..., rect.x.to_f32(), rect.y.to_f32(), None)`.
  - `draw_v2_page(..., rect.x.to_f32(), rect.y.to_f32(), ...)`.
- Ensure `crate::units::{F32Units, Pt}` are in scope where used (module
  already imports `F32Units`; qualify `Pt` or add to the import).

**Step 3 — update existing tests in `margin_box.rs`** so they compile with
`Pt` args (pass `<lit>.as_pt()`, compare with `.to_f32()` **inside the hot
condition**, never in a cold message-arg):

- `test_bounding_rect_*`, `test_flex_distribute_*`, `test_distribute_*`,
  `test_compute_edge_layout_*`.

**Step 4 — build:**

Run: `cargo build -p fulgur`
Expected: clean compile, no `px_to_pt`/`pt_to_px` left in `margin_box.rs`
and the 4 targeted ones gone from the margin-box render path.

**Step 5 — commit** (worktree; source edits need no `--sparse` after
`sparse-checkout disable`):

```bash
git add crates/fulgur/src/gcpm/margin_box.rs crates/fulgur/src/render.rs
git commit -m "refactor(fulgur): type GCPM margin-box measure pipeline to units::Pt"
```

---

## Task 2: Backfill coverage for changed arms

The migration turns all 16 `bounding_rect` arms and all 4
`compute_edge_layout` edge arms into changed lines. Current unit tests hit
only 3/16 positions and `Edge::Top` only; the integration path uses
`make_rect`, not the `bounding_rect` fallback. Add non-VRT unit tests so
patch coverage stays at 0 uncovered.

**Files:**

- Modify: `crates/fulgur/src/gcpm/margin_box.rs` (`#[cfg(test)] mod tests`)

**Step 1 — add an all-16-position `bounding_rect` test** iterating every
`MarginBoxPosition`, asserting each rect's four fields against the expected
page-space geometry (compare `.to_f32()` in the hot condition).

**Step 2 — add `compute_edge_layout` tests for `Edge::Bottom`,
`Edge::Left`, `Edge::Right`** (mirror the existing `Edge::Top` tests:
center-only and first+last), asserting cross-axis origin/extent and
primary-axis distribution.

**Step 3 — run the unit tests:**

Run: `cargo test -p fulgur --lib gcpm::margin_box`
Expected: PASS.

**Step 4 — commit:**

```bash
git add crates/fulgur/src/gcpm/margin_box.rs
git commit -m "test(fulgur): cover all bounding_rect positions + compute_edge_layout edges"
```

---

## Task 3: Full verification (epic protocol)

**Step 1 — build + lint + fmt:**

```bash
cargo build -p fulgur
cargo clippy -p fulgur --all-targets -- -D warnings
cargo fmt --check
```

Expected: all clean. (`cargo fmt` may single-line shortened asserts — run
`cargo fmt` then re-check.)

**Step 2 — lib + render_smoke + gcpm tests:**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur --lib
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur --test render_smoke --test gcpm_integration --test gcpm_snapshot
```

Expected: all PASS.

**Step 3 — VRT byte-identity (the byte-neutral gate):**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt
git status --porcelain crates/fulgur-vrt/goldens
```

Expected: VRT green **and** `git status` on goldens empty (no golden byte
changed). Never `FULGUR_VRT_UPDATE=1`.

**Step 4 — patch-coverage gate (measured, not reasoned):**

```bash
cargo llvm-cov nextest --workspace --exclude fulgur-vrt --lcov --output-path lcov.info
```

Intersect the added lines from `git diff origin/main` with `DA:N,0`
(uncovered) entries in `lcov.info`. Target: 0 uncovered added PROD lines;
any residual should be a dev-only assert artifact addressed per the
`{:?}`/single-line/hot-binding workaround.

**Step 5 — request code review** (`superpowers:requesting-code-review`),
then finish the branch (`superpowers:finishing-a-development-branch`).
