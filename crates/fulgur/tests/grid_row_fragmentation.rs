//! What happens to a grid row that does not fit the strip it landed on.
//!
//! Two independent defects from one report, both reproduced here at the
//! `Engine` level rather than against the walk's internals:
//!
//! 1. `break-inside: avoid` was inert on grid items. The property never
//!    reached them — an item is usually a leaf the walk does not recurse
//!    into, and the block-level check is gated on a flag cleared for
//!    everything under a flex or grid container — so the row was cut by
//!    the fragmentainer and reopened on the next page, which is exactly
//!    what `avoid` asks the engine not to do.
//! 2. With no `break-inside` at all, a row whose cells are multi-line
//!    inline roots painted *below the page's content box* when the strip
//!    could not hold even one line box.
//!
//! Authority is CSS Fragmentation Module Level 3
//! (<https://www.w3.org/TR/css-break-3/>) and CSS Grid Level 1.
//!
//! Assertions read [`fulgur::engine::LayoutOutput::geometry`], the same
//! fragment table `render.rs` consumes, so "paints outside the page"
//! becomes the checkable "has a fragment extending past the page's
//! content height". Coordinates are CSS px, matching the table.

use fulgur::engine::Engine;

/// `@page { size: 400px 300px; margin: 20px }` — a 360 x 260 px content
/// box. Both defects were reported against this page.
const PAGE_CSS: &str = "@page { size: 400px 300px; margin: 20px } body { margin: 0 }";
const PAGE_H: f32 = 260.0;

/// `(page_index, y, height)` for one element, found by its `id`.
///
/// The geometry table is keyed by Blitz node id, which a test has no
/// stable handle on, so the element is located by matching a uniquely
/// sized fragment instead: every fixture below gives the elements it
/// asserts on a distinct width.
fn frags_by_width(out: &fulgur::engine::LayoutOutput, width: f32) -> Vec<(u32, f32, f32)> {
    let mut hits: Vec<(u32, f32, f32)> = out
        .geometry
        .values()
        .filter(|g| {
            g.fragments
                .first()
                .is_some_and(|f| (f.width.to_f32() - width).abs() < 0.51)
        })
        .flat_map(|g| g.fragments.iter())
        .map(|f| (f.page_index, f.y.to_f32(), f.height.to_f32()))
        .collect();
    hits.sort_by(|a, b| a.partial_cmp(b).expect("finite fragment coordinates"));
    hits
}

fn layout(html: &str) -> fulgur::engine::LayoutOutput {
    // Deliberately no `page_size` on the builder: an explicit one wins
    // over the document's own `@page`, and both fixtures depend on the
    // 400 x 300 px page they declare.
    Engine::builder().build().layout(html).expect("layout")
}

/// Defect 1. The leading row does not fit the strip left on page 1 but
/// does fit a fresh page, so `avoid` is fulfillable (css-break-3 §4.4)
/// and the whole row must move rather than be sliced at the page edge.
///
/// Run twice: once with each item in its own track, once with
/// `grid-column: span 3`. The report singled out the spanning case,
/// where the output was byte-identical with and without the
/// declaration; the property was in fact inert for both.
#[test]
fn break_inside_avoid_moves_a_whole_grid_row_to_the_next_page() {
    for (label, span) in [("single-track", ""), ("spanning", "grid-column: span 3;")] {
        let html = format!(
            r#"<html><head><style>
                 {PAGE_CSS}
                 body {{ font: 10px/1.4 sans-serif }}
                 .spacer {{ height: 248px }}
                 .tbl {{ display: grid; grid-template-columns: repeat(6, 1fr) }}
                 .c {{ height: 22px; box-sizing: border-box;
                       break-inside: avoid; page-break-inside: avoid }}
                 .lead {{ {span} }}
               </style></head><body>
                 <div class="spacer"></div>
                 <div class="tbl">
                   <div class="c lead"></div><div class="c lead"></div>
                   <div class="c"></div><div class="c"></div><div class="c"></div>
                   <div class="c"></div><div class="c"></div><div class="c"></div>
                 </div>
               </body></html>"#
        );
        let out = layout(&html);

        // The leading items are the widest cells in the fixture, so
        // their width identifies them: 180px when spanning three of six
        // 60px tracks, 60px otherwise. In the single-track case that is
        // every cell, which is fine — none of them may straddle.
        let lead_w = if span.is_empty() { 60.0 } else { 180.0 };
        let lead = frags_by_width(&out, lead_w);
        assert!(
            !lead.is_empty(),
            "[{label}] fixture must produce cells {lead_w}px wide; \
             geometry={:?}",
            out.geometry
        );
        assert!(
            lead.iter().all(|(p, _, _)| *p != 0),
            "[{label}] the leading row must move whole to page 2, not be \
             cut at the page edge; got {lead:?}"
        );
        for (p, y, h) in &lead {
            assert!(
                (h - 22.0).abs() < 0.51,
                "[{label}] and arrive intact, not as a slice; got \
                 (page {p}, y {y}, height {h})"
            );
        }
    }
}

/// The control for the test above: with the declaration removed the row
/// is sliced at the fragmentainer edge exactly as before, so the fix
/// reaches only documents that ask for it.
#[test]
fn without_break_inside_avoid_a_grid_row_is_still_sliced_at_the_page_edge() {
    let html = format!(
        r#"<html><head><style>
             {PAGE_CSS}
             body {{ font: 10px/1.4 sans-serif }}
             .spacer {{ height: 248px }}
             .tbl {{ display: grid; grid-template-columns: repeat(6, 1fr) }}
             .c {{ height: 22px; box-sizing: border-box }}
             .lead {{ grid-column: span 3 }}
           </style></head><body>
             <div class="spacer"></div>
             <div class="tbl">
               <div class="c lead"></div><div class="c lead"></div>
               <div class="c"></div><div class="c"></div><div class="c"></div>
               <div class="c"></div><div class="c"></div><div class="c"></div>
             </div>
           </body></html>"#
    );
    let out = layout(&html);
    let lead = frags_by_width(&out, 180.0);
    assert_eq!(
        lead,
        vec![
            (0, 248.0, 12.0),
            (0, 248.0, 12.0),
            (1, 0.0, 10.0),
            (1, 0.0, 10.0)
        ],
        "the initial value `auto` still slices the row across the page \
         boundary (two cells, two slices each)"
    );
}

/// Defect 2. No `break-inside` here — the initial `auto`. Whatever the
/// engine does with a row that does not fit, nothing may be placed below
/// the page's content box.
///
/// The fixture needs real text, because the defect is specific to the
/// path that splits an inline root at its line boundaries. It is kept
/// independent of the host's fonts (see CLAUDE.md, *Font determinism
/// caveat*) by fixing `line-height` and forcing the line count with
/// `<br>` rather than letting the text wrap: a wider or narrower face
/// changes neither the number of line boxes nor their height.
#[test]
fn a_straddling_grid_row_never_paints_below_the_fragmentainer() {
    // 252px of spacer leaves an 8px strip. Each cell wants 4px of
    // decoration plus two 14px lines, so not one line box fits and the
    // cell has to open on the next page.
    let html = format!(
        r#"<html><head><style>
             {PAGE_CSS}
             body {{ font-family: sans-serif; font-size: 10px; line-height: 14px }}
             .spacer {{ height: 252px }}
             /* Narrower than the 360px content box on purpose: it makes
                every node of the grid subtree identifiable by width,
                which is how the overflow check below excludes the
                fragmentation root. */
             .tbl {{ display: grid; grid-template-columns: repeat(6, 50px); width: 300px }}
             .c {{ border: 1px solid #000; padding: 3px; white-space: nowrap }}
           </style></head><body>
             <div class="spacer"></div>
             <div class="tbl">
               <div class="c">alpha<br>one</div><div class="c">alpha<br>two</div>
               <div class="c">alpha<br>three</div><div class="c">alpha<br>four</div>
               <div class="c">alpha<br>five</div><div class="c">alpha<br>six</div>
             </div>
           </body></html>"#
    );
    let out = layout(&html);

    // The fragmentation root (body) is exempt: it carries one fragment
    // spanning the whole flow rather than a page-bound box, which is the
    // same exemption the walk's own `find_overflowing_fragments` makes.
    // Everything narrower than the 360px content box belongs to the grid
    // subtree, which is what this defect is about.
    let overflowing: Vec<_> = out
        .geometry
        .iter()
        .flat_map(|(id, g)| g.fragments.iter().map(move |f| (*id, f)))
        .filter(|(_, f)| f.width.to_f32() <= 300.5)
        .filter(|(_, f)| f.y.to_f32() + f.height.to_f32() > PAGE_H + 0.01)
        .map(|(id, f)| {
            (
                id,
                f.page_index,
                f.y.to_f32(),
                f.height.to_f32(),
                f.y.to_f32() + f.height.to_f32() - PAGE_H,
            )
        })
        .collect();
    assert!(
        overflowing.is_empty(),
        "no fragment may extend past the {PAGE_H}px page content box; \
         (node, page, y, height, overshoot) = {overflowing:?}"
    );

    // …and the row genuinely moved rather than being trimmed to fit:
    // the cells are 50px wide and carry their whole 36px border box
    // (1px border + 3px padding + two 14px lines, both edges) on page 2.
    let cells = frags_by_width(&out, 50.0);
    assert!(
        !cells.is_empty(),
        "fixture must produce 50px cells; geometry={:?}",
        out.geometry
    );
    assert!(
        cells
            .iter()
            .all(|(p, y, h)| *p == 1 && y.abs() < 0.51 && (h - 36.0).abs() < 0.51),
        "every cell should open whole at the top of page 2; got {cells:?}"
    );
}

/// A third defect from the same neighbourhood, filed separately as
/// `todo/FULGUR_FLEX_GRID_CONTENT_LOSS.md`: a flex / grid child whose
/// first fragment is too short for its first line box lost that content
/// outright. Both fragments were still painted, so the symptom was an
/// empty two-page frame — a silently wrong document rather than a
/// visibly broken one.
///
/// Mechanism. A single-line inline root never reached the line splitter
/// (`fragment_inline_child` returned early on `len() <= 1`), so a grid
/// item pinned at its container's floor — which §3.2 forbids pushing —
/// fell through to R7b strip-slicing. Slicing is the right treatment for
/// a monolithic *box* and the wrong one for a box carrying a line: a
/// line box is itself monolithic, so cutting the box in two leaves the
/// line belonging to neither half, and `render`'s line partition, which
/// assigns whole lines to fragments by height budget, finds no fragment
/// that can hold it and emits none.
///
/// The window is a few pixels wide and moves with the box's decoration,
/// so this sweeps rather than spot-checks — acceptance criterion 3 of
/// the report. `display: block` is the control: it never reproduced,
/// because a block child may be pushed whole.
#[test]
fn a_flex_or_grid_child_never_loses_its_only_line_to_a_short_strip() {
    // `.c` is 22px tall: 1px border + 3px padding + a 14px line box,
    // both edges. The reported window was 247..251 for this decoration;
    // stripping the decoration widened it to 248..259. The sweep covers
    // every reported window at 1px resolution with room either side.
    for display in ["grid", "flex", "block"] {
        for decoration in [
            "border: 1px solid #000; padding: 3px",
            "padding: 3px",
            "border: 1px solid #000",
            "",
        ] {
            for spacer in 243..=275 {
                let html = format!(
                    r#"<!doctype html><html><head><style>
                         {PAGE_CSS}
                         body {{ font: 10px/1.4 sans-serif }}
                         .spacer {{ height: {spacer}px }}
                         .tbl {{ display: {display} }}
                         .c {{ {decoration} }}
                       </style></head><body>
                         <div class="spacer">spacer</div>
                         <div class="tbl"><div class="c">alpha one alpha two</div></div>
                       </body></html>"#
                );
                let out = layout(&html);
                let label = format!("display:{display} decoration:{decoration:?} spacer:{spacer}");

                // `.c` is its own inline root, so it is the node the
                // paragraph for "alpha ..." is keyed by — a precise
                // handle, and one that fails loudly if the box ever
                // stops resolving a paragraph at all. Width would not do
                // here: constraining `.c` to a distinctive width would
                // wrap the text and destroy the single-line shape this
                // test is about.
                let cell_id = out
                    .drawables
                    .paragraphs
                    .iter()
                    .find(|(_, p)| {
                        p.lines.iter().flat_map(|l| l.items.iter()).any(|i| {
                            matches!(
                                i,
                                fulgur::paragraph::LineItem::Text(t) if t.text.contains("alpha")
                            )
                        })
                    })
                    .map(|(id, _)| *id);
                let Some(cell) = cell_id.and_then(|id| out.geometry.get(&id)) else {
                    panic!("{label}: the cell must resolve a paragraph and appear in geometry");
                };
                let lead_in = cell.content_lead_in.to_f32();
                let lead_out = cell.content_lead_out.to_f32();
                let last = cell.fragments.len() - 1;
                let budget = |pos: usize, h: f32| {
                    let mut h = h;
                    if pos == 0 {
                        h -= lead_in;
                    }
                    if pos == last {
                        h -= lead_out;
                    }
                    h
                };
                let best = cell
                    .fragments
                    .iter()
                    .enumerate()
                    .map(|(pos, f)| budget(pos, f.height.to_f32()))
                    .fold(f32::MIN, f32::max);
                assert!(
                    best >= 14.0 - 0.51,
                    "{label}: some fragment must have room for the 14px line box, \
                     else `render` drops it and the box paints empty; \
                     best budget={best}, lead_in={lead_in}, lead_out={lead_out}, \
                     frags={:?}",
                    cell.fragments
                );

                // And nothing may hang past the fragmentainer while
                // doing it — the `252` row of the report, where the
                // surviving line was rebased into the page margin.
                //
                // The slack is `OVERSIZE_QUANTIZATION_TOLERANCE_PX`, the
                // 1px the walk deliberately allows so Stylo/Taffy integer
                // rounding of a CSS `<length>` does not trip the slicing
                // gate. It is load-bearing here: `border: 1px` with no
                // padding puts the box's bottom edge exactly 1px past the
                // strip at spacer 245, `needs_leaf_slicing` declines by
                // that tolerance, and the box is emitted whole. That
                // predates this fix and is on a path it does not touch —
                // the line fits the strip there, so the guard added to
                // `fragment_inline_child` returns `None` and the walk is
                // byte-for-byte what it was.
                for f in &cell.fragments {
                    assert!(
                        f.y.to_f32() + f.height.to_f32() <= PAGE_H + 1.01,
                        "{label}: fragment escapes the {PAGE_H}px content box: {f:?}"
                    );
                }
            }
        }
    }
}

/// `todo/FULGUR_FRAGMENT_BOUNDARY_LINE_LOSS.md`: a multi-line child that
/// straddles a page break placed some of its lines and dropped the rest
/// from the document. Not deferred, not carried onto a third fragment —
/// absent, with no overflow page to find them on.
///
/// The cause was two independent accountings of one decision.
/// `pagination_layout` split the paragraph using Parley's line
/// coordinates, which are rounded to whole pixels; `render` re-derived
/// that split from the resulting fragment *heights* and
/// `ShapedLine::height`, which keeps the fractional line height. A
/// 14.4px line reads as 14px to the fragmenter and 10.8pt to the
/// consumer, so a fragment budgeted for two lines admitted one — and the
/// lines past the last fragment's budget had nowhere to go.
///
/// The fix publishes the partition the fragmenter chose
/// (`PaginationGeometry::line_boundaries`). This asserts the invariant
/// that makes the loss impossible: the partition is a cover of the
/// paragraph's own line vector, terminated by its length, so every line
/// belongs to exactly one fragment. `render` checks that same
/// terminator before trusting the partition, so agreement here is what
/// keeps the authoritative path live rather than silently falling back.
///
/// Swept across the whole straddling window — the report measured loss
/// at *every* offset at which the box actually fragments, a 46px range,
/// not a narrow coincidence — and across the line counts that put the
/// box over two, three and four fragmentainers.
#[test]
fn a_straddling_multi_line_child_keeps_every_line() {
    for display in ["grid", "flex", "block"] {
        for lines in [4, 8, 20] {
            for spacer in (190..=245).step_by(5) {
                let body = (1..=lines)
                    .map(|i| format!("L{i}"))
                    .collect::<Vec<_>>()
                    .join("<br>");
                let html = format!(
                    r#"<!doctype html><html><head><style>
                         {PAGE_CSS}
                         body {{ font: 12px sans-serif }}
                         .spacer {{ height: {spacer}px }}
                         .tbl {{ display: {display} }}
                         .c {{ border: 1px solid #000; padding: 2px 4px }}
                       </style></head><body>
                         <div class="spacer">spacer</div>
                         <div class="tbl"><div class="c">{body}</div></div>
                       </body></html>"#
                );
                let out = layout(&html);
                let label = format!("display:{display} lines:{lines} spacer:{spacer}");

                // Same handle as the sweep above: `.c` is its own inline
                // root, so it owns the paragraph carrying "L1".
                let (cell_id, para) = out
                    .drawables
                    .paragraphs
                    .iter()
                    .find(|(_, p)| {
                        p.lines.iter().flat_map(|l| l.items.iter()).any(|i| {
                            matches!(
                                i,
                                fulgur::paragraph::LineItem::Text(t) if t.text.contains("L1")
                            )
                        })
                    })
                    .map(|(id, p)| (*id, p))
                    .unwrap_or_else(|| panic!("{label}: the cell must resolve a paragraph"));
                let geom = out
                    .geometry
                    .get(&cell_id)
                    .unwrap_or_else(|| panic!("{label}: the cell must appear in geometry"));

                if geom.fragments.len() < 2 {
                    // Fits one strip, or moved whole — nothing to cover.
                    continue;
                }

                let b = &geom.line_boundaries;
                assert_eq!(
                    b.len(),
                    geom.fragments.len() + 1,
                    "{label}: a line-split inline root must publish one boundary                      per fragment plus the terminator; got {b:?} for {} fragments",
                    geom.fragments.len()
                );
                assert_eq!(
                    b.first(),
                    Some(&0),
                    "{label}: the first fragment opens on line 0; got {b:?}"
                );
                assert_eq!(
                    b.last(),
                    Some(&para.lines.len()),
                    "{label}: the partition must terminate at the paragraph's own                      line count ({}), else `render` cannot trust it and every line                      past the last fragment's height budget leaves the document;                      got {b:?}",
                    para.lines.len()
                );
                assert!(
                    b.windows(2).all(|w| w[0] < w[1]),
                    "{label}: every fragment must carry at least one line; got {b:?}"
                );
            }
        }
    }
}
