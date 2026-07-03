# P2b — Consolidate px_to_pt/pt_to_px internal call sites (byte-neutral)

**Goal:** Retype `convert::size_in_pt` / `convert::layout_in_pt` to return `units::Pt`
tuples and consolidate the surrounding `px_to_pt` / `pt_to_px` raw-f32 call sites onto the
`units::Px` / `Pt` newtype methods (`.in_pt()` / `.in_px()`), removing the manual `.pt()`
re-tagging at their call sites — with **zero** change to any generated PDF byte.

**Architecture:** The conversion helpers live in `convert/mod.rs`. Their `f32` return values
are re-tagged with `.pt()` at ~15 call sites before landing in already-`Pt`-typed fields.
Retyping the helpers to `Pt` lets those re-tags drop; a minority of consumers that mix with
still-`f32` `Margin`/inset math or feed the deliberate pt-valued-f32 boundary
(`compute_transform`) instead switch from `.pt()` to a leading `.to_f32()`. `insert_block_entry`
is the one consumer that `.pt()`s **inside** the function rather than at the call site, so its
`width`/`height` params are retyped to `Pt` too — otherwise retyping `size_in_pt` would *add*
a boundary at `block.rs` instead of removing one.

**Tech Stack:** Rust, `crate::units::{Px, Pt, F32Units}`, Taffy layout, VRT byte-compare.

**Issue:** `fulgur-2map.12` (epic `fulgur-2map`). Depends on merged P2a (`fulgur-2map.8`).

---

## The only gate: byte-neutrality

Every edit is exactly one of two shapes and nothing else:

1. **Drop a `.pt()` re-tag** where the helper now already returns `Pt` and the sink is `Pt`.
2. **Add a leading `.to_f32()`** where the value must stay `f32` (mixes with `f32` insets, or
   feeds the pt-valued-`f32` `compute_transform` boundary, or a `Display` format arg).

**Never reassociate arithmetic.** `(border_w - left_inset - right_inset).max(0.0)` becomes
`(border_w.to_f32() - left_inset - right_inset).max(0.0)` — the `.to_f32()` goes on the
**first** operand so the existing f32 evaluation order is preserved bit-for-bit. Never fold a
`(a * 0.75) * p` into `(a * p) * 0.75`. Never touch the body of `compute_transform`.

**Verification cadence (after every task):**

```bash
cd <worktree>
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo build 2>&1 | tail -3
cargo clippy -p fulgur --all-targets -- -D warnings 2>&1 | tail -3
cargo fmt --check
cargo test -p fulgur --lib 2>&1 | tail -3            # incl. render_smoke
cargo test -p fulgur-cli --test examples_determinism 2>&1 | tail -3
cargo test -p fulgur-vrt 2>&1 | tail -3
git status --short -- crates/fulgur-vrt/goldens/      # MUST be empty
```

`git status` over `crates/fulgur-vrt/goldens/` non-empty ⇒ the batch was **not** byte-neutral;
revert and re-audit. **NEVER** `FULGUR_VRT_UPDATE=1` — a diff is the signal that a conversion
was wrong, not that the golden needs updating.

Baseline was captured GREEN on the clean worktree base before any edit (lib 1568,
examples_determinism 11, VRT clean).

---

## Task 1 — Centerpiece: retype `size_in_pt` / `layout_in_pt` → `Pt` (atomic)

Retyping the return type breaks every call site until all are fixed, so this task is one
atomic batch verified by the existing suite staying byte-identical.

**Files:**

- Modify: `crates/fulgur/src/convert/mod.rs` (helper defs `73-86`; call sites `245`, `329`,
  `401`, `623`, `1041`; test sites `1573`, `1583`, `1599`, `1610`)
- Modify: `crates/fulgur/src/convert/block.rs` (`19`, `insert_block_entry` sig `82-101`)
- Modify: `crates/fulgur/src/convert/inline_root.rs` (`28`, sinks `134-135`, `198-199`)
- Modify: `crates/fulgur/src/convert/list_item.rs` (`20`, sinks `57-58`, `111-112`, `204-205`,
  `275-276`)
- Modify: `crates/fulgur/src/convert/table.rs` (`32`, sinks `44-45`, `47-48`)
- Modify: `crates/fulgur/src/convert/replaced.rs` (`50`, sinks `59-60`, `68-69`; test `511`)

**Step 1 — Retype the helper definitions:**

```rust
/// Convert a Taffy `Layout` (CSS px) to PDF pt as `(x, y, width, height)`.
#[inline]
fn layout_in_pt(layout: &taffy::Layout) -> (Pt, Pt, Pt, Pt) {
    (
        layout.location.x.px().in_pt(),
        layout.location.y.px().in_pt(),
        layout.size.width.px().in_pt(),
        layout.size.height.px().in_pt(),
    )
}

/// Convert a Taffy `Size<f32>` (CSS px) to PDF pt as `(width, height)`.
#[inline]
fn size_in_pt(size: taffy::Size<f32>) -> (Pt, Pt) {
    (size.width.px().in_pt(), size.height.px().in_pt())
}
```

Input stays raw `f32` (`taffy::Layout` is out of epic scope). `x.px().in_pt()` ≡
`Pt(x * 0.75)` ≡ `px_to_pt(x).pt()` — same constant, same single multiply, byte-identical.
Bring `Pt` / `F32Units` into scope (`use crate::units::{Pt, F32Units};` or fully-qualify).

**Step 2 — Fix each call site by its audited fate:**

| Site | Current | Fate | Rewrite |
|------|---------|------|---------|
| `mod.rs:245` | `(x,y,_,_)=layout_in_pt(..); return (x.pt(),y.pt())` | drop `.pt()` | `return (x, y)` |
| `mod.rs:329` | `layout_in_pt` → `eprintln!("{}", x, …)` (debug) | `.to_f32()` (Display) | `x.to_f32()`, `y.to_f32()`, `width.to_f32()`, `height.to_f32()` in the args |
| `mod.rs:401` | `size_in_pt` → `compute_transform(&styles,w,h)` | `.to_f32()` (pt-f32 boundary) | `compute_transform(&styles, w.to_f32(), h.to_f32())` |
| `mod.rs:623` | `size_in_pt` → `compute_transform(&styles,width_pt,height_pt)` | `.to_f32()` | `compute_transform(&styles, width_pt.to_f32(), height_pt.to_f32())` |
| `mod.rs:1041` | `(border_w-left_inset-right_inset).max(0.0)` (insets `f32`) | leading `.to_f32()` | `(border_w.to_f32() - left_inset - right_inset).max(0.0)` and same for height |
| `block.rs:19` | `insert_block_entry(node,style,width,height,out)` | pass `Pt` (see Step 3) | unchanged call; params retyped |
| `inline_root.rs:134-135,198-199` | `width:width.pt(), height:height.pt()` (Pt `Size`) | drop `.pt()` | `width, height` |
| `list_item.rs:57-58,111-112,204-205,275-276` | `width:width.pt(), height:height.pt()` | drop `.pt()` | `width, height` |
| `table.rs:44-45` (`layout_size`), `47-48` (`width`/`cached_height`) | `width.pt()` / `height.pt()` into Pt fields | drop `.pt()` | `width` / `height` |
| `replaced.rs:59-60,68-69` (Pt `Size`) | `width.pt()`, `height.pt()` | drop `.pt()` | `width`, `height` |
| `replaced.rs:50` arithmetic | `(width-left_inset-right_inset).max(0.0)` (insets `f32`) | leading `.to_f32()` | `(width.to_f32() - left_inset - right_inset).max(0.0)` and same for height |

Audit note verified during survey: at `inline_root.rs`, `list_item.rs`, `table.rs` the
`width`/`height` bindings are used **only** as `.pt()`-into-`Pt`-`Size`/field sinks — no f32
arithmetic — so they drop cleanly. `replaced.rs:50` is the mixed case (both f32 inset math
**and** a Pt `Size` sink) → `.to_f32()` for the math, drop `.pt()` for the `Size`.

**Step 3 — Retype `insert_block_entry` params to `Pt`:**

```rust
fn insert_block_entry(
    node: &Node,
    style: BlockStyle,
    width: crate::units::Pt,
    height: crate::units::Pt,
    out: &mut crate::drawables::Drawables,
) {
    // …
    layout_size: Some(Size { width, height }),   // was width.pt() / height.pt()
    // …
}
```

`BlockEntry.layout_size` is `Option<Size>` and `Size.{width,height}: Pt`, so the two interior
`.pt()` re-tags drop. This is the one consumer whose `.pt()` was inside the function; retyping
it is what keeps `block.rs` a net removal instead of a net addition.

**Step 4 — Fix the 4 test sites + 1 in `replaced.rs`:** `mod.rs:1573/1583` bind
`(x,y,w,h)=layout_in_pt(..)` and `1599/1610` bind `(w,h)=size_in_pt(..)`, then assert against
`Pt`-or-`f32` expectations; `replaced.rs:511` binds `(layout_w,layout_h)` then
`assert!(content_w <= layout_w, "… {content_w} > {layout_w}")`. Apply `.to_f32()` (or compare
against `.pt()`-tagged expectations) so the assertions still type-check and print identically.
These are `#[cfg(test)]`, so they never affect PDF bytes — pick whichever keeps the assert
readable; prefer comparing typed-to-typed and only `.to_f32()` in the `Display` message args.

**Step 5 — Verify (full cadence above), then commit:**

```bash
git add crates/fulgur/src/convert/
git commit -m "refactor(convert): retype size_in_pt/layout_in_pt to units::Pt (P2b, byte-neutral)"
```

---

## Task 2 — Secondary: `px_to_pt` → `.in_pt()` in enumerated files

**Scope boundary (authoritative):** only `convert/{list_marker, table, list_item, positioned,
inline_root, pseudo}.rs` and `convert/style/shadow.rs`. **Not** `mod.rs`, `blitz_adapter.rs`,
`border.rs`, `paragraph.rs`, `background.rs`, or `render.rs` (none are in the design's
secondary list; `background.rs` is explicitly un-migrated f32-on-both-sides).

**Per-site rule:** replace `px_to_pt(v)` with `v.in_pt()` **only** when `v` is genuinely raw
CSS-px `f32` and the result composes with an already-`Pt`/`Px`-typed field. `px_to_pt(v)` →
`v.px().in_pt()` when `v` is bare `f32`; `v.in_pt()` (no `.px()`) when `v` is already `Px`.
Where the result lands in a bare `f32` local with no typed neighbor, **leave it** — there is no
transitional gap to close and forcing it only adds noise.

**Clear wins (confirmed during survey):**

- `list_marker.rs:39-40` `px_to_pt(iw as f32).pt()` → `(iw as f32).px().in_pt()`
- `list_marker.rs:91-92` `px_to_pt(size.width()).pt()` → `size.width().px().in_pt()`
  (and `size.height()`)

**Audit individually (leave if no typed neighbor):** `list_marker.rs:191,239`;
`table.rs:114-115` (check what `cw`/`ch` feed); `shadow.rs:25-28` (target field type);
`list_item.rs:85,89,140,146,151`; `positioned.rs:448,451-452`; `inline_root.rs:397`;
`pseudo.rs:259`. For each, verify the binding's actual current type with a quick read before
rewriting; do not pattern-match on syntax.

**Verify (full cadence), then commit:**

```bash
git add crates/fulgur/src/convert/
git commit -m "refactor(convert): consolidate px_to_pt onto Px::in_pt at typed boundaries (P2b)"
```

---

## Task 3 — Secondary: `pt_to_px` → `.in_px()` at typed boundaries

**In-scope candidates only:** `convert/positioned.rs:89-91,270-271` and `convert/pseudo.rs:258`.
Apply `(bl_pt + br_pt).in_px()` / `pseudo_w_pt.in_px()` / `parent_width.in_px()` **iff** the
inner operand is already `Pt`-typed (verify each first).

**Explicitly excluded:**

- `engine.rs` (18 sites) — public-API Config/Blitz viewport f32 boundary (permanent).
- `background.rs` (2 sites) — internal gradient math, un-migrated f32 both sides.
- `convert/mod.rs:1620`, `pagination_layout.rs:4402,4469` — `#[cfg(test)]` bare `f32`
  literals / round-trip test of the helpers themselves; no typed neighbor.
- **`render.rs` (7 `pt_to_px`, 10 `px_to_pt`) — DEFERRED.** Not in the design's enumerated
  list. If the "borders a typed value" criterion turns out to spread across render.rs's 17
  sites, that is a separate, reviewable follow-up — file it, don't fold it into this PR
  (keeps the byte-verification surface bounded and the diff bisectable). Log the deferral.

**Verify (full cadence), then commit:**

```bash
git add crates/fulgur/src/convert/
git commit -m "refactor(convert): consolidate pt_to_px onto Pt::in_px at typed boundaries (P2b)"
```

---

## Task 4 — Final verification, coverage, docs

**Step 1 — Full byte-neutral proof:** run the complete cadence one more time from a clean
`git status`; confirm `crates/fulgur-vrt/goldens/` untouched and lib/determinism/VRT all green.

**Step 2 — Coverage:** the new `.to_f32()` sites in `compute_content_box` (`mod.rs:1041`) and
the transform-detection path (`mod.rs:401/623`) are PROD paths already exercised by
render_smoke / VRT, so codecov/patch is covered. The one debug-only site (`mod.rs:329`
`debug_print_tree`) is not exercised; if codecov flags it, apply the `{:?}`+bare workaround
from memory `project_units_migration_patch_coverage` rather than adding a test for a dev-only
tree dump. No new PROD branch is introduced, so no new render_smoke test is required.

**Step 3 — Record outcome** in this plan's footer (what dropped `.pt()` vs. gained
`.to_f32()`, what was deferred), then close `fulgur-2map.12` and note that `fulgur-2map.10`
(P3b) is unblocked.

**Step 4 — markdownlint** this file: `npx markdownlint-cli2 'docs/plans/2026-07-04-p2b-consolidate-conversions.md'`.
