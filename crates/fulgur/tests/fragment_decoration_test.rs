//! What a fragmented container paints on the page it leaves — two
//! defects that put empty bordered rectangles into production output:
//! a fragment opened on a page where no child fits, and a bottom border
//! stroked across a fragmentainer cut.
//!
//! Both defects are invisible to the geometry table alone — one is about
//! a fragment that should not exist, the other about an edge that should
//! not be stroked — so these assert against the emitted content stream.
//! Fixtures are text-free on purpose: glyph runs would make the output
//! depend on the host's installed fonts (see CLAUDE.md, *Font
//! determinism caveat*), and nothing here needs them.

use fulgur::engine::Engine;

/// One stroked straight line recovered from a page's content stream, in
/// PDF points, **y measured down from the top of the page** — the space
/// the rest of fulgur works in.
///
/// The CTM is applied first (giving PDF's bottom-left origin), then the
/// result is flipped against the MediaBox height. Going through the CTM
/// rather than reading the raw operands keeps this honest if the painter
/// ever nests another transform.
#[derive(Debug, Clone, Copy)]
struct Seg {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl Seg {
    fn is_horizontal(&self) -> bool {
        (self.y1 - self.y2).abs() < 0.01 && (self.x1 - self.x2).abs() > 0.01
    }
    fn is_vertical(&self) -> bool {
        (self.x1 - self.x2).abs() < 0.01 && (self.y1 - self.y2).abs() > 0.01
    }
    fn y_min(&self) -> f32 {
        self.y1.min(self.y2)
    }
    fn y_max(&self) -> f32 {
        self.y1.max(self.y2)
    }
}

type Matrix = [f32; 6];
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn apply(m: &Matrix, x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    // b applied first, then a — PDF's `cm` concatenates onto the CTM.
    [
        b[0] * a[0] + b[1] * a[2],
        b[0] * a[1] + b[1] * a[3],
        b[2] * a[0] + b[3] * a[2],
        b[2] * a[1] + b[3] * a[3],
        b[4] * a[0] + b[5] * a[2] + a[4],
        b[4] * a[1] + b[5] * a[3] + a[5],
    ]
}

fn nums(op: &lopdf::content::Operation) -> Vec<f32> {
    op.operands
        .iter()
        .filter_map(|o| match o {
            lopdf::Object::Integer(i) => Some(*i as f32),
            lopdf::Object::Real(r) => Some(*r),
            _ => None,
        })
        .collect()
}

/// The page's MediaBox height, used to flip PDF's bottom-left origin
/// into the top-down space `Seg` reports.
fn media_box_height(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> f32 {
    let media = doc
        .get_dictionary(page_id)
        .ok()
        .and_then(|d| d.get(b"MediaBox").ok())
        .and_then(|o| o.as_array().ok())
        .expect("page MediaBox");
    let v: Vec<f32> = media
        .iter()
        .filter_map(|o| match o {
            lopdf::Object::Integer(i) => Some(*i as f32),
            lopdf::Object::Real(r) => Some(*r),
            _ => None,
        })
        .collect();
    assert_eq!(v.len(), 4, "MediaBox is [llx lly urx ury]");
    v[3] - v[1]
}

/// Every stroked line segment on `page_index` (0-based).
///
/// Only `S` / `s` are collected: fills (backgrounds, glyph outlines) are
/// not borders and would drown the signal. Corner arcs are reduced to
/// their chord (see the `c` arm), so a rounded corner reads as a short
/// diagonal and a square one as no segment at all.
fn stroked_segments(pdf: &[u8], page_index: usize) -> Vec<Seg> {
    let doc = lopdf::Document::load_mem(pdf).expect("load pdf");
    let page_id = doc
        .page_iter()
        .nth(page_index)
        .unwrap_or_else(|| panic!("page {page_index} exists"));
    let bytes = doc.get_page_content(page_id).expect("page content");
    let content = lopdf::content::Content::decode(&bytes).expect("decode content");
    let page_height = media_box_height(&doc, page_id);

    let mut out = Vec::new();
    let mut ctm = IDENTITY;
    let mut stack: Vec<Matrix> = Vec::new();
    // Current subpath in *user* space, plus the pen position.
    let mut path: Vec<(f32, f32)> = Vec::new();

    for op in &content.operations {
        match op.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => ctm = stack.pop().unwrap_or(IDENTITY),
            "cm" => {
                let v = nums(op);
                if v.len() == 6 {
                    ctm = mul(&ctm, &[v[0], v[1], v[2], v[3], v[4], v[5]]);
                }
            }
            "m" | "l" => {
                let v = nums(op);
                if v.len() == 2 {
                    path.push(apply(&ctm, v[0], v[1]));
                }
            }
            // Corner arcs. Only the endpoint is kept, so a rounded corner
            // shows up as a short diagonal chord between the two edges it
            // joins — enough to tell "this corner is rounded" from "this
            // corner is square" without approximating the curve.
            "c" => {
                let v = nums(op);
                if v.len() == 6 {
                    path.push(apply(&ctm, v[4], v[5]));
                }
            }
            "re" => {
                let v = nums(op);
                if v.len() == 4 {
                    let (x, y, w, h) = (v[0], v[1], v[2], v[3]);
                    for (px, py) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)] {
                        path.push(apply(&ctm, px, py));
                    }
                }
            }
            "h" => {
                if let Some(&first) = path.first() {
                    path.push(first);
                }
            }
            "S" | "s" => {
                if op.operator == "s"
                    && let Some(&first) = path.first()
                {
                    path.push(first);
                }
                for w in path.windows(2) {
                    out.push(Seg {
                        x1: w[0].0,
                        y1: page_height - w[0].1,
                        x2: w[1].0,
                        y2: page_height - w[1].1,
                    });
                }
                path.clear();
            }
            "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => path.clear(),
            _ => {}
        }
    }
    out
}

/// `@page { size: 400px 300px; margin: 20px }` — a 360 x 260 px content
/// box, i.e. x 15 → 285 pt and y 15 → 210 pt.
const PAGE_CSS: &str = "@page { size: 400px 300px; margin: 20px } body { margin: 0 }";
const CONTENT_LEFT: f32 = 15.0;
const CONTENT_RIGHT: f32 = 285.0;
const CONTENT_BOTTOM: f32 = 210.0;
const CONTENT_TOP: f32 = 15.0;

/// Deliberately no `page_size` on the builder: an explicit one wins over
/// the document's own `@page`, and every fixture here depends on the
/// 400 x 300 px page it declares.
fn render(html: &str) -> Vec<u8> {
    Engine::builder().build().render(html).expect("render")
}

/// The two full-height side borders of a container that spans the whole
/// content width, identified by their x rather than by guessing at y.
fn side_borders(segs: &[Seg]) -> Vec<Seg> {
    segs.iter()
        .copied()
        .filter(|s| s.is_vertical())
        .filter(|s| (s.x1 - CONTENT_LEFT).abs() < 1.0 || (s.x1 - CONTENT_RIGHT).abs() < 1.0)
        .collect()
}

fn horizontals(segs: &[Seg]) -> Vec<Seg> {
    segs.iter().copied().filter(|s| s.is_horizontal()).collect()
}

/// Defect 1: a grid whose first row cannot fit the strip it landed on
/// must not open a fragment there. Before the fix the container claimed
/// the leftover strip and stroked a closed, empty bordered box on the
/// page it never placed a cell on.
///
/// A plain block in the same position already did the right thing (it
/// hands the break up), so `block` runs as the control — the three
/// display types must agree.
#[test]
fn container_with_no_room_for_its_first_row_paints_nothing_on_that_page() {
    for (label, display) in [
        ("grid", "display: grid; grid-template-columns: 1fr 1fr;"),
        ("flex", "display: flex; flex-wrap: wrap;"),
        ("block", ""),
    ] {
        // 245px of spacer leaves 15px of the 260px strip; the first row
        // needs 21px (20px cell + the container's 1px top border).
        let html = format!(
            r#"<html><head><style>
                 {PAGE_CSS}
                 .spacer {{ height: 245px }}
                 .tbl {{ {display} border: 1px solid #000 }}
                 .tbl > div {{ height: 20px; width: 50% }}
               </style></head><body>
                 <div class="spacer"></div>
                 <div class="tbl">
                   <div></div><div></div><div></div>
                   <div></div><div></div><div></div>
                 </div>
               </body></html>"#
        );
        let pdf = render(&html);
        let page0 = stroked_segments(&pdf, 0);
        assert!(
            page0.is_empty(),
            "[{label}] the container placed no cell on page 1, so it must not \
             stroke anything there; got {page0:?}"
        );
        // …and it is all there on page 2, as a normal closed box.
        let page1 = stroked_segments(&pdf, 1);
        let sides = side_borders(&page1);
        assert_eq!(
            sides.len(),
            2,
            "[{label}] expected both side borders on page 2; got {page1:?}"
        );
        let top = sides[0].y_min();
        let bottom = sides[0].y_max();
        assert!(
            (top - CONTENT_TOP).abs() < 1.0,
            "[{label}] the box starts at the top of page 2; got {top}"
        );
        assert!(
            horizontals(&page1).iter().any(|h| (h.y1 - top).abs() < 1.0),
            "[{label}] a box that moved whole keeps its top border; got {page1:?}"
        );
        assert!(
            horizontals(&page1)
                .iter()
                .any(|h| (h.y1 - bottom).abs() < 1.0),
            "[{label}] …and its bottom border; got {page1:?}"
        );
    }
}

/// Defect 2: `box-decoration-break: slice` (css-break-3 §4.3, the initial
/// value) gives the outgoing fragment no `border-bottom` and the incoming
/// one no `border-top`; only the side borders and the background run to
/// the fragmentainer edge.
///
/// Before the fix both fragments were closed rectangles, so the outgoing
/// one stroked a bottom edge at the page's content bottom — an empty
/// bordered band under the last row that fit.
#[test]
fn a_container_split_across_pages_leaves_the_cut_edges_open() {
    // Six 41px rows (40 + a 1px rule) inside a 1px border: 248px total,
    // starting 120px down a 260px strip, so three rows fit page 1.
    let html = format!(
        r#"<html><head><style>
             {PAGE_CSS}
             .spacer {{ height: 120px }}
             .tbl {{ border: 1px solid #000 }}
             .tbl > div {{ height: 40px; border-bottom: 1px solid #000 }}
           </style></head><body>
             <div class="spacer"></div>
             <div class="tbl">
               <div></div><div></div><div></div>
               <div></div><div></div><div></div>
             </div>
           </body></html>"#
    );
    let pdf = render(&html);

    // ── outgoing fragment ────────────────────────────────────────────
    let page0 = stroked_segments(&pdf, 0);
    let sides0 = side_borders(&page0);
    assert_eq!(
        sides0.len(),
        2,
        "the side borders continue to the fragmentainer edge; got {page0:?}"
    );
    let top0 = sides0[0].y_min();
    let cut0 = sides0[0].y_max();
    assert!(
        (cut0 - CONTENT_BOTTOM).abs() < 1.0,
        "the sides must run all the way to the page content bottom \
         ({CONTENT_BOTTOM}); got {cut0}"
    );
    assert!(
        horizontals(&page0)
            .iter()
            .any(|h| (h.y1 - top0).abs() < 1.0),
        "the leading edge is a real edge and keeps its border; got {page0:?}"
    );
    assert!(
        !horizontals(&page0)
            .iter()
            .any(|h| (h.y1 - cut0).abs() < 2.0),
        "the cut must NOT be stroked — that is the empty bordered band; \
         got {page0:?}"
    );

    // ── incoming fragment ────────────────────────────────────────────
    let page1 = stroked_segments(&pdf, 1);
    let sides1 = side_borders(&page1);
    assert_eq!(
        sides1.len(),
        2,
        "the continuation still has both side borders; got {page1:?}"
    );
    let cut1 = sides1[0].y_min();
    let bottom1 = sides1[0].y_max();
    assert!(
        (cut1 - CONTENT_TOP).abs() < 1.0,
        "the continuation starts at the page content top ({CONTENT_TOP}); \
         got {cut1}"
    );
    assert!(
        !horizontals(&page1)
            .iter()
            .any(|h| (h.y1 - cut1).abs() < 2.0),
        "the incoming cut must NOT be stroked either; got {page1:?}"
    );
    assert!(
        horizontals(&page1)
            .iter()
            .any(|h| (h.y1 - bottom1).abs() < 1.0),
        "…but the box's real bottom edge is still drawn; got {page1:?}"
    );
}

/// A box that fits on one page is not sliced, so nothing about it
/// changes: it still takes the closed-rectangle fast path with all four
/// edges. Guards the `OpenEdges::CLOSED` short-circuit in
/// `draw_block_border_sliced`.
#[test]
fn an_unsplit_box_keeps_all_four_edges() {
    let html = format!(
        r#"<html><head><style>
             {PAGE_CSS}
             .tbl {{ border: 1px solid #000; height: 60px }}
           </style></head><body><div class="tbl"></div></body></html>"#
    );
    let pdf = render(&html);
    let segs = stroked_segments(&pdf, 0);
    let sides = side_borders(&segs);
    assert_eq!(sides.len(), 2, "two side borders; got {segs:?}");
    let (top, bottom) = (sides[0].y_min(), sides[0].y_max());
    let hs = horizontals(&segs);
    assert!(
        hs.iter().any(|h| (h.y1 - top).abs() < 1.0),
        "top edge present; got {segs:?}"
    );
    assert!(
        hs.iter().any(|h| (h.y1 - bottom).abs() < 1.0),
        "bottom edge present; got {segs:?}"
    );
}

/// A cut squares off the two corners it touches, but the corners at the
/// box's *real* edges stay round. Dropping the radius wholesale would be
/// a visible regression on any split card — the case
/// `bugs/grid-row-promote-background` covers in VRT.
#[test]
fn a_split_rounded_box_keeps_the_corners_at_its_real_edges() {
    // Same shape as the plain split above, plus a radius.
    let html = format!(
        r#"<html><head><style>
             {PAGE_CSS}
             .spacer {{ height: 120px }}
             .tbl {{ border: 1px solid #000; border-radius: 8px }}
             .tbl > div {{ height: 40px }}
           </style></head><body>
             <div class="spacer"></div>
             <div class="tbl">
               <div></div><div></div><div></div>
               <div></div><div></div><div></div>
             </div>
           </body></html>"#
    );
    let pdf = render(&html);

    for (page, label) in [(0usize, "outgoing"), (1, "incoming")] {
        let segs = stroked_segments(&pdf, page);
        let sides = side_borders(&segs);
        assert_eq!(
            sides.len(),
            2,
            "[{label}] both sides run into the cut; got {segs:?}"
        );
        // Two corner chords — one per surviving corner. A square-cornered
        // slice would have none; an unsliced box would have four.
        let corners = segs
            .iter()
            .filter(|s| !s.is_horizontal() && !s.is_vertical())
            .count();
        assert_eq!(
            corners, 2,
            "[{label}] exactly the two corners at the real edge stay \
             rounded; got {segs:?}"
        );
    }

    // …and the cut itself is still unstroked, radius or no radius.
    let page0 = stroked_segments(&pdf, 0);
    assert!(
        !horizontals(&page0)
            .iter()
            .any(|h| (h.y1 - CONTENT_BOTTOM).abs() < 2.0),
        "the outgoing cut must not be stroked; got {page0:?}"
    );
    let page1 = stroked_segments(&pdf, 1);
    assert!(
        !horizontals(&page1)
            .iter()
            .any(|h| (h.y1 - CONTENT_TOP).abs() < 2.0),
        "the incoming cut must not be stroked; got {page1:?}"
    );
}

/// The other value of `box-decoration-break` (css-break-3 §5.4):
/// `clone` wraps every fragment in its own border, and §5.3 makes the
/// content box leave room for it inside the fragmentainer.
///
/// This is `repro/box-decoration-break-clone-ignored.html`: a one-column
/// grid of 40px rows split across two pages, rendered twice with the
/// declaration as the only difference. The `slice` half is the control —
/// it must not move.
///
/// Coordinates below are stroke *centres*, so a 1px (0.75pt) border
/// whose outer edge sits on the content edge strokes 0.375pt inside it.
#[test]
fn box_decoration_break_clone_wraps_each_fragment_and_reserves_room() {
    /// Every horizontal stroke's y on `page`, sorted.
    fn rules(pdf: &[u8], page: usize) -> Vec<f32> {
        let mut ys: Vec<f32> = horizontals(&stroked_segments(pdf, page))
            .iter()
            .map(|s| s.y1)
            .collect();
        ys.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        ys
    }

    fn render_with(decl: &str) -> Vec<u8> {
        let html = format!(
            r#"<html><head><style>
                 {PAGE_CSS}
                 .spacer {{ height: 120px }}
                 .tbl {{ display: grid; grid-template-columns: 1fr;
                         border: 1px solid #000; {decl} }}
                 .tbl > div {{ height: 40px; border-bottom: 1px solid #000 }}
               </style></head><body>
                 <div class="spacer"></div>
                 <div class="tbl">
                   <div></div><div></div><div></div>
                   <div></div><div></div><div></div>
                 </div>
               </body></html>"#
        );
        render(&html)
    }

    // Half a border width: an edge "at" C strokes its centre here.
    const HALF: f32 = 0.375;
    let near = |a: f32, b: f32| (a - b).abs() < 0.05;

    // --- control: the initial value, which must be untouched ----------
    let sliced = render_with("");
    assert_eq!(
        rules(&sliced, 0),
        vec![105.375, 136.125, 166.875, 197.625],
        "slice: three row rules plus the container's top border, and no \
         stroke at the cut"
    );
    assert_eq!(
        rules(&sliced, 1),
        vec![45.375, 76.125, 106.875, 107.625],
        "slice: the continuation has no top border either"
    );
    for s in side_borders(&stroked_segments(&sliced, 0)) {
        assert!(
            near(s.y_max(), CONTENT_BOTTOM),
            "slice: the sides run flush into the cut; got {s:?}"
        );
    }
    for s in side_borders(&stroked_segments(&sliced, 1)) {
        assert!(
            near(s.y_min(), CONTENT_TOP),
            "slice: the sides resume flush from the cut; got {s:?}"
        );
    }

    // --- clone --------------------------------------------------------
    let cloned = render_with("box-decoration-break: clone; -webkit-box-decoration-break: clone");
    let outgoing = rules(&cloned, 0);
    let incoming = rules(&cloned, 1);

    // §5.4, outgoing: a real bottom border, its OUTER edge on the
    // fragmentainer content edge — not tucked under the last row.
    assert!(
        outgoing.iter().any(|y| near(*y, CONTENT_BOTTOM - HALF)),
        "clone must close the outgoing fragment at {CONTENT_BOTTOM}; got \
         {outgoing:?}"
    );
    assert_eq!(
        outgoing.len(),
        rules(&sliced, 0).len() + 1,
        "the bottom border is the ONLY new stroke on the outgoing page; \
         got {outgoing:?}"
    );

    // §5.4, incoming: the top border repeats.
    assert!(
        incoming.iter().any(|y| near(*y, CONTENT_TOP + HALF)),
        "clone must re-wrap the incoming fragment at {CONTENT_TOP}; got \
         {incoming:?}"
    );

    // §5.3: that border is drawn *inside* the fragmentainer, so the
    // content box starts 0.75pt lower — every row rule on the incoming
    // page shifts down by exactly one border width.
    let sliced_incoming = rules(&sliced, 1);
    for y in &sliced_incoming {
        assert!(
            incoming.iter().any(|c| near(*c, y + 0.75)),
            "every slice rule at {y} must reappear 0.75pt lower under \
             clone; got {incoming:?}"
        );
    }

    // Both fragments are closed boxes: their side borders are inset by
    // half a border width at BOTH ends, where slice ran them flush into
    // the cut.
    let outgoing_sides = side_borders(&stroked_segments(&cloned, 0));
    assert_eq!(outgoing_sides.len(), 2, "got {outgoing_sides:?}");
    for s in &outgoing_sides {
        assert!(
            near(s.y_max(), CONTENT_BOTTOM - HALF),
            "the outgoing box closes at the fragmentainer edge; got {s:?}"
        );
    }
    let incoming_sides = side_borders(&stroked_segments(&cloned, 1));
    assert_eq!(incoming_sides.len(), 2, "got {incoming_sides:?}");
    for s in &incoming_sides {
        assert!(
            near(s.y_min(), CONTENT_TOP + HALF),
            "the incoming box opens at the fragmentainer edge; got {s:?}"
        );
    }
}
