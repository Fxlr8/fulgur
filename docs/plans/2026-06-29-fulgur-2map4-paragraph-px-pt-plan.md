# P1b: Migrate paragraph types to Px/Pt — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or subagent-driven-development) to implement this plan task-by-task.

**Goal:** Migrate the coordinate fields of `ShapedLine`, `ShapedGlyphRun`, `ShapedGlyph`, `InlineImage`, `InlineBoxItem` from raw `f32` (and the legacy `draw_primitives::Pt = f32` alias) to `crate::units::Pt`, byte-neutrally.

**Architecture:** Follow the spike pattern (fulgur-2map.2 / PR #509): type each field as its native stored unit, call `to_f32()` only at FFI (Krilla, skrifa, the f32 `Rect`/decoration draw helpers), never reassociate float ops. `font_size` becomes `units::Pt` so `EM(f32) * font_size(Pt) = Pt` falls out naturally. Font-metric helpers (`get_decoration_metrics`/`DecorationMetrics`, `LineFontMetrics`) stay `f32` (Strategy C) — skrifa is an f32 FFI boundary. The full per-site method-gap inventory lives in the beads issue `fulgur-2map.4` design field; this plan is the SEQUENCE + verification gates.

**Tech Stack:** Rust, `crate::units` newtypes, Krilla, skrifa, Parley. Determinism guarded by `examples_determinism` + `fulgur-vrt` byte-wise PDF goldens.

**Base branch:** `origin/main` (this worktree). **Baseline already taken GREEN before any edit:** fulgur --lib 1495, examples_determinism 11, VRT `run_fulgur_vrt` ok.

**Byte-neutral law (applies to EVERY task):**
- Preserve f32 operation order; never reassociate. `a + b - c` stays `a + b - c`.
- `units::Pt` ops are byte-transparent: `f32 * Pt = Pt(self * rhs.0)`, `Pt::sum = Pt(map(.0).sum())`, `Pt::max/min` mirror `f32::max/min`, `Pt::abs` mirrors `f32::abs`.
- NEVER run `FULGUR_VRT_UPDATE=1`. Goldens must stay untouched.
- `FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"` for examples_determinism + VRT.

---

### Task 1: Add `Pt::abs()` (and `Px::abs()`) to `units.rs`

**Files:**
- Modify: `crates/fulgur/src/units.rs` (impl Pt around :151-181; impl Px around :137-149)
- Test: `crates/fulgur/src/units.rs` `#[cfg(test)] mod tests`

**Step 1 — Write the failing test** (append to units.rs tests mod):

```rust
#[test]
fn abs_mirrors_f32() {
    assert_eq!((-3.5_f32).pt().abs(), 3.5_f32.pt());
    assert_eq!(2.0_f32.pt().abs(), 2.0_f32.pt());
    assert_eq!((-1.0_f32).px().abs(), 1.0_f32.px());
    // Byte-identical to the raw f32 op it replaces.
    let v = -7.25_f32;
    assert_eq!(v.pt().abs().to_f32(), v.abs());
}
```

**Step 2 — Run, expect FAIL** (`abs` not found):

Run: `cargo test -p fulgur --lib units::tests::abs_mirrors_f32`
Expected: compile error `no method named 'abs'`.

**Step 3 — Implement.** In `impl Pt` (mirror the existing `max`/`min` doc-comment style):

```rust
    /// Absolute value. Mirrors `f32::abs` so a migrated `(a - b).abs()`
    /// stays byte-identical.
    #[inline]
    pub fn abs(self) -> Pt {
        Pt(self.0.abs())
    }
```

In `impl Px` add the symmetric:

```rust
    /// Absolute value. Mirrors `f32::abs`.
    #[inline]
    pub fn abs(self) -> Px {
        Px(self.0.abs())
    }
```

**Step 4 — Run, expect PASS:** `cargo test -p fulgur --lib units::tests::abs_mirrors_f32`

**Step 5 — Commit:**

```bash
git add crates/fulgur/src/units.rs
git commit -m "feat(units): add Pt/Px abs() for P1b paragraph migration"
```

---

### Task 2: Migrate the 5 paragraph struct fields + all consumers (compiler-driven, atomic)

This is the load-bearing task. Field-type changes cascade through the type checker, so the struct defs and ALL consumers land in ONE compiling commit. Work the compiler error list top-to-bottom; the design field enumerates every site so nothing should surprise you. Do NOT insert any `* PX_TO_PT` / `* 0.75` to "fix" the EM×font_size width sites — that is a REFUTED hazard (see design); the product is already pt.

**Files (all in `crates/fulgur/src/`):**
- Modify `paragraph.rs`: struct defs (`ShapedGlyphRun.font_size`/`.x_offset`; `ShapedLine.height`/`.baseline`; `InlineImage` width/height/x_offset/computed_y; `InlineBoxItem` width/height/x_offset/computed_y) → `crate::units::Pt`; `ShapedGlyph` UNCHANGED. Then `draw_shaped_lines` (sig `x,y: crate::units::Pt`, `line_top` accumulator → `Pt::ZERO`, 3 LineItem arms), `draw_line_decorations` (`DecorationSpan{x,width}` → Pt, `font_size` → Pt, `run_width` Pt, `gap=(run_x-last_end).abs()`, `gap < 0.5_f32.pt()`, `.max(Pt::ZERO)`, `get_decoration_metrics(.., span.font_size.to_f32())`, `line_y = baseline_y + metrics.X.pt()`, `draw_decoration_line(span.x.to_f32(), line_y.to_f32(), span.width.to_f32(), ..)`), `recalculate_line_box` (`LineFontMetrics` stays f32 → `.pt()` tags; `VerticalAlign::Length(v)`/`Percent(p)` v/p f32 → tag to match current pt arithmetic; Pt comparisons), test fixtures at ~:1363,:1404,:1597 (`font_size: 10.0` → `10.0_f32.pt()`; coord fields likewise), and the `draw_shaped_lines` test arms ~:2059-2130.
- Modify `convert/inline_root.rs`: construction at :518-540 (`ShapedGlyphRun{font_size: px_to_pt(..)` already pt — wrap producer so the field is Pt; `x_offset: px_to_pt(glyph_run.offset())`), :589-598 (`InlineBoxItem`), :603-608 (`ShapedLine`). `px_to_pt(x)` returns f32 today — produce `Pt` via `x.px().in_pt()` OR `px_to_pt(x).pt()` (pick one consistently; both byte-identical, prefer `.px().in_pt()` to match spike idiom). EM-fractions (`g.advance / font_size_parley`) UNCHANGED.
- Modify `convert/list_marker.rs`: `InlineImage` construction ~:136-147 (width/height/x_offset/computed_y → Pt; `x_offset: 0.0` → `Pt::ZERO`, `computed_y: 0.0` → `Pt::ZERO`).
- Modify `render.rs`: `paragraph_lines_for_page` split path ~:3238-3284 (`consumed`/`target_h` → Pt via `sum().px().in_pt()` pattern since Fragment is still px-f32 (P1c, open); `line_top`/`next_top` → Pt; `next_top > consumed + eps` → tag eps; `line.baseline -= consumed`, `img.computed_y -= consumed`); list-item marker `draw_shaped_lines` caller ~:3070 (pass `units::Pt` x/y).
- Modify any `pageable.rs` `draw_shaped_lines` caller (ListItemPageable::draw) to pass `units::Pt`.
- Link rects: `draw_primitives::Rect` is f32 (P1a did NOT migrate it) → `.to_f32()` on each Pt coord at rect construction (paragraph.rs link-rect arms for text/image/inline-box).
- Krilla FFI: `Point::from_xy(x + run.x_offset, baseline_y)` and `from_translate(...)` and `draw_glyphs(.., run.font_size, ..)` → `.to_f32()` on the Pt args; `Size::from_wh(img.width, img.height)` → `.to_f32()`.

**Step 1 — Change the 5 struct field types** in paragraph.rs. Expect a long compiler error list.

**Step 2 — Drive the compiler.** Fix each error using the design's method-gap inventory. Reach for, in order of preference: same-unit op (Pt±Pt) → `Pt::ZERO`/`Pt::max`/`Pt::abs` → `.pt()` tag on an f32 that is semantically pt → `.to_f32()` ONLY at a genuine FFI/f32-struct boundary (Krilla, skrifa, `Rect`, `draw_decoration_line`). If you find yourself changing a numeric literal or op order, STOP — that breaks byte-neutrality.

**Step 3 — Build clean:**

Run: `cargo build -p fulgur`
Expected: `Finished` (0 errors).

**Step 4 — Lint clean:**

Run: `cargo clippy -p fulgur -- -D warnings` then `cargo fmt --check`
Expected: no warnings; fmt clean. (Watch for clippy wanting `.to_f32()` simplifications — keep explicit if it preserves clarity.)

**Step 5 — Unit tests green (incl. UNCHANGED metrics tests):**

Run: `cargo test -p fulgur --lib`
Expected: 1495+ passed, 0 failed. The `get_decoration_metrics_*` / `*_grow_with_font_size` tests must pass UNCHANGED — that is the proof Strategy C kept the metrics subsystem out of scope. (If you had to edit them, you drifted into Strategy A — reconsider.)

**Step 6 — Byte-neutral determinism gate:**

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo test -p fulgur-cli --test examples_determinism
cargo test -p fulgur-vrt
```

Expected: examples_determinism 11 passed; VRT `run_fulgur_vrt` ok against UNCHANGED goldens. NEVER set `FULGUR_VRT_UPDATE=1`. If VRT goes red, a byte changed — bisect the boundary (most likely an EM×font_size width site, a `.px().in_pt()` vs `.pt()` mismatch, or a reassociated sum) and fix the typing, NOT the golden.

**Step 7 — Commit:**

```bash
git add -A
git commit -m "refactor(paragraph): migrate ShapedLine/ShapedGlyphRun/InlineImage/InlineBoxItem coords to units::Pt (byte-neutral, P1b)"
```

---

### Task 3: Final holistic byte-neutral verification

**Step 1 — Full workspace build + lint:**

```bash
cargo build
cargo clippy -p fulgur -- -D warnings
cargo fmt --check
```

**Step 2 — Full determinism re-run from a clean state** (guards against incremental-compilation masking):

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo test -p fulgur --lib
cargo test -p fulgur-cli --test examples_determinism
cargo test -p fulgur-vrt
```

Expected: all green, goldens byte-identical.

**Step 3 — Confirm scope hygiene:**

```bash
git diff origin/main --stat
```

Expected touched files: `units.rs`, `paragraph.rs`, `convert/inline_root.rs`, `convert/list_marker.rs`, `render.rs`, possibly `pageable.rs`, plus this plan. NOT touched: `draw_primitives.rs` `Rect`/`Pt` alias, Fragment, Drawables aggregate, DecorationMetrics/LineFontMetrics field types, any `goldens/`.

**Step 4 — Done.** Hand back for code review (requesting-code-review) → finishing-a-development-branch.

---

## Notes / pitfalls

- `Pt` derives `PartialOrd`, so `Pt < Pt` works; `Pt < f32` does NOT — use `0.5_f32.pt()` / `Pt::ZERO` or compare via `.to_f32()`.
- `px_to_pt(x)` (convert.rs helper) returns `f32`. To produce a `Pt` field, prefer `x.px().in_pt()` (spike idiom) over `px_to_pt(x).pt()`; both are byte-identical (`x * 0.75`), pick one and stay consistent.
- The decoration internal helpers (`draw_decoration_line`/`draw_straight_line`/wavy, ~:303-435) take raw f32 and STAY f32 — they are below the to_f32 boundary. Do not migrate their internal `half < 0.01` / `(cx+half).min(...)`.
- Coverage: this is pure type migration (no new logic), so existing tests + the new `Pt::abs` unit test suffice; no new draw-path smoke test required.
