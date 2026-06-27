# fulgur-xa9q: abs page extension with in-flow content — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow body-direct `position:absolute` subtrees to extend the page count
when their start lands at/beyond the in-flow page budget (Chrome-compatible),
without regressing clipped/size-contained decorations, so WPT
`fixedpos-005/006-print` pass (count + pixel) with zero regressions.

**Architecture:** Replace the coarse body-level `may_extend_pages =
!body_has_in_flow_content` gate inside `record_subtree_fragments_at_offset`'s
`walk` with a per-element rule: an element whose START page is at/beyond
`total_pages` extends pages, **unless** it sits inside a `contain:size`
containment boundary (threaded down `walk` as `containment_boundary: bool`).
Fixing the count exposes two further layers (per-page pixel placement of the
extended abs content; two `page-name-*` side-effect regressions) that must be
root-caused empirically before the gate is locked.

**Tech Stack:** Rust, Blitz/Stylo layout, Taffy, krilla PDF; WPT reftest harness
(`crates/fulgur-wpt`), pdftocairo rasterization.

**Design source of truth:** beads issue `fulgur-xa9q` `design` field (full root-cause
analysis + spike evidence). This plan operationalizes it.

**Critical conventions (do not skip):**

- Always run WPT/VRT with `FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"`
  (font determinism). The worktree has `target/wpt` symlinked to the main checkout.
- The WPT harness **exits 0 even on regression**. The ONLY truth signal is
  `target/wpt-report/css-page/regressions.json` (must be `[]`) and `summary.md`.
- New draw/convert logic needs lib-side tests too (codecov does not see VRT/WPT
  draw paths). Keep/extend tests in `crates/fulgur/tests/abs_positioned_pagination.rs`.
- Commit frequently with English messages.

---

## Task 1: Layer 1 — per-element extension gate with containment boundary

This is the validated spike change. The failing test that drives it is the
already-present (ignored) lib test `abs_extends_pages_despite_in_flow_content`.

**Files:**

- Modify: `crates/fulgur/src/pagination_layout.rs`
  (`record_subtree_fragments_at_offset` / inner `fn walk`, around lines 2908-3215)
- Modify: `crates/fulgur/tests/abs_positioned_pagination.rs:225-227`
  (remove `#[ignore]`)

**Step 1: Un-ignore the failing lib test**

In `crates/fulgur/tests/abs_positioned_pagination.rs`, delete the line:

```rust
#[ignore = "tracked by fulgur-xa9q: abs page extension with in-flow content"]
```

above `fn abs_extends_pages_despite_in_flow_content()`.

**Step 2: Run it to confirm it fails (baseline)**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" \
  cargo test -p fulgur --test abs_positioned_pagination abs_extends_pages_despite_in_flow_content
```

Expected: FAIL — `left: 1, right: 3` (abs clamped, only 1 page).

**Step 3: Thread `containment_boundary` through `walk` and refine the gate**

In `fn walk(...)` add a parameter after `may_extend_pages: bool,`:

```rust
        // true once at/below a `contain: size` ancestor. Inside such a
        // subtree the "START beyond budget extends" exception is suppressed
        // for descendants, so clipped overflow cannot generate pages
        // (page-background-003 cat box). The contained node itself is already
        // clamped by `is_size_contained`.
        containment_boundary: bool,
```

Replace the gate/clamp block (currently using `may_extend_pages`):

```rust
            if is_size_contained {
                last_page_f = first_page_f;
            }
            // An abs whose START is at/beyond the existing in-flow page budget
            // extends the page count even with in-flow content present
            // (Chrome-compatible) — UNLESS it sits inside a containment/clip
            // boundary, where overflow is invisible and must not paginate.
            let node_may_extend = may_extend_pages
                || (first_page_f >= total_pages as f32 && !containment_boundary);
            if first_page_f.is_finite()
                && last_page_f.is_finite()
                && first_page_f <= last_page_f
                && (node_may_extend || first_page_f < total_pages as f32)
            {
                let first_page = first_page_f as u32;
                let last_page = if node_may_extend {
                    last_page_f as u32
                } else {
                    (last_page_f as u32).min(total_pages.saturating_sub(1))
                };
```

At BOTH recursive `walk(...)` call sites (nested-abs branch and in-flow-child
branch) pass `containment_boundary || is_size_contained` in the new argument
position (after `may_extend_pages`).

At the top-level `walk(...)` call inside `record_subtree_fragments_at_offset`
pass `false` for `containment_boundary` (after `may_extend_pages`).

**Step 4: Run the lib tests**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" \
  cargo test -p fulgur --test abs_positioned_pagination -- --include-ignored
```

Expected: 10 passed, 0 failed. In particular `abs_extends_pages_despite_in_flow_content`
PASSes and `abs_positioned_does_not_force_extra_pages_for_short_flow` (budget-internal
tall abs stays 1 page) still PASSes.

**Step 5: Spot-check page counts on the four target tests (both sides)**

```bash
BIN=target/debug/fulgur; cargo build -q --bin fulgur
WPT=target/wpt/css/css-page
for t in fixedpos-005-print fixedpos-006-print fixedpos-008-print page-background-003-print; do
  for side in "" "-ref"; do
    FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" $BIN render "$WPT/$t$side.html" -o /tmp/x.pdf 2>/dev/null
    printf "%s%s pages=%s\n" "$t" "$side" "$(pdfinfo /tmp/x.pdf | awk '/^Pages:/{print $2}')"
  done
done
```

Expected (spike-confirmed): 005 test=5 ref=5, 006 test=5 ref=5, 008 test=6 ref=6,
page-background-003 test=2 ref=2.

**Step 6: Commit**

```bash
git add crates/fulgur/src/pagination_layout.rs crates/fulgur/tests/abs_positioned_pagination.rs
git commit -m "fix(pagination): per-element abs page extension gated by containment boundary (fulgur-xa9q layer 1)"
```

---

## Task 2: Establish the regression baseline (full css-page WPT)

**Step 1: Run the full css-page phase**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-wpt --test wpt_css_page
```

**Step 2: Read the truth signal**

```bash
cat target/wpt-report/css-page/summary.md
python3 -m json.tool target/wpt-report/css-page/regressions.json
```

Expected after Task 1 (spike-confirmed): 3 regressions remain —
`fixedpos-008-print` (page 2 diff ~1627px), `page-name-display-none-child-print`
(count 2 vs 3), `page-name-propagated-003-print` (page 1 diff ~142px). And
`fixedpos-005/006-print` now report a `page 2 diff` (count matched, pixels off).
Record this list; Tasks 3-4 must drive it to `[]`.

No commit (measurement only).

---

## Task 3: Layer 2 — page-2 pixel placement of extended abs content

The extended abs content lands ~1px off vs the ref's in-flow equivalent
(`vh`/page rounding). Root-cause first, then fix.

**Files:**

- Investigate/Modify: `crates/fulgur/src/pagination_layout.rs`
  (the `start_ratio` snap, ~lines 3023-3039; `final_y_for_paging` / `stored_y`)

**Step 1: Quantify the divergence**

```bash
ls target/wpt-run/fixedpos-005-print/diff/
```

Open `work/test/page-2.png` vs `work/ref/page-2.png` and the `diff/page2.diff.png`.
Measure the y-offset of the diverging text line in test vs ref (e.g. via the
existing `diff_pages` bin, or pixel inspection). Determine whether the abs stored
y for the extended page differs from the in-flow page-break y by a sub-px amount.

**Step 2: Write a focused lib test (TDD)**

Add to `crates/fulgur/tests/abs_positioned_pagination.rs` a test mirroring the
fixedpos-005 structure (in-flow `div height:300vh` + abs `top:100vh` whose text
must land at the same y as an equivalent in-flow first line on page 2). Assert on
the abs fragment's stored y (extract via the geometry/inspect path) OR on a
render-level invariant. Run; expect FAIL on the ~1px offset.

**Step 3: Fix the snap rule**

Make the extended-page abs `stored_y` use the same integer-multiple snapping the
in-flow fragmenter uses (reuse / align with the `snapped_start_ratio` logic so
`top:100vh` resolves to exactly the page boundary). Keep it behind the existing
finite/tolerance guards.

**Step 4: Verify**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" \
  cargo test -p fulgur --test abs_positioned_pagination -- --include-ignored
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-wpt --test wpt_css_page
python3 -m json.tool target/wpt-report/css-page/regressions.json
```

Expected: `fixedpos-005/006/008-print` no longer report a page-2 diff.

**Step 5: Commit**

```bash
git add -A && git commit -m "fix(pagination): align extended-page abs y-snap with in-flow page breaks (fulgur-xa9q layer 2)"
```

---

## Task 4: Layer 3 — page-name-* side-effect regressions

`page-name-display-none-child-print` (count 2 vs 3) and
`page-name-propagated-003-print` (page 1 ~142px) regress under the looser gate.

**Step 1: Root-cause each**

Read both test+ref HTML under `target/wpt/css/css-page/`. Determine why the
per-element gate changes their count/pixels (likely an abs whose start lands at a
named-page boundary now extends where it should not, or a `display:none`/named
child interaction). Use CLI page-count + diff images as in Task 2/3.

**Step 2: Add the minimal guard**

Tighten the gate ONLY as much as needed (e.g. additional containment/clip or
named-page boundary condition). Re-verify it does NOT undo Tasks 1-3. Add a lib
or smoke test capturing the guard if it is a pure-function condition.

**Step 3: Verify zero regressions**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-wpt --test wpt_css_page
cat target/wpt-report/css-page/summary.md   # regressions: 0
python3 -m json.tool target/wpt-report/css-page/regressions.json   # []
```

**Step 4: Commit**

```bash
git add -A && git commit -m "fix(pagination): guard abs extension at named-page boundaries (fulgur-xa9q layer 3)"
```

---

## Task 5: Promote WPT expectations + lib smoke coverage

**Files:**

- Modify: `crates/fulgur-wpt/expectations/css-page.txt:35-36` (and 38 if needed)
- Add: `crates/fulgur/tests/render_smoke.rs` (end-to-end smoke for abs extension)

**Step 1: Flip the target expectations FAIL -> PASS**

Change lines 35-36 from `FAIL` to `PASS` and replace the trailing
`# fulgur-xa9q: page count ...` note with `# fulgur-xa9q` (resolved). Confirm 008
and page-background-003 remain `PASS`.

**Step 2: Add a lib smoke test**

In `crates/fulgur/tests/render_smoke.rs` add an end-to-end smoke (in-flow text +
abs `bottom:-200vh`) asserting `!pdf.is_empty()` and the expected page count, so
the extension path has lib-side coverage independent of WPT.

**Step 3: Final full verification**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-wpt --test wpt_css_page
cat target/wpt-report/css-page/summary.md     # regressions: 0, fixedpos-005/006 now PASS
cargo clippy --all-targets 2>&1 | tail -5
cargo fmt --check
```

Expected: all lib tests pass, `regressions.json` is `[]`, clippy clean, fmt clean.

**Step 4: Commit**

```bash
git add -A && git commit -m "test(wpt): promote fixedpos-005/006 to PASS + abs-extension smoke (fulgur-xa9q)"
```

---

## Done criteria (maps to issue acceptance)

1. `fixedpos-005/006-print` PASS (count + pixel).
2. Zero regressions: protected set + `page-name-display-none-child` +
   `page-name-propagated-003` all PASS (`regressions.json == []`).
3. `abs_extends_pages_despite_in_flow_content` un-ignored and passing;
   `short_flow`/`nested_abs` lib tests preserved.
4. Extension rationale recorded in code comments (why START>=budget extends with
   in-flow present; why `contain:size` descendants clip).
