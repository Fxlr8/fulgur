# draw_primitives Size + BlockStyle → units::Pt (P1a) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or
> superpowers:subagent-driven-development) to implement this plan task-by-task.

**Goal:** Migrate the two **single-unit-pt** `draw_primitives` structs — `Size`
and `BlockStyle`'s coordinate fields — from bare `f32` to the `units::Pt`
newtype, byte-neutral, as phase P1a of the `fulgur-2map` epic (beads issue
`fulgur-2map.3`).

**Architecture:** `draw_primitives` currently aliases `pub type Pt = f32` and
uses it (or bare `f32`) for all coordinates. This phase types two *single-unit*
structs to the real `units::Pt`, leaving the `type Pt = f32` alias intact as a
transitional bridge (alias removal is P2 / `fulgur-2map.8`). The dual-stage
`Affine2D` + `Point2` (transform matrix/origin, unresolved px→pt fold) are
**out of scope** — spun out to `fulgur-1ino`. Producers convert px→pt with
`Px::in_pt`; the many draw-layer consumers untag to `f32` with `to_f32()` at
the point of use so the existing f32 drawing arithmetic stays byte-identical.

**Tech Stack:** Rust, `units::{Px, Pt, F32Units}` (`crates/fulgur/src/units.rs`;
`Pt::ZERO`/`max`/`min` already exist from the spike), krilla draw layer, VRT PDF
byte comparison (`crates/fulgur-vrt`), `examples_determinism` (`crates/fulgur-cli`).

## Scope

**In scope:**

- `draw_primitives::Size { width, height }` → `units::Pt` + its producers
  (convert: `inline_root`, `replaced`, `list_item` `BlockEntry.layout_size`) and
  consumers (`render` `layout_size`).
- `draw_primitives::BlockStyle` coordinate fields:
  `border_widths: [f32;4]` → `[units::Pt;4]`,
  `padding: [f32;4]` → `[units::Pt;4]`,
  `border_radii: [[f32;2];4]` → `[[units::Pt;2];4]`,
  plus producers (`convert::style::box_metrics`, `convert::style::border`) and
  ~46 reader sites (`background.rs`, `draw_primitives.rs` border/shadow draw
  helpers, `multicol_layout.rs`, `convert::positioned`, `replaced`, `mod`,
  `style::border`, `inline_root`).

**Out of scope (do NOT touch):**

- `Affine2D` + `Point2` → `fulgur-1ino` (dual-stage transform fold).
- `type Pt = f32` alias flip → P2 / `fulgur-2map.8`.
- `Rect` (link-area `f32`), `BoxShadow` / `BackgroundLayer` internal coords,
  `size_in_pt()` helper return type — not listed for P1a.

## The byte-neutral rule (read before every edit)

`px_to_pt(v)` is exactly `v * 0.75` (`PX_TO_PT`), identical to `v.px().in_pt()`
(one multiply). NEVER distribute the multiply across an addition (no
reassociation); preserve exact left-to-right operand order. **Untag-at-use:**
where a now-`Pt` field feeds existing `f32` arithmetic or an `f32`-taking draw
helper, append `.to_f32()` at the read site so the surrounding f32 expression is
**unchanged** — this is the lowest-risk byte-neutral move and the default for
the BlockStyle readers. Proof = unchanged goldens; never `FULGUR_VRT_UPDATE=1`.

## Verification commands (end of every task)

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
```

- `cargo build` → `Finished`
- `cargo clippy -p fulgur -- -D warnings` → clean
- `cargo fmt --check` → clean
- `cargo test -p fulgur --lib` → all pass
- `cargo test -p fulgur --test render_smoke` → all pass
- `cargo test -p fulgur-cli --test examples_determinism` → `11 passed; 0 failed`
- `cargo test -p fulgur-vrt` → `run_fulgur_vrt ... ok` (the **load-bearing** byte
  proof; NO golden regeneration)

**Baseline captured GREEN** in this worktree before editing (examples_determinism
11/0, VRT ok). Base: `origin/main` `ca4d9b41`.

---

### Task 1: Migrate `draw_primitives::Size` to `units::Pt`

Small, re-proves the per-struct bridge before the bulky BlockStyle.

**Files:**

- Modify: `crates/fulgur/src/draw_primitives.rs` (`Size` def ~line 82)
- Modify: producers — `crates/fulgur/src/convert/inline_root.rs`,
  `crates/fulgur/src/convert/replaced.rs`,
  `crates/fulgur/src/convert/list_item.rs` (each `Size { width, height }`)
- Modify: consumers — `crates/fulgur/src/render.rs` (`layout_size` reads:
  total_width / height ~lines 1636, 1651, 2053, …)

**Step 1: Type the fields**

In `draw_primitives.rs`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: crate::units::Pt,
    pub height: crate::units::Pt,
}
```

**Step 2: Build to surface every break**

Run: `cargo build` → Expected: FAIL, listing each producer/consumer site.
Use the errors as the worklist.

**Step 3: Fix producers (tag pt values with `.pt()`)**

Each `Size { width, height }` is built from `size_in_pt(...)` f32-pt locals
(`inline_root.rs:27 let (width, height) = size_in_pt(node.final_layout.size);`).
Tag at construction — byte-neutral (pure label):

```rust
use crate::units::F32Units; // add to each producer file if absent
// ...
layout_size: Some(Size { width: width.pt(), height: height.pt() }),
```

Do NOT change `size_in_pt`'s return type (out of scope).

**Step 4: Fix consumers (untag at use)**

In `render.rs`, where `layout_size.width` / `.height` feed existing f32
arithmetic, append `.to_f32()` so the math is unchanged, e.g.
`self.layout_size.map(|s| s.width)` → `... s.width.to_f32()`. Preserve operand
order. If a site compares/combines purely with other `Pt`, keep it `Pt`.

**Step 5: Build + lint**

`cargo build` && `cargo clippy -p fulgur -- -D warnings` && `cargo fmt --check`
→ all clean.

**Step 6: Byte-identity proof**

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo test -p fulgur --lib
cargo test -p fulgur --test render_smoke
cargo test -p fulgur-cli --test examples_determinism
cargo test -p fulgur-vrt
```

Expected: all pass; VRT goldens byte-identical. If VRT reds, a reassociation
crept in — fix operand order, never regenerate.

**Step 7: Commit**

```bash
git add crates/fulgur/src/draw_primitives.rs crates/fulgur/src/convert/ crates/fulgur/src/render.rs
git commit -m "refactor(draw_primitives): type Size with units::Pt (byte-neutral, P1a)"
```

---

### Task 2: Migrate `BlockStyle` coordinate fields to `units::Pt`

Bulky but mechanical (~46 reader sites). Apply **untag-at-use** by default.

**Files:**

- Modify: `crates/fulgur/src/draw_primitives.rs` (`BlockStyle` def ~605; border/
  shadow draw helpers ~10 reader sites; `#[cfg(test)]` `BlockStyle { ... }`
  constructions ~1681+, 2012+)
- Modify producers: `crates/fulgur/src/convert/style/box_metrics.rs`
  (border_widths, padding), `crates/fulgur/src/convert/style/border.rs`
  (border_radii)
- Modify readers: `crates/fulgur/src/background.rs` (11),
  `crates/fulgur/src/multicol_layout.rs` (5),
  `crates/fulgur/src/convert/positioned.rs` (4),
  `crates/fulgur/src/convert/replaced.rs` (2),
  `crates/fulgur/src/convert/mod.rs` (2),
  `crates/fulgur/src/convert/inline_root.rs` (1)

**Step 1: Type the fields**

```rust
    /// Border widths: top, right, bottom, left
    pub border_widths: [crate::units::Pt; 4],
    /// Padding: top, right, bottom, left
    pub padding: [crate::units::Pt; 4],
    /// Border radii: [top-left, top-right, bottom-right, bottom-left] × [rx, ry]
    pub border_radii: [[crate::units::Pt; 2]; 4],
```

**Step 2: Fix producers (px_to_pt → `.px().in_pt()`)**

`box_metrics.rs` (`use crate::units::F32Units;`):

```rust
    style.border_widths = [
        layout.border.top.px().in_pt(),
        layout.border.right.px().in_pt(),
        layout.border.bottom.px().in_pt(),
        layout.border.left.px().in_pt(),
    ];
    style.padding = [
        layout.padding.top.px().in_pt(),
        layout.padding.right.px().in_pt(),
        layout.padding.bottom.px().in_pt(),
        layout.padding.left.px().in_pt(),
    ];
```

`border.rs` `border_radii`: same transform — each `px_to_pt(r)` → `r.px().in_pt()`
(the rx/ry pairs), preserving the per-element conversion (not a folded one).

**Step 3: Build to get the reader worklist**

`cargo build` → FAIL. Each error is a reader site. Default fix: append
`.to_f32()` at the read so the existing f32 expression is byte-identical, e.g.

```rust
// before:  let bw = style.border_widths[0];           // was f32
// after:   let bw = style.border_widths[0].to_f32();   // Pt -> f32 at use
```

For `border_radii` reads: `style.border_radii[i][j].to_f32()`. Where a reader
already lives in clean `Pt` space (rare here), keep `Pt`. **Preserve operand
order everywhere.** Update the `#[cfg(test)]` `BlockStyle { ... }` literals and
assertions in `draw_primitives.rs` and `box_metrics.rs` to construct/compare
`Pt` (e.g. `assert_eq!(style.border_widths, [3.0, 6.0, 9.0, 12.0].map(|v| v.pt())` ... )
or compare `.map(Pt::to_f32)` — keep the asserted numbers identical).

**Step 4: Build + lint**

`cargo build` && `cargo clippy -p fulgur -- -D warnings` && `cargo fmt --check`.

**Step 5: Byte-identity proof (full suite)**

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo test -p fulgur --lib
cargo test -p fulgur --test render_smoke
cargo test -p fulgur-cli --test examples_determinism
cargo test -p fulgur-vrt
```

Expected: all pass; VRT goldens byte-identical (border/padding/radius rendering
exercised by many goldens). A red golden = a converted reader changed operand
order or a producer reassociated — fix the code.

**Step 6: Commit**

```bash
git add crates/fulgur/src/draw_primitives.rs crates/fulgur/src/convert/ crates/fulgur/src/background.rs crates/fulgur/src/multicol_layout.rs
git commit -m "refactor(draw_primitives): type BlockStyle border/padding/radii with units::Pt (byte-neutral, P1a)"
```

---

### Task 3: Docs + final verification

**Files:**

- Modify: `docs/plans/2026-06-27-engine-layout-api-design.md` (append a one-line
  P1a-done note under the §7 "Spike outcome" area, noting Size + BlockStyle
  migrated and Affine2D/Point2 spun out to `fulgur-1ino`)

**Step 1: Append the note**, then `npx markdownlint-cli2` the file → 0 errors.

**Step 2: Final full verification** (all commands from "Verification commands"
above) → all green, goldens byte-identical.

**Step 3: Commit**

```bash
git add docs/plans/2026-06-27-engine-layout-api-design.md
git commit -m "docs(2map): record P1a (Size + BlockStyle) migration; Affine2D/Point2 -> fulgur-1ino"
```

---

## Done criteria (maps to `fulgur-2map.3`)

- [ ] `Size.width`/`height` and `BlockStyle.{border_widths,padding,border_radii}`
      are `units::Pt`-typed.
- [ ] Producers convert via `Px::in_pt` (one multiply); readers untag with
      `to_f32()` at use; `type Pt = f32` alias untouched.
- [ ] `examples_determinism` + VRT goldens **byte-identical** (no
      `FULGUR_VRT_UPDATE`).
- [ ] `cargo build` / `clippy -p fulgur -D warnings` / `fmt --check` /
      `cargo test -p fulgur --lib` / `--test render_smoke` all clean.
- [ ] Affine2D/Point2 deferral recorded (`fulgur-1ino`).
