//! What happens to a grid row that does not fit the strip it landed on.
//!
//! With no `break-inside` at all — the initial `auto` — a row whose
//! cells are multi-line inline roots painted *below the page's content
//! box* when the strip could not hold even one line box. Reproduced here
//! at the `Engine` level rather than against the walk's internals.
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
/// box. The defect was reported against this page.
const PAGE_CSS: &str = "@page { size: 400px 300px; margin: 20px } body { margin: 0 }";
const PAGE_H: f32 = 260.0;

/// `(page_index, y, height)` for one element, found by its `id`.
///
/// The geometry table is keyed by Blitz node id, which a test has no
/// stable handle on, so the element is located by matching a uniquely
/// sized fragment instead: the fixture below gives the elements it
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
    // over the document's own `@page`, and the fixture depends on the
    // 400 x 300 px page it declares.
    Engine::builder().build().layout(html).expect("layout")
}

/// No `break-inside` here — the initial `auto`. Whatever the engine does
/// with a row that does not fit, nothing may be placed below the page's
/// content box.
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
