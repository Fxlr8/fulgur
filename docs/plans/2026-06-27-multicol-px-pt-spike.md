# Multicol column-rule Px/Pt Validation Spike — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or
> superpowers:subagent-driven-development) to implement this plan task-by-task.

**Goal:** Migrate the multicol column-rule **pt carrier** (the geometry that
`Drawables::multicol_rules` hands to the draw layer) from bare `f32` to the
`units::Pt` newtype, end-to-end from the px→pt conversion site to the krilla
FFI boundary, proving the per-phase byte-neutral pattern for the
`fulgur-2map` epic (beads issue `fulgur-2map.2`).

**Architecture:** Today `convert::record_multicol_rule` converts a px-native
`multicol_layout::ColumnGroupGeometry` into a *second* `ColumnGroupGeometry`
holding pt values (`groups_pt`) — one struct naming two unit spaces, the
"naming-contract hole" the epic closes. This spike introduces a dedicated
`Pt`-typed carrier (`drawables::ColumnRuleGeometry`) for the pt side, so the
producer (`record_multicol_rule`) converts with `Px::in_pt`, the carrier
fields are `Pt`, and the single consumer (`render::paint_multicol_rule_for_page`)
does `Pt` arithmetic and drops to `f32` with `to_f32()` only at the
`stroke_line` FFI call. **Byte-neutrality is the acceptance test:** the
existing `examples_determinism` and VRT goldens (incl. `multicol-rule-solid`)
must stay byte-identical, proven by preserving f32 operation order (no
reassociation).

**Tech Stack:** Rust, `units::{Px, Pt, F32Units}` newtypes
(`crates/fulgur/src/units.rs`), krilla draw layer, VRT PDF byte comparison
(`crates/fulgur-vrt`), `examples_determinism` (`crates/fulgur-cli`).

## Scope

**In scope (the pt carrier path):**

- `units::Pt` gains `max` / `min` helpers (unit-preserving, mirror `f32::max`).
- New `drawables::ColumnRuleGeometry` with `Pt` coordinate fields; becomes the
  element type of `MulticolRuleEntry.groups`.
- `convert::record_multicol_rule` builds the `Pt` carrier via `.px().in_pt()`.
- `render::paint_multicol_rule_for_page` consumes the `Pt` carrier, does `Pt`
  arithmetic, `to_f32()` only at `stroke_line`.

**Out of scope (left for `fulgur-2map.6` / P1d proper):**

- Typing the *px source* struct `multicol_layout::ColumnGroupGeometry` to `Px`
  (ripples into 2 construction sites, `col_heights: Vec<f32>`, the
  `fold(0.0, f32::max)` readers, `convert_multicol_paragraph_slices`, and
  ~20 multicol_layout unit-test assertions — its own bounded task, not needed
  to prove the conversion pattern here).
- Typing `column_css::ColumnRuleSpec.width` to `Pt` (pt-only relabel, no
  px→pt conversion to validate; pulls in the CSS length parser + ~15 parser
  test assertions).
- `Engine::layout()` public exposure and the `draw_primitives::Pt = f32` alias
  replacement (later epic phases).

## The byte-neutral rule (read before every edit)

`px_to_pt(v)` is exactly `v * 0.75` (`PX_TO_PT`). Its newtype equivalent is
`v.px().in_pt()` which is `Pt(v * 0.75)` — **one** multiply, identical f32
rounding. NEVER distribute the multiply across an addition: `px_to_pt(a + b)`
must become `(a + b).px().in_pt()`, **not** `a.px().in_pt() + b.px().in_pt()`.
Preserve the exact left-to-right operand order of every converted expression.
The proof is the unchanged goldens; if a golden flips, a reassociation crept
in — fix the code, **never** run `FULGUR_VRT_UPDATE=1`.

## Verification commands (used at the end of every task)

Always export the pinned fontconfig first (determinism — see CLAUDE.md):

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
```

- Build: `cargo build` → Expected: `Finished`
- Lint: `cargo clippy -p fulgur -- -D warnings` → Expected: no warnings
- Format: `cargo fmt --check` → Expected: clean
- Unit: `cargo test -p fulgur --lib` → Expected: all pass
- Byte-identity 1: `cargo test -p fulgur-cli --test examples_determinism`
  → Expected: `11 passed; 0 failed`
- Byte-identity 2: `cargo test -p fulgur-vrt` → Expected: `run_fulgur_vrt ... ok`
  (NO golden regeneration)

**Baseline already captured GREEN** in this worktree before any edit
(`examples_determinism` 11/0, VRT `run_fulgur_vrt` ok), so any later red is
attributable to the change, not font drift.

---

### Task 1: Add unit-preserving `max` / `min` to `units::Pt`

The consumer clamps lengths with `.max(0.0)` / `.min(cutoff)` and
`fold(0.0, f32::max)`. `Pt` derives `PartialOrd` (comparisons already work) but
has no `max`/`min`. Add them mirroring `f32::max`/`f32::min` exactly so bytes
do not move.

**Files:**

- Modify: `crates/fulgur/src/units.rs` (in the `impl Pt { ... }` block, near
  `in_px`)
- Test: `crates/fulgur/src/units.rs` (`#[cfg(test)] mod tests`)

**Step 1: Write the failing test**

Add to `mod tests` in `units.rs`:

```rust
#[test]
fn pt_max_min_mirror_f32() {
    assert_eq!(Pt(1.0).max(Pt(2.0)), Pt(2.0));
    assert_eq!(Pt(1.0).min(Pt(2.0)), Pt(1.0));
    // identical to f32::max/min, including the 0.0 clamp idiom
    assert_eq!(Pt(-3.0).max(Pt(0.0)), Pt(0.0));
    assert_eq!([Pt(0.0), Pt(2.0), Pt(1.0)].into_iter().fold(Pt(0.0), Pt::max), Pt(2.0));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fulgur --lib units::tests::pt_max_min_mirror_f32`
Expected: FAIL — `no method named max found for struct Pt`

**Step 3: Write minimal implementation**

In `impl Pt { ... }` (add after `in_px`):

```rust
/// Larger of two lengths. Mirrors `f32::max` (same NaN handling) so a
/// migrated `x.max(y)` stays byte-identical.
#[inline]
pub fn max(self, other: Pt) -> Pt {
    Pt(self.0.max(other.0))
}

/// Smaller of two lengths. Mirrors `f32::min`.
#[inline]
pub fn min(self, other: Pt) -> Pt {
    Pt(self.0.min(other.0))
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fulgur --lib units::tests::pt_max_min_mirror_f32`
Expected: PASS

**Step 5: Build + lint + commit**

```bash
cargo clippy -p fulgur -- -D warnings
cargo fmt
git add crates/fulgur/src/units.rs
git commit -m "feat(units): add unit-preserving Pt::max/Pt::min (multicol spike prep)"
```

---

### Task 2: Introduce the `Pt`-typed `ColumnRuleGeometry` carrier and thread it producer→consumer

This is one atomic change: switching `MulticolRuleEntry.groups` to a new
`Pt`-typed struct breaks both the producer (`convert::record_multicol_rule`)
and the consumer (`render::paint_multicol_rule_for_page`) at once, so all three
edits land together to keep the crate compiling. Byte-neutrality is verified by
the goldens at the end.

**Files:**

- Modify: `crates/fulgur/src/drawables.rs` (add `ColumnRuleGeometry`, retype
  `MulticolRuleEntry.groups` ~line 207-210)
- Modify: `crates/fulgur/src/convert/mod.rs` (`record_multicol_rule`,
  ~line 668-693)
- Modify: `crates/fulgur/src/render.rs` (`paint_multicol_rule_for_page`,
  ~line 2290-2375)

**Step 1: Add the carrier type in `drawables.rs`**

Replace the `MulticolRuleEntry` struct (currently `groups:
Vec<crate::multicol_layout::ColumnGroupGeometry>`) with:

```rust
/// Per-column-group geometry for painting a multicol `column-rule`, in PDF
/// pt (already converted from `multicol_layout::ColumnGroupGeometry`'s CSS
/// px by `convert::record_multicol_rule`). A distinct pt-typed carrier so
/// the px source struct is no longer reused across two unit spaces.
#[derive(Debug, Clone)]
pub struct ColumnRuleGeometry {
    /// Horizontal offset from the container border-box left to column 0.
    pub x_offset: crate::units::Pt,
    /// Vertical offset from the container border-box top (incl. padding-top
    /// + border-top) to this group.
    pub y_offset: crate::units::Pt,
    /// Width of a single column.
    pub col_w: crate::units::Pt,
    /// Gap between adjacent columns.
    pub gap: crate::units::Pt,
    /// Number of columns this group balances across.
    pub n: u32,
    /// Per-column filled height; length == `n`.
    pub col_heights: Vec<crate::units::Pt>,
}

/// Multicol column-rule paint spec + per-column-group geometry.
/// Mirrors the fields `MulticolRulePageable` carries — render at the
/// container's location after children paint, partitioning `groups`
/// per page based on the container's fragment cumulative heights.
#[derive(Debug, Clone)]
pub struct MulticolRuleEntry {
    pub rule: crate::column_css::ColumnRuleSpec,
    pub groups: Vec<ColumnRuleGeometry>,
}
```

**Step 2: Update the producer `record_multicol_rule` in `convert/mod.rs`**

Replace the `groups_pt` builder (the `.map(|g| ColumnGroupGeometry { ... })`
block, ~line 671-684) with a builder for the new carrier. Each
`px_to_pt(g.field)` becomes `g.field.px().in_pt()` (one multiply, byte-neutral;
`g.field` is still bare-f32 px from the source struct, so tag it with `.px()`):

```rust
// `ColumnGroupGeometry` is recorded in CSS px; convert to the pt-typed
// ColumnRuleGeometry carrier so downstream paint matches every other
// Drawables entry's units. Each conversion is one multiply (byte-neutral).
let groups: Vec<crate::drawables::ColumnRuleGeometry> = geometry
    .groups
    .iter()
    .map(|g| crate::drawables::ColumnRuleGeometry {
        x_offset: g.x_offset.px().in_pt(),
        y_offset: g.y_offset.px().in_pt(),
        col_w: g.col_w.px().in_pt(),
        gap: g.gap.px().in_pt(),
        n: g.n,
        col_heights: g.col_heights.iter().copied().map(|h| h.px().in_pt()).collect(),
    })
    .collect();
out.multicol_rules.insert(
    node_id,
    crate::drawables::MulticolRuleEntry { rule, groups },
);
```

Add `use crate::units::F32Units;` at the top of `convert/mod.rs` if not already
imported (needed for `.px()`). The old `use crate::convert::px_to_pt;` /
`px_to_pt` call in this function is removed for the geometry conversion (it may
still be used elsewhere in the file — leave those).

**Step 3: Update the consumer `paint_multicol_rule_for_page` in `render.rs`**

The carrier fields are now `Pt`. Make the page-partition scalars `Pt` and keep
the **exact operand order** of every expression. `Pt(0.0)` can't be written
outside `units` (private field) — use `0.0_f32.pt()`. Drop to `f32` only at
`stroke_line`.

Replace the body from `let consumed` through the inner `stroke_line` call with:

```rust
    use crate::units::F32Units;

    let consumed = container_geom.fragments[..target_pos]
        .iter()
        .map(|f| f.height)
        .sum::<f32>()
        .px()
        .in_pt();
    let cutoff = target_frag.height.px().in_pt();

    let x_base = margin_left_pt.pt() + target_frag.x.px().in_pt();
    let y_base = margin_top_pt.pt() + target_frag.y.px().in_pt();
    let zero = 0.0_f32.pt();

    for group in &entry.groups {
        if group.n < 2 || group.col_heights.len() != group.n as usize {
            continue;
        }
        let group_top = group.y_offset - consumed;
        let max_h = group
            .col_heights
            .iter()
            .copied()
            .fold(zero, crate::units::Pt::max);
        let group_bottom = group_top + max_h;
        if group_bottom <= zero || group_top >= cutoff {
            continue;
        }
        let visible_top = group_top.max(zero);
        let y_top = y_base + visible_top;
        let consumed_above = (visible_top - group_top).max(zero);
        let visible_h = (group_bottom.min(cutoff) - visible_top).max(zero);
        for i in 0..(group.n as usize - 1) {
            let h_left = (group.col_heights[i] - consumed_above)
                .max(zero)
                .min(visible_h);
            let h_right = (group.col_heights[i + 1] - consumed_above)
                .max(zero)
                .min(visible_h);
            if h_left <= zero || h_right <= zero {
                continue;
            }
            let rule_x = x_base
                + group.x_offset
                + (i as f32 + 1.0) * group.col_w
                + i as f32 * group.gap
                + group.gap / 2.0;
            let y_bot = y_top + h_left.min(h_right);
            stroke_line(
                canvas,
                rule_x.to_f32(),
                y_top.to_f32(),
                rule_x.to_f32(),
                y_bot.to_f32(),
                stroke.clone(),
            );
        }
    }
    canvas.surface.set_stroke(None);
```

Notes on byte-neutrality of the rewrite (verify each as you type):

- `consumed`/`cutoff`/`x_base`/`y_base`: `px_to_pt(v)` → `v.px().in_pt()`;
  `margin_*_pt + px_to_pt(v)` → `margin_*_pt.pt() + v.px().in_pt()` — same
  order, one multiply.
- `(i as f32 + 1.0) * group.col_w`: `f32 * Pt` uses the commutative
  `Mul<Pt> for f32` impl = `(i+1.0) * col_w.0` — identical. Same for
  `i as f32 * group.gap` and `group.gap / 2.0` (`Pt / f32`).
- `.max(0.0)`/`.min(...)`/`fold(0.0, f32::max)` → `.max(zero)`/`.min(...)`/
  `fold(zero, Pt::max)` — `Pt::max`/`min` mirror `f32` exactly (Task 1).
- comparisons (`<= 0.0`, `>= cutoff`) → vs `zero`/`cutoff`; `PartialOrd`
  compares the inner `f32`.
- The `use crate::convert::px_to_pt;` at the top of the function is now unused
  for geometry — remove it **only if** nothing else in the function still uses
  it (it is used for `target_frag` scalars above, which we just migrated, so it
  becomes unused — remove it and rely on `.px().in_pt()`). Confirm with clippy.

**Step 4: Build**

Run: `cargo build`
Expected: `Finished` (no type errors — producer and consumer both updated)

**Step 5: Lint + format**

```bash
cargo clippy -p fulgur -- -D warnings
cargo fmt --check
```

Expected: clean (no unused `px_to_pt` import warning).

**Step 6: Byte-identity proof (the real acceptance test)**

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo test -p fulgur --lib
cargo test -p fulgur-cli --test examples_determinism
cargo test -p fulgur-vrt
```

Expected: `fulgur --lib` all pass; `examples_determinism` 11 passed / 0 failed;
VRT `run_fulgur_vrt ... ok` (the `multicol-rule-solid` / `multicol-2` /
`multicol-span-all` goldens stay byte-identical). If VRT goes red, a
reassociation crept in — diff the offending expression, fix operand order, do
NOT regenerate the golden.

**Step 7: Commit**

```bash
git add crates/fulgur/src/drawables.rs crates/fulgur/src/convert/mod.rs crates/fulgur/src/render.rs
git commit -m "refactor(multicol): type column-rule pt carrier with units::Pt (byte-neutral spike)"
```

---

### Task 3: Lib-side smoke test for the migrated render path (codecov patch coverage)

Per CLAUDE.md coverage policy, the VRT-only path is excluded from codecov; the
changed `convert`/`render` lines need a lib-side test reachable via
`Engine::render_html`. Add an end-to-end smoke test that renders a multicol
container with a `column-rule` so the migrated producer + consumer execute.

**Files:**

- Test: `crates/fulgur/tests/render_smoke.rs` (append a new `#[test]`)

**Step 1: Write the test**

```rust
/// fulgur-2map.2: the multicol column-rule pt carrier (units::Pt) renders
/// end-to-end through record_multicol_rule + paint_multicol_rule_for_page.
#[test]
fn multicol_column_rule_renders() {
    let html = r#"<!doctype html><html><body>
      <div style="column-count:3; column-gap:20px; column-rule:2px solid #333;">
        <p>alpha beta gamma delta epsilon zeta eta theta iota kappa
        lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega</p>
      </div>
    </body></html>"#;
    let pdf = fulgur::Engine::builder()
        .build()
        .render_html(html)
        .expect("render multicol column-rule");
    assert!(!pdf.is_empty());
}
```

**Step 2: Run it**

Run: `cargo test -p fulgur --test render_smoke multicol_column_rule_renders`
Expected: PASS (non-empty PDF; exercises the migrated path)

**Step 3: Commit**

```bash
git add crates/fulgur/tests/render_smoke.rs
git commit -m "test(multicol): lib smoke test for column-rule pt-carrier render path"
```

---

### Task 4: Document the established per-phase pattern + close out

The spike's deliverable is a *reusable pattern*. Record it so P1a–P1d follow it.

**Files:**

- Modify: `docs/plans/2026-06-27-engine-layout-api-design.md` (append a short
  "Spike outcome (fulgur-2map.2)" note under §7 or §8)

**Step 1: Append the pattern note**

Add a subsection capturing the validated recipe:

```markdown
### Spike outcome (fulgur-2map.2) — validated per-phase byte-neutral pattern

Migrated the multicol column-rule **pt carrier** end-to-end as the P0.5 spike:

1. **Type the converted (pt) side with a dedicated carrier.** Do not reuse the
   px source struct for pt values. `drawables::ColumnRuleGeometry` (Pt fields)
   replaced the second `ColumnGroupGeometry` that held pt — closing the
   one-struct-two-units hole on the pt side.
2. **Convert at the boundary, one multiply.** `px_to_pt(v)` → `v.px().in_pt()`.
   Never distribute across `+` (no reassociation). `margin + px_to_pt(v)` →
   `margin.pt() + v.px().in_pt()`.
3. **`to_f32()` only at FFI** (the `stroke_line` krilla call).
4. **Clamps/folds need unit-preserving helpers** — added `Pt::max`/`Pt::min`
   mirroring `f32` exactly; `Pt(0.0)` is unconstructable outside `units`, use
   `0.0_f32.pt()`.
5. **Proof = unchanged goldens.** `examples_determinism` + VRT stayed
   byte-identical; never `FULGUR_VRT_UPDATE=1`.

Deferred to P1d (`fulgur-2map.6`): typing the px source
`multicol_layout::ColumnGroupGeometry` to `Px`, and
`column_css::ColumnRuleSpec.width` to `Pt`.
```

**Step 2: Markdown lint**

Run: `npx markdownlint-cli2 'docs/plans/2026-06-27-*.md'`
Expected: no errors

**Step 3: Final full verification**

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo build && cargo clippy -p fulgur -- -D warnings && cargo fmt --check \
  && cargo test -p fulgur --lib \
  && cargo test -p fulgur-cli --test examples_determinism \
  && cargo test -p fulgur-vrt
```

Expected: all green, goldens byte-identical.

**Step 4: Commit**

```bash
git add docs/plans/2026-06-27-engine-layout-api-design.md
git commit -m "docs(2map): record multicol Px/Pt spike outcome + per-phase pattern"
```

---

## Done criteria (maps to `fulgur-2map.2` acceptance)

- [ ] `MulticolRuleEntry.groups` carries a `Pt`-typed `ColumnRuleGeometry`.
- [ ] Conversion uses `Px::in_pt` (one multiply); `to_f32()` only at the
      `stroke_line` FFI boundary.
- [ ] `examples_determinism` + VRT goldens **byte-identical** (no
      `FULGUR_VRT_UPDATE`).
- [ ] `cargo build`, `cargo clippy -p fulgur -- -D warnings`,
      `cargo fmt --check`, `cargo test -p fulgur --lib` all clean.
- [ ] Lib smoke test covers the migrated render path.
- [ ] Per-phase byte-neutral pattern documented for P1a–P1d.
