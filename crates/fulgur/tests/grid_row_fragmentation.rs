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
