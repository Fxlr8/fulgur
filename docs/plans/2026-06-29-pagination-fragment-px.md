# pagination_layout Fragment → units::Px (P1c) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (or executing-plans) to implement this plan task-by-task.

**Goal:** Type `pagination_layout::Fragment`'s coordinate fields
(`x`/`y`/`width`/`height`) with `units::Px`, plus all fragmenter producers
and render/convert consumers, byte-neutral (beads `fulgur-2map.5`, phase P1c).

**Architecture:** Faithful copy of the P1d pattern
(`docs/plans/2026-06-29-multicol-geometry-px-pt.md`): tag at the source
(`.px()` on Taffy-space f32 locals at every `Fragment { ... }` construction),
untag at use (`.to_f32()` for f32 px-space readers and f32 FFI;
`.in_pt()` / `.in_pt().to_f32()` at the px→pt conversion boundary) so existing
f32 arithmetic stays byte-identical. **Lower per-site risk than P1d**: Fragment
fields are bare `f32` (not `taffy::Point`/`Size`), so there is no taffy-generics
bound problem. Higher *volume*: one struct, ~34 construction sites and ~40
reader sites across 7 files.

**Tech Stack:** Rust, `units::{Px, Pt, F32Units}` (`Px::in_pt`/`to_f32`,
`F32Units::px` exist; `Px::ZERO` added in Task 1), krilla/Parley FFI, VRT
byte comparison.

## Scope

**In scope:**

- `pagination_layout::Fragment`: `x`/`y`/`width`/`height: f32` → `units::Px`
  (`page_index: u32` unchanged).
- All `Fragment { ... }` construction sites (prod + tests): tag with `.px()`.
- All readers: render.rs (px→pt boundary), convert/positioned.rs (px-space),
  gcpm/target_ref.rs, pagination_layout.rs internal, engine.rs/render.rs test
  helpers, column_css.rs/blitz_adapter.rs test asserts.

**Out of scope (record for reviewers + later phases):**

- Internal pagination math stays `f32`: `cursor_y`, `page_height_px`,
  `child_h`, `gap`, and the gap clamp `(this_top - prev_bottom).max(0.0)`.
  These are Taffy-space f32 locals; tag with `.px()` ONLY at the `Fragment`
  construction boundary (mirrors P1d Task 1: local stays f32, struct field
  becomes the newtype). This keeps the hot-path arithmetic byte-identical and
  confines the type change to the struct boundary.
- `PaginationGeometry.is_repeat` / `paragraph_splits` / any non-coordinate
  field; `Drawables` aggregate fields (that is P1e / `fulgur-2map.7`, which
  this blocks); `type Pt = f32` alias (P2 / `fulgur-2map.8`).

## The byte-neutral rules (read before EVERY edit)

1. `px_to_pt(v)` ≡ `v * 0.75` ≡ `v.px().in_pt()` ≡ `Px::in_pt` (one f32
   multiply). `.px()` / `.to_f32()` / `.in_pt()` are pure labels /
   single-op conversions on `#[repr(transparent)]` newtypes.
2. **Reassociation rule (the one real risk at the px→pt boundary).** f32:
   `(a+b)*0.75 != a*0.75 + b*0.75`. Preserve the CURRENT order:
   - `px_to_pt(a + b)` form  → `(a + b).px().in_pt()` (add in px, convert once).
   - `px_to_pt(a) + px_to_pt(b)` form → keep two separate conversions.
   - A pre-audit grep found **zero** combined-field conversions in
     render/convert/gcpm readers today (every reader converts a single
     Fragment field), but `cargo build`'s worklist is the definitive source.
     Re-check any new multi-field reader against this rule before splitting.
3. Internal pagination_layout f32 sums (e.g. `frag.y + frag.height` in px
   space) stay f32 via `.to_f32()`, preserving operand order — these were f32
   already, so they are byte-identical.
4. **Untag-at-use is the default** for readers that feed f32 math, an f32
   FFI/helper, or an f32-typed sibling in a `+`. NEVER reorder/reassociate.
   Proof = unchanged VRT goldens; **never** `FULGUR_VRT_UPDATE=1`.

## Reader two-systems in render.rs (both exist today)

- **Unmigrated** (31 sites): `margin_left_pt + px_to_pt(frag.x)` where the
  sibling is f32 pt → `margin_left_pt + frag.x.in_pt().to_f32()`. (`.in_pt()`
  returns `Pt`; there is no `Add<Pt> for f32`, so `.to_f32()` is required.
  `.in_pt().to_f32()` ≡ `px_to_pt`, ×0.75, byte-identical.)
- **Spike-migrated** (render.rs ~2321/2323/2324):
  `margin_left_pt.pt() + target_frag.x.px().in_pt()` → drop the spike `.px()`
  tag → `margin_left_pt.pt() + target_frag.x.in_pt()`.

## Verification commands (end of every task)

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo build                                          # Finished
cargo clippy -p fulgur --all-targets -- -D warnings  # clean
cargo fmt --check                                    # clean
cargo test -p fulgur --lib                           # 1497 pass (Task 1: +1)
cargo test -p fulgur --test render_smoke             # 171 pass
cargo test -p fulgur-cli --test examples_determinism # 11 passed
cargo test -p fulgur-vrt                             # run_fulgur_vrt ... ok
git status --short -- crates/fulgur-vrt/goldens/     # MUST be empty
```

**Baseline captured GREEN** before any edit on the SAME base commit
(`origin/main` `d6903b08`, includes units + P1a + P1b paragraph + P1d
multicol): `--lib` 1497, render_smoke 171, examples_determinism 11/0, VRT ok,
goldens clean. This attributes any later red to the change, not font drift.

---

### Task 1: Add `Px::ZERO` helper (+ unit test)

`frag.height > 0.0` appears 3× in render.rs (~1654, ~2070, ~2623). Mirror the
P1d convention (`r.width > Pt::ZERO`) with a typed zero so the migrated
comparison reads `frag.height > Px::ZERO`. `units` is `pub mod` (lib.rs:63)
and `Px` is a `pub` type, so a `pub const` is exempt from dead_code even before
Task 2 uses it. Add ONLY `ZERO` (genuinely used 3×); do NOT pre-add
`Px::max`/`min` — no current Px clamp exists (the gap clamp stays f32). If
Task 2's build surfaces a real need for `Px::max`/`min`, add them then.

**Files:** `crates/fulgur/src/units.rs` (the `impl Px { ... }` block ~137-149;
the `#[cfg(test)] mod tests` block).

**Step 1 — add the const** to `impl Px`, mirroring `Pt::ZERO`:

```rust
impl Px {
    /// The zero length. Use instead of `0.0_f32.px()` for clamp / sign
    /// idioms (`x > Px::ZERO`); the private field means `Px(0.0)` is not
    /// constructible outside this module.
    pub const ZERO: Px = Px(0.0);

    // ... existing to_f32 / in_pt ...
}
```

**Step 2 — add a unit test** in `mod tests`:

```rust
    #[test]
    fn px_zero_is_zero() {
        assert_eq!(Px::ZERO, Px(0.0));
        assert!(Px(1.0) > Px::ZERO);
        assert!(!(Px(0.0) > Px::ZERO));
    }
```

**Step 3 — verify:** `cargo build`; `cargo test -p fulgur --lib`
(now 1498 pass); `cargo clippy -p fulgur --all-targets -- -D warnings`;
`cargo fmt --check`. (No golden run needed — units-only change.)

**Step 4 — commit:**

```bash
git add crates/fulgur/src/units.rs
git commit -m "feat(units): add Px::ZERO mirroring Pt::ZERO (P1c prep)"
```

---

### Task 2: Migrate `Fragment` coordinate fields → `units::Px`

This is the atomic core: typing the struct field breaks compilation across all
7 files at once, so the build is red until every construction site AND every
reader is fixed. Work file-by-file off the `cargo build` worklist; do NOT run
goldens until the build is green.

**Files:**

- `crates/fulgur/src/pagination_layout.rs` — struct (~78); ~24 prod + test
  `Fragment { ... }` construction sites; internal `first_frag` readers
  (~2428/2486/2539) and test asserts (~3797 `(f.height - 30.0)`, ~4346
  `(frag.y - 770.0)`, ~4309/4341 etc.).
- `crates/fulgur/src/render.rs` — 31 `px_to_pt(frag.*)` readers; 3 spike
  `.px().in_pt()` readers (~2321/2323/2324); 3 `frag.height > 0.0`
  comparisons (~1654/2070/2623); 2 test helpers `make_fragment` (~4477),
  `make_frag_with_height` (~4755).
- `crates/fulgur/src/convert/positioned.rs` — readers `parent_frag.x`/`.y`
  (~267/268, px-space), tuple `(frag.x, frag.y)` (~1417); construction (~307).
- `crates/fulgur/src/gcpm/target_ref.rs` — construction sites (~400/407).
- `crates/fulgur/src/engine.rs` — test helper `frag_on_page` (~1444).
- `crates/fulgur/src/column_css.rs`, `crates/fulgur/src/blitz_adapter.rs` —
  test construction / asserts (build will surface).

**Step 1 — type the fields** (use fully-qualified `crate::units::Px` to avoid
the `type Pt = f32` alias-shadowed module; mirror P1a/P1d):

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Fragment {
    pub page_index: u32,
    pub x: crate::units::Px,
    pub y: crate::units::Px,
    pub width: crate::units::Px,
    pub height: crate::units::Px,
}
```

Update the struct doc comment: the fields are still "CSS pixels — Taffy's
native unit", now type-enforced.

**Step 2 — build to get the worklist:** `cargo build` lists every break.
Confirm `use crate::units::F32Units;` is in scope in each file that gains a
`.px()` (add the import if a break says `.px` is unknown).

**Step 3 — tag at construction** (the layout-math locals — `cursor_y`,
`child_h`, computed `x`/`w`/`h`, literals like `0.0` — are Taffy-space f32;
`.px()` is a pure label). At EVERY `Fragment { ... }`:

```rust
        Fragment {
            page_index,
            x: x.px(),
            y: cursor_y.px(),
            width: width.px(),
            height: child_h.px(),
        }
```

For literal constructions in tests, e.g. `x: 0.0` → `x: 0.0_f32.px()` (or
`x: 0.0.px()` if the receiver type infers; prefer the explicit suffix).
Test helpers (`frag_on_page`, `make_fragment`, `make_frag_with_height`) take
f32 params — `.px()` them at the `Fragment { ... }` they build.

**Step 4 — untag readers** (per the two-systems + reassociation + untag-at-use
rules above):

- **render.rs px→pt readers:** `px_to_pt(frag.x)` → `frag.x.in_pt().to_f32()`
  (same for `.y`/`.width`/`.height` and the `desc_frag`/`first_frag`/
  `target_frag` variants). Where the call uses `crate::convert::px_to_pt(...)`,
  the replacement is `frag.height.in_pt().to_f32()` likewise.
- **render.rs spike readers (~2321/2323/2324):** drop `.px()` →
  `target_frag.height.in_pt()`, `target_frag.x.in_pt()`, `target_frag.y.in_pt()`.
- **render.rs comparisons (~1654/2070/2623):** `frag.height > 0.0` →
  `frag.height > crate::units::Px::ZERO`.
- **convert/positioned.rs:** `let parent_x_px = parent_frag.x;` →
  `parent_frag.x.to_f32()` (the `_px` var stays f32 px-space; no conversion).
  Same for `parent_y_px`. Tuple `(frag.x, frag.y)` → `(frag.x.to_f32(), frag.y.to_f32())`
  IF the tuple is `(f32, f32)` (confirm from the worklist; if it propagates
  into a px→pt expression, follow the reassociation rule there instead).
- **pagination_layout.rs internal readers** (`first_frag.y`/`.height`, split
  accumulation): untag with `.to_f32()` preserving operand order; these are
  px-space f32 math, byte-identical. `.fragments.push/clear/iter/len/first`
  are Vec ops — unaffected.
- **test asserts** reading `.x`/`.y`/`.width`/`.height` (pagination_layout
  ~3797/4346/etc., column_css, blitz_adapter): append `.to_f32()`; keep the
  asserted numbers identical.

**Step 5 — verify** (full block above) → build/clippy/fmt clean; `--lib`,
render_smoke, examples_determinism, VRT all pass; `git status --short --
crates/fulgur-vrt/goldens/` **empty** (byte-identical goldens, no
`FULGUR_VRT_UPDATE`).

**Step 6 — commit:**

```bash
git add -A
git commit -m "refactor(pagination): type Fragment coords with units::Px (byte-neutral, P1c)"
```

---

### Task 3: Docs + final verification

**Files:** `docs/plans/2026-06-27-engine-layout-api-design.md` (append a P1c
outcome note under the phase-status section — find where P1a/P1b/P1d outcomes
are recorded and add P1c: `Fragment` coords → `Px`, internal pagination math
stays f32, byte-neutral), and commit this plan
(`docs/plans/2026-06-29-pagination-fragment-px.md`).

**Step 1** append the P1c outcome note; `npx markdownlint-cli2 'docs/plans/2026-06-29-pagination-fragment-px.md' 'docs/plans/2026-06-27-engine-layout-api-design.md'` → 0 errors.

**Step 2** run the full verification block → all green, goldens byte-identical.

**Step 3** commit:

```bash
git add docs/plans/2026-06-29-pagination-fragment-px.md docs/plans/2026-06-27-engine-layout-api-design.md
git commit -m "docs(2map): record P1c (pagination Fragment) outcome + plan"
```

> Coverage note: render_smoke + existing pagination/render unit tests exercise
> the changed producer/reader lines through `Engine::render_html`. If
> codecov/patch later flags an uncovered changed line, add a focused
> render_smoke case (per CLAUDE.md "Coverage scope"); confirm with
> `cargo llvm-cov nextest --workspace --exclude fulgur-vrt`.

---

## Done criteria (maps to `fulgur-2map.5`)

- [ ] `Fragment.x/.y/.width/.height` are `units::Px`; `page_index` unchanged.
- [ ] Internal pagination math (`cursor_y`/`page_height_px`/`child_h`/gap)
      stays f32; `.px()` only at the `Fragment` construction boundary.
- [ ] Producers tag at source; readers untag at use; `.in_pt()` /
      `.in_pt().to_f32()` only at the px→pt boundary; reassociation order
      preserved.
- [ ] `examples_determinism` + VRT goldens byte-identical (no
      `FULGUR_VRT_UPDATE`); `git status` on goldens empty.
- [ ] build / clippy `-D warnings` / fmt / `--lib` / render_smoke clean.
- [ ] `Px::ZERO` added (used 3×); `Px::max`/`min` NOT pre-added unless build
      required them. `type Pt = f32` alias untouched (P2).
- [ ] Unblocks `fulgur-2map.7` (P1e Drawables aggregate).
