# P1e: Migrate Drawables Aggregate Fields to units::Pt — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task (chosen execution mode).

**Goal:** Retype the remaining raw-`f32` PDF-pt coordinate fields on `Drawables` entry structs to `units::Pt`, byte-neutral (PDF output unchanged), completing the per-field side of epic `fulgur-2map`'s coordinate migration (5th in the P1a–P1d series).

**Architecture:** Type each field as its native stored unit (`units::Pt`); convert only at genuine boundaries (`px_to_pt` producers tag with `.pt()`, FFI drops to `f32` via `.to_f32()`); never reassociate arithmetic — preserve exact `f32` op order at every site so the bytes are identical. One struct/field-group per task, ascending friction, each independently golden-verified and committed.

**Tech Stack:** Rust, `crate::units::{Px, Pt}` (newtypes over `f32` with `Add/Sub/Mul<f32>/Neg/Sum/ZERO/max/min/abs`), VRT byte-wise PDF golden compare (`cargo test -p fulgur-vrt`), `examples_determinism`.

---

## Ground rules (apply to EVERY task)

- **Byte-neutral pattern** (from spike fulgur-2map.2 / P1a–P1d):
  - `px_to_pt(v)` feeding a now-`Pt` field → `v.px().in_pt()` (one multiply, NO reassociation), or tag an already-pt `f32` with `.pt()`.
  - At arithmetic sites, `Pt + Pt`, `Pt - Pt`, `Pt * f32`, `Pt / f32`, `.max(Pt::ZERO)`, `(a-b).abs()` via `Pt::abs`.
  - Drop to `f32` with `.to_f32()` ONLY at: krilla/tiny-skia FFI draw calls, `draw_primitives::Rect` (still f32), `Affine2D` (still f32), `skrifa` size.
  - Preserve operand order; do not factor or combine terms.
- **NEVER** run `FULGUR_VRT_UPDATE=1`. Goldens must stay byte-identical.
- **Per-task verification** (the byte-neutral proof — this replaces "write a failing test"):
  1. `cargo build -p fulgur` — clean.
  2. `cargo clippy -p fulgur -- -D warnings` — clean.
  3. `cargo fmt --check` — clean (run `cargo fmt` if needed).
  4. `cargo test -p fulgur --lib` — green.
  5. `cargo test -p fulgur --test render_smoke` — green.
  6. `FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt` — `run_fulgur_vrt` ok (goldens byte-identical).
- **Baseline already captured GREEN** on clean main (worktree HEAD d6903b08): VRT ok, examples_determinism 11, lib 1497, render_smoke 171. Any red after an edit attributes to that edit.
- **Patch-coverage recurrence** (memory `project_units_migration_patch_coverage`, names P1c/**P1e**): typed-arg assert regions on `.to_f32()` drop codecov/patch. When adding/adjusting assertions, prefer `{:?}` debug-format + bare args / single-line / hot-path binding; cover PROD draw paths with behavioral `render_smoke` tests (the coverage job excludes fulgur-vrt). Handle test-fixture construction sites listed per task so nothing surfaces mid-impl.
- **Working directory:** `/home/ubuntu/fulgur/.worktrees/fulgur-2map.7-drawables-aggregate-px-pt`.

---

## Task 1: TableEntry.width / cached_height → Pt (re-prove the bridge cheaply)

**Why first:** smallest consumer surface, unit unambiguously pt (`size_in_pt`).

**Files:**

- Modify: `crates/fulgur/src/drawables.rs` — `TableEntry.width: f32 → Pt`, `cached_height: f32 → Pt` (lines ~141-142).
- Modify: `crates/fulgur/src/convert/table.rs` — `convert_table`: `let (width, height) = size_in_pt(...)`. `size_in_pt` already returns `Pt`; drop the `.pt()` on `layout_size` (`width: width`, `height: height`) and feed `width`/`cached_height: height` directly. Verify `size_in_pt` return type first; if it returns `(Pt,Pt)` the `.pt()` calls become redundant, if it returns `(f32,f32)` keep `.pt()` on `width`/`cached_height`.
- Modify: `crates/fulgur/src/render.rs` — `table_box_size` fallback (~2673-2690): `entry.cached_height` now `Pt`; the `frag.height` zero-guard compare and the returned size. Drop `.to_f32()` at the krilla draw boundary.
- Test fixtures: `render.rs:~4700` (`cached_height: 100.0`), `~5207` (`cached_height: 50.0`), `~4741/4750` (helper signature) → `.pt()` / `Pt`.

**Step 1:** Verify `size_in_pt` signature: `grep -n "fn size_in_pt" crates/fulgur/src/convert/mod.rs` and read its return type.

**Step 2:** Retype the two struct fields in `drawables.rs`.

**Step 3:** Fix `convert/table.rs` construction (see Files note re `size_in_pt`).

**Step 4:** Fix `render.rs` `table_box_size` consumers + the `Drawables::is_empty`/`Default` are unaffected (no coord). Fix test fixtures.

**Step 5:** Run the full per-task verification (build / clippy / fmt / lib / render_smoke / VRT). All green + goldens byte-identical.

**Step 6:** Commit.

```bash
git add -A && git commit -m "refactor(table): type TableEntry width/cached_height with units::Pt (byte-neutral, P1e)"
```

---

## Task 2: ImageEntry + SvgEntry width/height → Pt

**Why together:** structurally parallel, share `convert/replaced.rs` + `convert/list_marker.rs` construction and the render image/svg draw consumers.

**Files:**

- Modify: `crates/fulgur/src/drawables.rs` — `ImageEntry.width/height`, `SvgEntry.width/height` (`f32 → Pt`).
- Modify: `crates/fulgur/src/convert/replaced.rs`:
  - `make_image_entry` returns `ImageEntry` with `width: w, height: h` where `(w,h) = resolve_image_dimensions(...)`. `resolve_image_dimensions` returns `(f32,f32)` (intrinsic device-px fallback in `(None,None)`, prod path = pt css dims). KEEP `resolve_image_dimensions` returning `f32` (it mixes a device-px branch), tag at the `ImageEntry` construction: `width: w.pt(), height: h.pt()`. Document that the `(None,None)` branch is **production-reachable** via auto-sized pseudo `content: url()` (`resolve_pseudo_size` → `None` for `auto`), NOT test-only — so `.pt()` there preserves a device-px value verbatim (byte-neutral); the device-px-vs-pt provenance is a pre-existing question tracked in `fulgur-t82j`.
  - `convert_svg`: `width: content_w, height: content_h` from `maybe_insert_block_for_replaced` (returns `(f32,f32)` pt) → `.pt()`.
  - `make_image_entry` tests (`test_make_image_entry_*`, ~254-310): assertions on `img.width`/`img.height` → `.to_f32()` on the `Pt` (or compare against `Pt`). Use `{:?}`/bare-arg form for patch coverage.
- Modify: `crates/fulgur/src/convert/list_marker.rs` — the `ImageEntry`/`SvgEntry` built from `clamp_marker_size` (already `(Pt,Pt)`): `size_raster_marker` returns `(width,height)`; check its type. If `clamp_marker_size` yields `Pt`, feed directly; the inner `ImageEntry { width, height }` now wants `Pt` — align `size_raster_marker`'s return or `.pt()` at use.
- Modify: `crates/fulgur/src/render.rs` — image draw (`~1041/1091/1663/2051`) and svg draw consumers: scale by `width`/`height` then `.to_f32()` at krilla draw. Test fixture `render.rs:~5348-5353` (`width: 10.0, height: 10.0`) → `.pt()`.
- Modify: `crates/fulgur/src/convert/pseudo.rs` — `build_pseudo_image_entry`/`build_inline_pseudo_image` paths that read `parent_cb.width` (taffy px, NOT the entry field) are unaffected; only the `ImageEntry`/`InlineImage` field reads change. Distinguish carefully: `LineItem::Image(i).width` is `InlineImage` (P1b, already Pt) — do NOT touch.

**Step 1:** `grep -n "fn size_raster_marker\|fn clamp_marker_size" crates/fulgur/src/convert/list_marker.rs crates/fulgur/src/draw_primitives.rs` — confirm return types.

**Step 2:** Retype both struct field pairs in `drawables.rs`.

**Step 3:** Fix `replaced.rs` (make_image_entry + convert_svg) construction with `.pt()` tags.

**Step 4:** Fix `list_marker.rs` construction.

**Step 5:** Fix `render.rs` image/svg draw consumers (`.to_f32()` at FFI) + test fixtures + `make_image_entry` unit tests.

**Step 6:** Full per-task verification — green + goldens byte-identical. Add a behavioral `render_smoke` test only if a new PROD branch became uncovered (e.g. an `<img>`/`<svg>` sizing smoke) per patch-coverage rule.

**Step 7:** Commit.

```bash
git add -A && git commit -m "refactor(replaced): type ImageEntry/SvgEntry width/height with units::Pt (byte-neutral, P1e)"
```

---

## Task 3: ListItemMarker widths + ListItemEntry.marker_line_height → Pt

**Files:**

- Modify: `crates/fulgur/src/drawables.rs`:
  - `ListItemMarker::Text { width: f32 → Pt }`, `ListItemMarker::Image { width: f32 → Pt, height: f32 → Pt }` (~164-175).
  - `ListItemEntry.marker_line_height: f32 → Pt` (~187).
  - `ListItemEntry` `Debug` impl (~192-200): keep `.field("marker_line_height", &self.marker_line_height)` — `Pt: Debug`, fine.
  - Test fixtures in `drawables.rs` tests: `marker_line_height: 12.0` (~518), `ListItemMarker::Text { width: 0.0 }` (~514) → `.pt()`.
- Modify: `crates/fulgur/src/convert/list_item.rs` — `extract_marker_lines` returns `(Vec<ShapedLine>, f32, f32)` = `(lines, marker_width, marker_line_height)`; both are pt (`px_to_pt`). Change return to `(.., Pt, Pt)` OR `.pt()` at the `ListItemEntry`/`ListItemMarker::Text` construction (~28-44, ~98). Prefer typing `extract_marker_lines`'s return for source-truth.
- Modify: `crates/fulgur/src/convert/list_marker.rs` — `extract_marker_lines` body: `line_width += px_to_pt(g.advance)` → `Pt` accumulation (`line_width: Pt`, init `Pt::ZERO`); `line_height_pt = px_to_pt(metrics.line_height)` → `Pt`. `resolve_list_marker(node, marker_line_height, ...)` signature: `line_height: f32 → Pt` (it forwards to `clamp_marker_size`/`size_raster_marker` which want `Pt`).
- Modify: `crates/fulgur/src/render.rs` — `~3083-3092`: `ListItemMarker::Text { lines, width }` and `Image { width, height }` now `Pt`; `marker_y = y + (entry.marker_line_height - *height) / 2.0` = `Pt + (Pt - Pt)/f32` = `Pt`; `.to_f32()` at draw. `ListItemMarker::Text` width used for marker x-advance → keep `Pt` until FFI.

**Step 1:** Map `extract_marker_lines` / `resolve_list_marker` / `size_raster_marker` signatures and their `line_height` threading.

**Step 2:** Retype struct fields + variants in `drawables.rs`; fix test fixtures.

**Step 3:** Thread `Pt` through `list_marker.rs` (extract_marker_lines accumulation, resolve_list_marker signature) and `list_item.rs` construction.

**Step 4:** Fix `render.rs` marker draw (`marker_y` arithmetic, `.to_f32()` at draw).

**Step 5:** Full per-task verification — green + goldens byte-identical.

**Step 6:** Commit.

```bash
git add -A && git commit -m "refactor(list): type ListItemMarker/marker_line_height with units::Pt (byte-neutral, P1e)"
```

---

## Task 4: ParagraphSlice.origin_pt / size_pt → (Pt, Pt) [owner-confirmed in scope]

**Files:**

- Modify: `crates/fulgur/src/drawables.rs` — `ParagraphSlice.origin_pt: (f32,f32) → (Pt,Pt)`, `size_pt: (f32,f32) → (Pt,Pt)` (~258-260). `Debug` impl (~266-273) keeps `.field("origin_pt", &self.origin_pt)` — `(Pt,Pt): Debug`, fine.
- Modify: `crates/fulgur/src/convert/` multicol slice producer — find where `ParagraphSlice { origin_pt, size_pt }` is built (`grep -rn "ParagraphSlice {" crates/fulgur/src`). Source is `multicol_layout` pt geometry (P1d migrated `ColumnLineSlice` to `Px`; the slice origin/size are pt). Tag with `.pt()` or feed `Pt`.
- Modify: `crates/fulgur/src/render.rs` — the paragraph-slices dispatcher reads `slice.origin_pt`/`size_pt`, adds the container body-relative position (`Pt`), `.to_f32()` at draw. `grep -rn "origin_pt\|size_pt\|paragraph_slices" crates/fulgur/src/render.rs`.

**Step 1:** `grep -rn "ParagraphSlice {" crates/fulgur/src` and read producer + render consumer.

**Step 2:** Retype the two tuple fields in `drawables.rs`.

**Step 3:** Fix producer construction + render consumer.

**Step 4:** Full per-task verification — green + goldens byte-identical (multicol VRT goldens are the load-bearing proof here).

**Step 5:** Commit.

```bash
git add -A && git commit -m "refactor(multicol): type ParagraphSlice origin/size with units::Pt (byte-neutral, P1e)"
```

---

## Task 5: Drawables.body_offset_pt: (f32,f32) → (Pt,Pt) (highest friction, last)

**Files:**

- Modify: `crates/fulgur/src/drawables.rs` — `body_offset_pt: (f32,f32) → (Pt,Pt)` (~330); `Default` `body_offset_pt: (0.0,0.0) → (Pt::ZERO, Pt::ZERO)` (~407); `is_empty` doc-comment unaffected.
- Modify: `crates/fulgur/src/convert/mod.rs` — `extract_body_offset_pt(doc) -> (f32,f32)` (~231) → `(Pt,Pt)`; its `body_layout.location.x/y` are pt (verify; `px_to_pt` or already-pt) → `.pt()`/`.px().in_pt()`. Assignment `drawables.body_offset_pt = ...` (~197).
- Modify: `crates/fulgur/src/render.rs` — ~10 sites (`112-116`, `261`, `268`, `275-280`, `838-839`): each `margin.top + body_offset_pt.1`, `desc_y = margin_top_pt + body_offset_pt.1 + px_to_pt(desc_frag.y)`. `resolved_margin.*` and `margin_*_pt` are `Pt` (verify); `px_to_pt(frag.x/y)` → `.pt()` to keep `Pt + Pt`. Preserve add order.
- Modify: `crates/fulgur/src/paragraph.rs` — `~830/833`: `+ ctx.drawables.body_offset_pt.0/.1` into a `Pt` accumulation.
- `pagination_layout.rs:2644` — doc-comment only, no code change.

**Step 1:** Verify `resolved_margin.top`/`margin_top_pt` and `body_layout.location.x/y` types (`grep`/read).

**Step 2:** Retype field + `Default` in `drawables.rs`.

**Step 3:** Fix `convert/mod.rs` producer (`extract_body_offset_pt`).

**Step 4:** Fix all `render.rs` + `paragraph.rs` consumer sites; preserve operand order; `.pt()` on `px_to_pt(frag.*)` results.

**Step 5:** Full per-task verification — green + goldens byte-identical. body offset touches every multi-page doc — VRT is decisive.

**Step 6:** Commit.

```bash
git add -A && git commit -m "refactor(drawables): type body_offset_pt with units::Pt (byte-neutral, P1e)"
```

---

## Task 6 (optional polish): drop `_pt` suffix on now-typed fields

Only if clean. Mirrors P1b's separate cleanup commit (8424511c after a63f0b18). Rename together for consistency:

- `Drawables.body_offset_pt → body_offset`
- `ParagraphSlice.origin_pt → origin`, `size_pt → size`

Update all readers. Pure rename, byte-neutral. Full verification + commit:

```bash
git add -A && git commit -m "refactor(drawables): drop _pt suffix on Pt-typed body_offset/slice fields (byte-neutral, P1e)"
```

If any rename gets noisy, SKIP — it is not required for acceptance.

---

## Final acceptance (before PR)

- All of: `cargo build`, `cargo clippy -p fulgur -- -D warnings`, `cargo fmt --check` clean.
- `cargo test -p fulgur --lib`, `--test render_smoke` green.
- `FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt` ok; `cargo test -p fulgur-cli --test examples_determinism` 11 passed. Goldens byte-identical throughout (NEVER updated).
- `grep -rn "\.width: f32\|cached_height: f32\|marker_line_height: f32\|body_offset_pt: (f32" crates/fulgur/src/drawables.rs` returns nothing for the migrated fields.
- Close note records the **transforms carve-out**: `TransformEntry` was deliberately NOT migrated. BOTH its fields are `draw_primitives` composite types built on the **legacy `draw_primitives::Pt` (= `type Pt = f32`) alias**, NOT `units::Pt` — `matrix: Affine2D` (e/f translate is the px/pt-fold blocker) AND `origin: Point2 { x: Pt, y: Pt }` (proof: `render.rs:1332` `x_pt + tx.origin.x` is a bare-f32 add, which would not compile if `origin.x` were `units::Pt`). Both are owned by the still-open **fulgur-1ino** and are out of P1e's raw-`f32`-field scope. So P2/fulgur-2map.8 must not read transforms (incl. `origin`) as done. (Earlier drafts wrongly said `origin` was already `units::Pt` — corrected per the final holistic review; the `Size`-vs-`Point2` asymmetry in `draw_primitives.rs`, where `Size` already uses `units::Pt` but `Point2`/`Rect`/`Affine2D` still use the f32 alias, is what made `Point2` look migrated.)
