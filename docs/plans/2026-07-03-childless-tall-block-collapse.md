# MAX_PAGES revert + childless tall-block collapse — Implementation Plan

**Goal:** Restore the 10k pagination DoS bound (regressed to 100k by commit
d3204be) and collapse pathologically tall *childless* blocks to a single
page so a 79-byte input stops amplifying into a huge render job.

**Architecture:** Two levers in `crates/fulgur`. (1) `MAX_PAGES` back to
`10_000` (absolute page-index ceiling; keeps the additive-not-multiplicative
property). (2) In the body-direct tall-block slice loop, when a block has no
rendered descendant content and its slicing would exceed the cap, gate the
per-page fragment `push` while leaving the loop's `page_index`/`cursor_y`/
`remaining` math untouched — so following siblings never reflow and a
trailing block collapses for free via `implied_page_count` (max fragment
index). Background presence does not gate the collapse.

**Tech Stack:** Rust, Blitz/Taffy layout, `PaginationGeometryTable`,
`cargo test`/`clippy`/`fmt`.

**Issue:** fulgur-ezst (design in its `design` field). Related: fulgur-2m6w.

**Scope boundary:** The `position:absolute; top:<huge>` page-extension path
(`pagination_layout.rs` ~3145) stays cap-bounded (clamped to `MAX_PAGES`),
NOT collapsed — a deliberate boundary. After the revert it yields up to 10k
bounded pages, not 1. Only the body-direct in-flow slice loop gets the
childless collapse.

---

## Task 1: Revert MAX_PAGES 100k → 10k

**Files:**

- Modify: `crates/fulgur/src/lib.rs:5-34` (doc comment + constant)

**Step 1: Replace the constant and its doc comment**

Replace the doc comment block (lines 5-33) and constant (line 34) with:

```rust
/// Per-element page-amplification bound (NOT a strict total-page ceiling —
/// see the note below). Bounds the per-page-strip slicing of an oversized
/// block in `pagination_layout` (and the absolute-positioning page-extension
/// path) so a tiny input with a pathologically tall CSS height / offset
/// (e.g. `height: 99999999px`, `position:absolute; top:99999999px`) cannot
/// force unbounded fragment / page generation — which downstream inflates a
/// `vec![Vec::new(); page_count]` allocation and a per-page render loop into
/// a CPU/memory-exhaustion DoS.
///
/// The cap is keyed off the running `page_index`, not a per-element budget.
/// That is deliberate: it makes many oversized siblings **additive** rather
/// than **multiplicative** — each extra oversized block contributes only its
/// own first fragment once the slice loop is capped, so the document total
/// settles at `MAX_PAGES + N` for `N` body children instead of
/// `N * MAX_PAGES`. A per-element budget would re-open that multi-element
/// amplification, so the cap must stay keyed off the absolute page index
/// (Codex review on PR #501).
///
/// Consequence: the total page count can slightly **exceed** `MAX_PAGES`
/// (it is `MAX_PAGES + N`, still input-proportional and rendered in bounded
/// time). This constant therefore caps single-element amplification, not the
/// absolute page count — code must not assume `page_count <= MAX_PAGES`.
///
/// Kept deliberately conservative for untrusted server-side rendering (a
/// shared multi-tenant renderer is the primary deployment): a tiny
/// pathological input must not amplify into one render iteration and PDF
/// page per fragment. The most common amplifier — a childless block with a
/// huge height, which prints only blank pages — is additionally collapsed to
/// a single page in `pagination_layout` (fulgur-ezst), so this ceiling only
/// bounds the residual content-bearing case. Content past the cap is
/// truncated (clamp-and-warn). Sibling of the `MAX_DOM_DEPTH` / background
/// `MAX_TILES` defensive bounds.
pub(crate) const MAX_PAGES: u32 = 10_000;
```

**Step 2: Build and run the pagination tests**

Run: `cargo test -p fulgur --lib pagination_layout 2>&1 | tail -20`
Expected: PASS. The existing `pathological_tall_block_is_page_capped` /
`f32_max_height_block_is_page_capped` assert `pages <= MAX_PAGES + 1`, which
still holds at 10k (they read the constant symbolically). Task 3 retargets
them to the collapse behavior.

**Step 3: Commit**

```bash
git add crates/fulgur/src/lib.rs
git commit -m "fix(pagination): revert MAX_PAGES ceiling 100k -> 10k (DoS regression)"
```

---

## Task 2: Add the `subtree_has_rendered_content` predicate

**Files:**

- Modify: `crates/fulgur/src/pagination_layout.rs` (add fn next to
  `record_subtree_descendants`, ~line 1116)

**Step 1: Add the predicate**

Insert immediately before `fn record_subtree_descendants` (line 1116):

```rust
/// fulgur-ezst: true if `parent_id`'s subtree renders any box (a descendant
/// with non-zero size). Mirrors `record_subtree_descendants`' walk — the
/// `layout_children` preference and the zero-size-container recursion (so a
/// `<tbody>`/`<tr>`/anonymous wrapper still counts its cells as content) —
/// but short-circuits on the first rendered descendant. Used to classify a
/// pathologically tall block as "childless" (collapsible). Conservative: a
/// subtree deeper than `MAX_DOM_DEPTH` reads as no content, matching
/// `record_subtree_descendants` (which also stops recording there, so such
/// content renders blank regardless). Keep this walk in sync with
/// `record_subtree_descendants`.
fn subtree_has_rendered_content(doc: &BaseDocument, parent_id: usize, depth: usize) -> bool {
    if depth >= crate::MAX_DOM_DEPTH {
        return false;
    }
    let Some(parent) = doc.get_node(parent_id) else {
        return false;
    };
    let layout_children_borrow = parent.layout_children.borrow();
    let walk_children: &[usize] = layout_children_borrow
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or(&parent.children);
    for &child_id in walk_children {
        let Some(child) = doc.get_node(child_id) else {
            continue;
        };
        let layout = child.final_layout;
        if layout.size.height <= 0.0 && layout.size.width <= 0.0 {
            if subtree_has_rendered_content(doc, child_id, depth + 1) {
                return true;
            }
            continue;
        }
        return true;
    }
    false
}
```

**Step 2: Build**

Run: `cargo build -p fulgur 2>&1 | tail -5`
Expected: compiles (a `dead_code` warning is fine until Task 3 wires it in;
if clippy denies warnings locally, proceed — Task 3 uses it immediately).

**Step 3: Commit**

```bash
git add crates/fulgur/src/pagination_layout.rs
git commit -m "feat(pagination): add subtree_has_rendered_content childless predicate"
```

---

## Task 3: Collapse pathological childless blocks in the slice loop (TDD)

**Files:**

- Test + Modify: `crates/fulgur/src/pagination_layout.rs` (slice loop
  ~978-1011; tests ~3961-3997)

**Step 1: Retarget the two existing cap tests to the collapse behavior
(write the failing tests)**

Replace `pathological_tall_block_is_page_capped` (lines ~3961-3979) and
`f32_max_height_block_is_page_capped` (lines ~3981-3997) with:

```rust
    /// fulgur-ezst: a tiny input with a pathologically tall CSS height on a
    /// CHILDLESS block prints only blank pages, so instead of slicing it to
    /// the `MAX_PAGES` cap (~10k blank pages) the fragmenter collapses it to
    /// a single page. `height: 99999999px` would otherwise slice into
    /// ~125 000 fragments — the small-input DoS.
    #[test]
    fn pathological_childless_tall_block_collapses() {
        let html = r#"<html><body><div style="height: 99999999px"></div></body></html>"#;
        let mut doc = parse(html, 600.0);
        let table = run_pass(&mut doc, 800.0);
        assert_eq!(
            implied_page_count(&table),
            1,
            "childless pathological height must collapse to a single page",
        );
    }

    /// fulgur-ezst: Stylo/Taffy clamps an absurd `<length>` to `f32::MAX`
    /// (≈3.4e38), the worst-case finite input. A childless block that tall
    /// also collapses to one page (the ceiling-bounded slice loop still runs
    /// its counter math but pushes no fragment).
    #[test]
    fn f32_max_childless_height_collapses() {
        let html = r#"<html><body><div style="height: 1e39px"></div></body></html>"#;
        let mut doc = parse(html, 600.0);
        let table = run_pass(&mut doc, 800.0);
        assert_eq!(
            implied_page_count(&table),
            1,
            "childless f32::MAX height must collapse to a single page",
        );
    }
```

**Step 2: Run to verify they FAIL**

Run: `cargo test -p fulgur --lib pathological_childless_tall_block_collapses f32_max_childless_height_collapses 2>&1 | tail -20`
Expected: FAIL — `implied_page_count` is `10_001`, not `1` (collapse not
implemented yet).

**Step 3: Implement the collapse in the slice loop**

In `pagination_layout.rs`, locate the block starting at `let mut remaining =
child_h - first_slice_h;` (line ~978). Insert the collapse decision after
`let mut last_slice_h = first_slice_h;` and gate the `push`:

```rust
                let mut remaining = child_h - first_slice_h;
                let mut last_slice_h = first_slice_h;
                // fulgur-ezst: a CHILDLESS block whose slicing would exceed
                // the cap is a pathological amplifier — `<div
                // style="height:99999999px">` is a web-only spacer/overflow
                // idiom that prints nothing but blank pages. Emit only its
                // first slice: skip the per-page fragment pushes while
                // leaving the loop's page_index / cursor_y / remaining math
                // untouched, so following in-flow siblings keep their exact
                // positions (no reflow) and a trailing block collapses for
                // free (`implied_page_count` reads the max fragment index).
                // Background / border presence does NOT gate this — nobody
                // authors a >MAX_PAGES-tall filled band on purpose. A
                // content-bearing block, or a childless band that fits
                // within the cap, is not collapsed and takes the
                // truncate-and-warn path below unchanged.
                let collapse_childless = self.page_height_px > 0.0
                    && (remaining / self.page_height_px).ceil() > crate::MAX_PAGES as f32
                    && !subtree_has_rendered_content(self.doc, child_id, 0);
                // fulgur-2m6w: cap the per-page-strip slicing at
                // `MAX_PAGES`. `child_h` is attacker-controlled CSS
                // (`height` / `vh`), so without this bound a few bytes of
                // HTML (`<div style="height:99999999px">`) generate ~10^5
                // fragments — and a non-finite height would never reduce
                // `remaining`, looping forever. The `page_index` ceiling is
                // load-bearing on its own (it stops the loop even when
                // `remaining` is `+inf`); content past the cap is truncated.
                while remaining > 0.0 && page_index < crate::MAX_PAGES {
                    page_index += 1;
                    last_slice_h = remaining.min(self.page_height_px);
                    if !collapse_childless {
                        self.geometry
                            .entry(child_id)
                            .or_default()
                            .fragments
                            .push(Fragment {
                                page_index,
                                x: frag_x.px(),
                                y: 0.0_f32.px(),
                                width: child_w.px(),
                                height: last_slice_h.px(),
                            });
                    }
                    remaining -= last_slice_h;
                }
                if remaining > 0.0 {
                    if collapse_childless {
                        log::warn!(
                            "pagination: collapsed a childless block of \
                             height {child_h}px (slicing would exceed the \
                             {}-page limit) to a single page (fulgur-ezst)",
                            crate::MAX_PAGES,
                        );
                    } else {
                        log::warn!(
                            "pagination: block height {child_h}px exceeds the \
                             {}-page limit; truncating remaining content to \
                             bound rendering (fulgur-2m6w)",
                            crate::MAX_PAGES,
                        );
                    }
                }
```

**Step 4: Run to verify the two tests PASS**

Run: `cargo test -p fulgur --lib pathological_childless_tall_block_collapses f32_max_childless_height_collapses 2>&1 | tail -20`
Expected: PASS.

**Step 5: Add the remaining unit tests (background, content-bearing cap,
sub-cap band)**

Append after `f32_max_childless_height_collapses`:

```rust
    /// fulgur-ezst: background presence does not gate the collapse — a
    /// childless filled band this tall is still a pathological amplifier.
    #[test]
    fn childless_tall_block_with_background_collapses() {
        let html =
            r#"<html><body><div style="height: 99999999px; background: red"></div></body></html>"#;
        let mut doc = parse(html, 600.0);
        let table = run_pass(&mut doc, 800.0);
        assert_eq!(implied_page_count(&table), 1);
    }

    /// fulgur-ezst: a tall block WITH rendered content is not childless, so
    /// it is NOT collapsed — it still clamps to ~MAX_PAGES (truncate-and-
    /// warn). Also guards routing: a huge parent with a small child must
    /// reach the slice loop, not the recursion branch (the recursion gate
    /// measures descendant overflow, not the parent's intrinsic height).
    #[test]
    fn content_bearing_tall_block_is_page_capped() {
        let html = r#"<html><body><div style="height: 99999999px"><p>x</p></div></body></html>"#;
        let mut doc = parse(html, 600.0);
        let table = run_pass(&mut doc, 800.0);
        let pages = implied_page_count(&table);
        assert!(
            pages > 1_000,
            "content-bearing block must take the cap path, not collapse; got {pages}",
        );
        assert!(
            pages <= crate::MAX_PAGES + 1,
            "content-bearing block must stay clamped to ~MAX_PAGES ({}); got {pages}",
            crate::MAX_PAGES,
        );
    }

    /// fulgur-ezst: a childless band that fits WITHIN the cap renders its
    /// full page count — the collapse must not over-fire on ordinary
    /// multi-page spacers. `height: 4000px` on an 800px strip = 5 pages.
    #[test]
    fn childless_subcap_band_renders_full() {
        let html = r#"<html><body><div style="height: 4000px"></div></body></html>"#;
        let mut doc = parse(html, 600.0);
        let table = run_pass(&mut doc, 800.0);
        assert_eq!(
            implied_page_count(&table),
            5,
            "a sub-cap childless band must render fully, not collapse",
        );
    }
```

**Step 6: Run the full pagination test module**

Run: `cargo test -p fulgur --lib pagination_layout 2>&1 | tail -25`
Expected: PASS (new tests + unchanged `non_finite_height_treated_as_zero`,
abs-path cap tests which read `MAX_PAGES` symbolically).

**Step 7: Commit**

```bash
git add crates/fulgur/src/pagination_layout.rs
git commit -m "feat(pagination): collapse pathological childless tall blocks (fulgur-ezst)"
```

---

## Task 4: End-to-end smoke coverage

**Files:**

- Modify: `crates/fulgur/tests/render_smoke.rs` (~4683; add a new test)

**Step 1: Add the collapse smoke test**

Keep the existing `pathological_tall_block_render_is_bounded` (50000px, ~63
pages — a sub-cap childless band that is NOT collapsed and exercises the
multi-slice draw path). Add after it:

```rust
/// fulgur-ezst: a childless block tall enough to exceed the page cap must
/// collapse end-to-end — a real `render_html` yields a tiny, single-page
/// PDF rather than a ~10k-page one. Byte size cleanly separates the two: a
/// collapsed render is a few KB; the uncollapsed cap render would be MBs.
#[test]
fn childless_cap_exceeding_block_collapses_end_to_end() {
    let html = r#"<!doctype html><html><body><div style="height:99999999px"></div></body></html>"#;
    let pdf = Engine::builder()
        .build()
        .render_html(html)
        .expect("render must terminate and succeed");
    assert!(!pdf.is_empty(), "expected a non-empty PDF");
    assert!(
        pdf.len() < 500_000,
        "collapsed render must be small (single page); got {} bytes",
        pdf.len(),
    );
}
```

**Step 2: Run the smoke tests**

Run: `cargo test -p fulgur --test render_smoke childless_cap_exceeding_block_collapses_end_to_end pathological_tall_block_render_is_bounded 2>&1 | tail -15`
Expected: PASS (both).

**Step 3: Commit**

```bash
git add crates/fulgur/tests/render_smoke.rs
git commit -m "test(pagination): end-to-end childless collapse smoke test (fulgur-ezst)"
```

---

## Task 5: Full verification

**Step 1: Full fulgur test suite**

Run: `cargo test -p fulgur 2>&1 | tail -25`
Expected: all green (lib + integration). Watch for any test that hardcoded
100k-page expectations (grep found none, but confirm).

**Step 2: Lint**

Run: `cargo clippy -p fulgur --all-targets 2>&1 | tail -15`
Expected: no warnings (the predicate is now used).

Run: `cargo fmt --check 2>&1 | tail -5`
Expected: clean.

Run: `npx markdownlint-cli2 'docs/plans/2026-07-03-childless-tall-block-collapse.md' 2>&1 | tail -5`
Expected: 0 errors.

**Step 3: Manual DoS sanity check (optional, mirrors the finding's PoC)**

```bash
printf '%s' '<div style="height:99999999px"></div>' > /tmp/poc.html
cargo run -q -p fulgur-cli --release -- render /tmp/poc.html -o /tmp/poc.pdf
ls -l /tmp/poc.pdf   # expect a few KB, not ~31 MB
```

Expected: fast, tiny PDF (single page).

**Step 4: No commit** (verification only).
