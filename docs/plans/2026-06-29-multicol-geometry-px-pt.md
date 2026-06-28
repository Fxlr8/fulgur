# multicol/column geometry → units::Px/Pt (P1d) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (or executing-plans) to implement this plan task-by-task.

**Goal:** Complete the multicol coordinate-newtype migration the spike
(`fulgur-2map.2`) deferred — type `multicol_layout`'s px-source geometry
(`ColumnGroupGeometry`, `ColumnLineSlice`) to `units::Px` and
`column_css::ColumnRuleSpec.width` to `units::Pt`, byte-neutral (beads
`fulgur-2map.6`, phase P1d).

**Architecture:** The spike already split the pt carrier
(`drawables::ColumnRuleGeometry`), so these source structs are now **pure
single-unit**. Same pattern as P1a: tag at the source (`.px()` / `.pt()`),
untag at use (`.to_f32()`) so existing f32 arithmetic and FFI calls (Parley
`break_all_lines`, krilla `colored_stroke`) stay byte-identical. Two cleanups
fall out: `convert::record_multicol_rule` drops the spike's now-redundant
`.px()` tag (`g.x_offset.px().in_pt()` → `g.x_offset.in_pt()`), and the px→pt
conversions in `convert_multicol_paragraph_slices` become `.in_pt()`.

**Tech Stack:** Rust, `units::{Px, Pt, F32Units}` (`Pt::ZERO`/`max`/`min` exist
from the spike; `Px::in_pt`/`to_f32` exist), `taffy::{Point, Size}<T>` generics,
krilla, Parley, VRT byte comparison.

## Scope

**In scope (three single-unit migrations):**

- `multicol_layout::ColumnGroupGeometry`: `x_offset`/`y_offset`/`col_w`/`gap` →
  `units::Px`; `col_heights: Vec<f32>` → `Vec<units::Px>`. (`n: u32`,
  `paragraph_splits` unchanged.)
- `multicol_layout::ColumnLineSlice`: `origin: taffy::Point<f32>` →
  `taffy::Point<units::Px>`, `size: taffy::Size<f32>` → `taffy::Size<units::Px>`.
- `column_css::ColumnRuleSpec.width: f32` (pt) → `units::Pt`.

**Out of scope:** the `type Pt = f32` alias (P2 / `fulgur-2map.8`);
`ParagraphSplitEntry.line_range` (not a coordinate); the pt carrier
`ColumnRuleGeometry` (done in the spike). No new `units` helpers needed
(no Px arithmetic beyond direct `Px ± Px`; `Pt::ZERO` already exists).
`pageable.rs` does not exist — `render.rs` is the v2 path.

## The byte-neutral rule (read before every edit)

`px_to_pt(v)` ≡ `v * 0.75` ≡ `v.px().in_pt()` ≡ `Px::in_pt` (one f32 multiply).
`.px()`/`.pt()`/`.to_f32()` are pure labels on `#[repr(transparent)]` newtypes.
NEVER reorder/reassociate arithmetic; preserve operand order. **Untag-at-use**
is the default for readers that feed f32 math or an f32 FFI/helper. Proof =
unchanged VRT goldens; never `FULGUR_VRT_UPDATE=1`.

## Verification commands (end of every task)

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo build                                          # Finished
cargo clippy -p fulgur --all-targets -- -D warnings  # clean
cargo fmt --check                                    # clean
cargo test -p fulgur --lib                           # all pass
cargo test -p fulgur --test render_smoke             # all pass
cargo test -p fulgur-cli --test examples_determinism # 11 passed
cargo test -p fulgur-vrt                             # run_fulgur_vrt ... ok
git status --short -- crates/fulgur-vrt/goldens/     # MUST be empty
```

**Baseline captured GREEN** before editing (examples_determinism 11/0, VRT ok).
Base: `origin/main` `2ef34d18`.

---

### Task 1: `ColumnGroupGeometry` coordinate fields → `units::Px`

**Files:** `crates/fulgur/src/multicol_layout.rs` (struct ~84; 2 construction
sites ~856 & ~965; internal `col_heights` fold reader ~675; ~20 unit-test
asserts), `crates/fulgur/src/convert/mod.rs` (`record_multicol_rule` ~676,
`convert_multicol_paragraph_slices` ~743/788).

**Step 1 — type the fields** (use fully-qualified `crate::units::Px` to avoid
the `type Pt = f32` alias-shadowed module; mirror P1a):

```rust
    pub x_offset: crate::units::Px,
    pub y_offset: crate::units::Px,
    pub col_w: crate::units::Px,
    pub gap: crate::units::Px,
    // ...
    pub col_heights: Vec<crate::units::Px>,
```

**Step 2 — build to get the worklist** (`cargo build` lists every break).

**Step 3 — tag at construction** (the layout-math locals are Taffy-space f32;
`.px()` is a pure label). At both `ColumnGroupGeometry { ... }` sites:

```rust
        x_offset: inset_left.px(),
        y_offset: inset_top.px(),
        col_w: col_w.px(),
        gap: gap.px(),
        n,
        col_heights: col_heights.iter().copied().map(F32Units::px).collect(),
        paragraph_splits: ...,
```

(`use crate::units::F32Units;` — confirm it's in scope in multicol_layout.rs;
add if absent. The local `col_heights: Vec<f32>` keeps being built in f32; the
`fold(0.0, f32::max)` that computes `max_col_h` from the LOCAL before
construction stays f32 — only the struct field becomes `Vec<Px>`.)

**Step 4 — untag readers:**

- `convert::record_multicol_rule`: `g.x_offset.px().in_pt()` →
  `g.x_offset.in_pt()` (drop the spike tag; byte-identical). `col_heights`
  `.map(|h| h.px().in_pt())` → `.map(|h| h.in_pt())`.
- `convert::convert_multicol_paragraph_slices`: `px_to_pt(group.x_offset)` →
  `group.x_offset.in_pt()` (and `group.y_offset`); `break_all_lines(Some(group.col_w))`
  → `break_all_lines(Some(group.col_w.to_f32()))` (Parley FFI is f32);
  `px_to_pt(group.col_w)` (if any) → `group.col_w.in_pt()`.
- multicol_layout internal: the `geometry.col_heights....fold(0.0_f32, f32::max)`
  reader → `geometry.col_heights.iter().copied().map(crate::units::Px::to_f32).fold(0.0_f32, f32::max)`.
- unit-test asserts reading `.col_heights[i]`/`.x_offset`/etc → append `.to_f32()`
  (keep asserted numbers identical).

**Step 5 — verify** (full block above) → goldens byte-identical.

**Step 6 — commit:** `git add -A && git commit -m "refactor(multicol): type ColumnGroupGeometry coords with units::Px (byte-neutral, P1d)"`

---

### Task 2: `ColumnLineSlice` `origin`/`size` → `taffy::{Point,Size}<units::Px>`

`taffy::Point<T>`/`Size<T>` are generic; `Px` satisfies their bounds (derives
`Default`/`Clone`/`Copy`; has `Add`/`Sub`/`Mul`/`Div`). If `cargo build`
reveals a taffy bound `Px` cannot satisfy, STOP and spin out a tracked issue
(like `fulgur-1ino`) noting the taffy-generics reason — do not hack around it.

**Files:** `crates/fulgur/src/multicol_layout.rs` (struct ~38; 3 construction
sites ~931/1225 + `ColumnLineSlice::default()` ~926; distribution arithmetic
~1278-1279, ~1303; tests), `crates/fulgur/src/convert/mod.rs`
(`convert_multicol_paragraph_slices` ~861-864).

**Step 1 — type the fields:**

```rust
    pub origin: taffy::Point<crate::units::Px>,
    pub size: taffy::Size<crate::units::Px>,
```

**Step 2 — build worklist.**

**Step 3 — tag at construction** (`.px()` the f32 taffy-space values):

```rust
        origin: taffy::Point { x: x.px(), y: y.px() },
        size: taffy::Size { width: w.px(), height: h.px() },
```

(`ColumnLineSlice::default()` keeps deriving `Default` —
`taffy::Point<Px>::default()` works since `Px: Default`.)

**Step 4 — untag readers:**

- multicol_layout distribution: `slice.origin.y + slice.size.height` stays
  `Px + Px` (works, byte-identical) — but the consumer of that sum
  (`bottom`) and `resolve_col_idx(slice.origin.x)` (closure `|x: f32|`) need
  `.to_f32()` where they feed f32 code. Preserve operand order; prefer
  `.to_f32()` at the f32 boundary. (`let bottom = (slice.origin.y + slice.size.height).to_f32();`
  if `bottom` is compared as f32, OR keep `bottom: Px` and untag at its use.)
- `convert::convert_multicol_paragraph_slices`: `px_to_pt(col_slice.origin.x)`
  → `col_slice.origin.x.in_pt().to_f32()` (NOT bare `.in_pt()` — after Task 1
  the sibling `group_x_pt` is `f32` and these are summed as `group_x_pt + <f32>`;
  there is no `Add<Pt> for f32`, and the result feeds the out-of-scope
  `(f32,f32)` `ParagraphSlice` tuples). Same for `col_slice.origin.y` and
  `col_slice.size.height`. `.in_pt().to_f32()` ≡ `px_to_pt` (×0.75 as f32).
- tests reading `.origin.x`/`.size.height` → `.to_f32()`.

**Step 5 — verify** → goldens byte-identical (multicol inline-root-split
goldens exercise paragraph slices).

**Step 6 — commit:** `git add -A && git commit -m "refactor(multicol): type ColumnLineSlice origin/size with taffy Point/Size<Px> (byte-neutral, P1d)"`

---

### Task 3: `ColumnRuleSpec.width` → `units::Pt`

**Files:** `crates/fulgur/src/column_css.rs` (struct ~71, `Default` ~81, parser
assigns ~570 & ~709, ~14 test asserts), `crates/fulgur/src/convert/mod.rs`
(`record_multicol_rule` filter ~663), `crates/fulgur/src/render.rs`
(`build_multicol_stroke` ~2383-2388, 1 test), `crates/fulgur/src/multicol_layout.rs`
(test ~3297), `crates/fulgur/src/blitz_adapter.rs` (test ~5129).

**Step 1 — type the field + Default:**

```rust
    pub width: crate::units::Pt,
// in impl Default:
            width: 1.0_f32.pt(),   // needs `use crate::units::F32Units;`
```

**Step 2 — build worklist.**

**Step 3 — producers / readers:**

- parser assigns (`spec.width = w;` ×2, `w` is f32 pt from `length_to_pt`) →
  `spec.width = w.pt();`. (Leave `length_to_pt`'s `-> Option<f32>` signature.)
- `record_multicol_rule` filter: `r.width > 0.0` → `r.width > crate::units::Pt::ZERO`.
- `render::build_multicol_stroke`: `rule.width <= 0.0` →
  `rule.width <= crate::units::Pt::ZERO`; `colored_stroke(&rule.color, rule.width, opacity)`
  → `colored_stroke(&rule.color, rule.width.to_f32(), opacity)` (helper takes f32);
  `let w = rule.width;` → `let w = rule.width.to_f32();` (dash `w * 3.0` etc.
  stay f32, unchanged).
- test asserts `(rule.width - N).abs()` → `(rule.width.to_f32() - N).abs()`
  (column_css ~14, render ~1, multicol_layout ~1, blitz_adapter ~1); keep
  numbers identical.

**Step 4 — verify** → multicol-rule-solid / multicol-2 / multicol-span-all
goldens byte-identical.

**Step 5 — commit:** `git add -A && git commit -m "refactor(column_css): type ColumnRuleSpec.width with units::Pt (byte-neutral, P1d)"`

---

### Task 4: Docs + final verification

**Files:** `docs/plans/2026-06-27-engine-layout-api-design.md` (append a P1d
outcome note under §7, noting ColumnGroupGeometry + ColumnLineSlice → Px and
ColumnRuleSpec.width → Pt), and commit this plan
(`docs/plans/2026-06-29-multicol-geometry-px-pt.md`).

**Step 1** append note; `npx markdownlint-cli2` both files → 0 errors.
**Step 2** run the full verification block → all green, goldens byte-identical.
**Step 3** commit:
`git add docs/plans/*.md && git commit -m "docs(2map): record P1d (multicol/column geometry) outcome"`

> Coverage note: the migration updates many existing unit-test asserts and the
> render_smoke multicol cases (from the spike + P1a) exercise the changed
> producer/reader lines through `Engine::render_html`. If codecov/patch later
> flags an uncovered changed line, add a focused render_smoke case (per
> CLAUDE.md "Coverage scope"); confirm with `cargo llvm-cov nextest --workspace
> --exclude fulgur-vrt`.

---

## Done criteria (maps to `fulgur-2map.6`)

- [ ] `ColumnGroupGeometry` + `ColumnLineSlice` coords are `units::Px`;
      `ColumnRuleSpec.width` is `units::Pt`.
- [ ] `record_multicol_rule` drops the spike `.px()` tag (`g.field.in_pt()`).
- [ ] Producers tag at source; readers untag at use; `to_f32()` only at FFI
      (Parley/krilla) and f32-typed internal helpers.
- [ ] `examples_determinism` + VRT goldens byte-identical (no `FULGUR_VRT_UPDATE`).
- [ ] build / clippy `-D warnings` / fmt / `--lib` / `--test render_smoke` clean.
- [ ] `type Pt = f32` alias untouched (P2). ColumnLineSlice included (or spun
      out with a tracked issue if taffy bounds reject `Px`).
