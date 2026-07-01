# P2a: Remove `type Pt = f32` alias Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Delete the legacy `pub type Pt = f32;` alias in `draw_primitives.rs` so the
crate has exactly one `Pt` type (`units::Pt`), by retyping its last few consumers
(`Rect`, `DestinationRegistry`, `BookmarkCollector`/`BookmarkEntry`, `clamp_marker_size`,
`InlineBoxRenderCtx`, `SvgRender::draw`) to either `units::Pt` or plain `f32` as
appropriate, with zero change to rendered PDF bytes.

**Architecture:** This is a byte-neutral mechanical refactor, not new behavior, so there
is no "write a failing test first" step — the existing test suite + VRT goldens ARE the
spec. Each task retypes one struct/function, then the compiler (`cargo build -p fulgur`)
enumerates every call site that needs a matching fixup. Two fixup rules, applied per call
site (see `.claude/rules/coordinate-system.md` and `project_units_migration_patch_coverage`
memory):

- Caller already holds a `units::Pt`/`units::Px` value feeding the old bare-`f32`/`Pt`-alias
  parameter → **drop** the existing `.to_f32()` (or add nothing — it now flows through
  untagged).
- Caller holds a raw `f32` (from an out-of-scope source like `Margin`/`Config`, or a test
  literal) → **add** `.pt()` (from `crate::units::F32Units`) at the call site.
- The one exception is `InlineBoxRenderCtx` (Task 5): it has no typed neighbor at all (both
  sides are permanently-f32 margin plumbing), so the fix is a straight rename `Pt` → `f32`,
  not a retype.

After every task: `cargo build -p fulgur` must succeed with the fixups applied, then run
the file-local unit tests, then the full `cargo test -p fulgur --lib` + VRT + examples_determinism
at the end of the whole plan (Task 8) to prove byte-neutrality end-to-end.

**Tech Stack:** Rust, `units::Pt`/`units::Px` newtypes (`crates/fulgur/src/units.rs`,
already fully implemented — Add/Sub/Mul\<f32\>/Div\<f32\>/Div\<Self\>/Neg/PartialOrd/Sum/
ZERO/max/min/abs, no new operators needed), Krilla (`krilla::geom::Point`/`Transform`) as
the FFI boundary.

**Baseline (already verified in the worktree before this plan was written):**
`examples_determinism` 11/11 passed, VRT `run_fulgur_vrt` ok, `git status --short --
crates/fulgur-vrt/goldens/` empty, `cargo build -p fulgur` clean.

---

### Task 1: Retype `draw_primitives::Rect` to `units::Pt`

**Files:**

- Modify: `crates/fulgur/src/draw_primitives.rs` (struct def ~L281, `transform_rect`
  ~L204, `push_rect` ~L402, 12 test construction sites: L1640, L1663, L1672, L1690,
  L1960, L1980, L2194, L2204, L2238, L2248, L2357, L2375)
- Modify: `crates/fulgur/src/paragraph.rs` (3 production construction sites: text-run
  link rect ~L727, image link rect ~L769, inline-box link rect ~L866)

**Step 1: Retype the struct**

```rust
// draw_primitives.rs — was: pub x: f32, pub y: f32, pub width: f32, pub height: f32
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: crate::units::Pt,
    pub y: crate::units::Pt,
    pub width: crate::units::Pt,
    pub height: crate::units::Pt,
}
```

**Step 2: Fix `transform_rect`** — untag right before the f32-only `transform_point` call
(a/b/c/d stay dimensionless f32; this is the correct FFI-ish boundary landing point now
that Rect itself is typed):

```rust
pub fn transform_rect(&self, r: &Rect) -> Quad {
    let x0 = r.x.to_f32();
    let y0 = r.y.to_f32();
    let x1 = (r.x + r.width).to_f32();
    let y1 = (r.y + r.height).to_f32();
    let bl = self.transform_point(x0, y1);
    let br = self.transform_point(x1, y1);
    let tr = self.transform_point(x1, y0);
    let tl = self.transform_point(x0, y0);
    Quad {
        points: [[bl.0, bl.1], [br.0, br.1], [tr.0, tr.1], [tl.0, tl.1]],
    }
}
```

**Step 3: Fix `push_rect`'s degenerate-rect guard** — use the operator, not a to_f32 compare:

```rust
pub fn push_rect(&mut self, link: &std::sync::Arc<crate::paragraph::LinkSpan>, rect: Rect) {
    if rect.width <= crate::units::Pt::ZERO || rect.height <= crate::units::Pt::ZERO {
        return;
    }
    // ... unchanged below
```

**Step 4: Fix the 3 production construction sites in `paragraph.rs`** — each currently
`.to_f32()`s an already-Pt value into the old f32 `Rect`; drop the `.to_f32()` calls. Example
(text-run link rect, ~L727-734):

```rust
// BEFORE
let rect = crate::draw_primitives::Rect {
    x: (x + run.x_offset).to_f32(),
    y: line_top_abs.to_f32(),
    width: run_width.max(crate::units::Pt::ZERO).to_f32(),
    height: line.height.to_f32(),
};
// AFTER
let rect = crate::draw_primitives::Rect {
    x: x + run.x_offset,
    y: line_top_abs,
    width: run_width.max(crate::units::Pt::ZERO),
    height: line.height,
};
```

Apply the same drop-`.to_f32()` pattern to the image link rect (~L769, fields
`(x + img.x_offset)`, `(y + img.computed_y)`, `img.width.max(Pt::ZERO)`,
`img.height.max(Pt::ZERO)`) and the inline-box link rect (~L866, fields `ox`, `oy`,
`ib.width.max(Pt::ZERO)`, `ib.height.max(Pt::ZERO)`).

**Step 5: Fix the 12 test construction sites in `draw_primitives.rs`** — every one is a
literal-field-value `Rect { x: 0.0, y: 0.0, width: N.0, height: N.0 }`; append `.pt()`
(from `crate::units::F32Units`, already imported in this test module as
`use crate::units::F32Units;`) to every literal field value. Example (L1640):

```rust
// BEFORE
Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 }
// AFTER
Rect { x: 0.0.pt(), y: 0.0.pt(), width: 10.0.pt(), height: 10.0.pt() }
```

Repeat identically at every other listed line (all are the same shape: 4 float literal
fields on a `Rect { ... }` struct literal).

**Step 6: Build and fix any remaining errors**

Run: `cargo build -p fulgur`
Expected: succeeds once all sites above are fixed. If the compiler flags a site not listed
above, apply the same rule (typed neighbor → drop conversion; raw literal/f32 → `.pt()`).

**Step 7: Run targeted tests**

Run: `cargo test -p fulgur --lib draw_primitives:: paragraph::`
Expected: all pass, same test count as baseline.

**Step 8: Commit**

```bash
git add crates/fulgur/src/draw_primitives.rs crates/fulgur/src/paragraph.rs
git commit -m "refactor(draw_primitives): type Rect with units::Pt (byte-neutral, P2a)"
```

---

### Task 2: Retype `DestinationRegistry` to `units::Pt`

**Files:**

- Modify: `crates/fulgur/src/draw_primitives.rs` (struct ~L22, `record`/`get` ~L65-75,
  test call sites ~L1608-1619, ~L2266-2268)
- Modify: `crates/fulgur/src/render.rs` (call site ~L120-122)

**Step 1: Retype the struct and methods**

```rust
pub struct DestinationRegistry {
    current_page_idx: usize,
    entries: BTreeMap<String, (usize, crate::units::Pt, crate::units::Pt)>,
    transform_stack: Vec<Affine2D>,
}

impl DestinationRegistry {
    // ...
    pub fn record(&mut self, id: &str, x: crate::units::Pt, y: crate::units::Pt) {
        let (tx, ty) = self.current_transform().transform_point(x.to_f32(), y.to_f32());
        self.entries
            .entry(id.to_string())
            .or_insert((self.current_page_idx, tx.pt(), ty.pt()));
    }

    pub fn get(&self, id: &str) -> Option<(usize, crate::units::Pt, crate::units::Pt)> {
        self.entries.get(id).copied()
    }
}
```

Note `transform_point` still takes/returns bare `f32` (a/b/c/d are dimensionless) — untag
with `.to_f32()` going in, re-tag with `.pt()` coming out. This is the same
tag-at-boundary pattern as Task 1's `transform_rect`.

**Step 2: Fix the `render.rs` call site** (~L120-122) — `x_pt`/`y_pt` are computed from a
mix of out-of-scope f32 `Margin` and already-Pt fragment values `.to_f32()`'d down to raw
f32; tag once at the call boundary:

```rust
// BEFORE
let x_pt = resolved_margin.left + first_frag.x.in_pt().to_f32();
let y_pt = resolved_margin.top + body_y_off + first_frag.y.in_pt().to_f32();
dest_registry.record(id.as_str(), x_pt, y_pt);
// AFTER
let x_pt = resolved_margin.left + first_frag.x.in_pt().to_f32();
let y_pt = resolved_margin.top + body_y_off + first_frag.y.in_pt().to_f32();
dest_registry.record(id.as_str(), x_pt.pt(), y_pt.pt());
```

(`x_pt`/`y_pt` themselves stay `f32` locals — only the call arguments are tagged. Do not
retype the locals; `resolved_margin.left`/`.top` are permanently-f32 `Margin` fields.)

**Step 3: Fix the 2 test blocks in `draw_primitives.rs`**

`destination_registry_push_pop_transform_affects_record` (~L1604-1620): tag the `record`
call literals with `.pt()`, untag the `get` result with `.to_f32()` at the assertion:

```rust
reg.record("anchor", 5.0.pt(), 7.0.pt());
let (page, x, y) = reg.get("anchor").expect("recorded");
assert_eq!(page, 3);
assert!((x.to_f32() - 15.0).abs() < 1e-4);
assert!((y.to_f32() - 27.0).abs() < 1e-4);
reg.pop_transform();
reg.record("anchor2", 1.0.pt(), 2.0.pt());
let (_, x2, y2) = reg.get("anchor2").expect("recorded");
assert!((x2.to_f32() - 1.0).abs() < 1e-4);
assert!((y2.to_f32() - 2.0).abs() < 1e-4);
```

`destination_registry_first_write_wins` (~L2266-2268, exact test name may differ slightly
— confirm via `grep -n "first-write-wins" -A5 crates/fulgur/src/draw_primitives.rs`): same
`.pt()` tagging on the two `record(...)` calls.

**Step 4: Build, test, commit**

Run: `cargo build -p fulgur` then `cargo test -p fulgur --lib draw_primitives:: render::`
Expected: clean build, all tests pass.

```bash
git add crates/fulgur/src/draw_primitives.rs crates/fulgur/src/render.rs
git commit -m "refactor(draw_primitives): type DestinationRegistry with units::Pt (byte-neutral, P2a)"
```

---

### Task 3: Retype `BookmarkCollector`/`BookmarkEntry.y_pt` to `units::Pt`

**Files:**

- Modify: `crates/fulgur/src/draw_primitives.rs` (`BookmarkEntry` ~L938,
  `BookmarkCollector::record` ~L963, test ~L2140-2157)
- Modify: `crates/fulgur/src/render.rs` (call site ~L478-479)
- Modify: `crates/fulgur/src/outline.rs` (`TreeNode.y_pt` ~L21, `to_krilla_node` ~L91-92,
  test helper `entry()` ~L104-111 and its 4 call sites ~L133-136, L174, L184)

**Step 1: Retype `BookmarkEntry`/`BookmarkCollector`**

```rust
pub struct BookmarkEntry {
    pub page_idx: usize,
    pub y_pt: crate::units::Pt,
    pub level: u8,
    pub label: String,
}

impl BookmarkCollector {
    // ...
    pub fn record(&mut self, level: u8, label: String, y_pt: crate::units::Pt) {
        self.entries.push(BookmarkEntry { page_idx: self.current_page_idx, y_pt, level, label });
    }
}
```

**Step 2: Fix the `render.rs` call site** (~L478-479), same tag-at-boundary pattern as Task 2:

```rust
let y_pt = margin_top_pt + first_frag.y.in_pt().to_f32();
c.record(anchor.level, anchor.label.clone(), y_pt.pt());
```

**Step 3: Fix `outline.rs`**

```rust
// TreeNode struct
pub(crate) struct TreeNode {
    pub label: String,
    #[allow(dead_code)]
    pub level: u8,
    pub page_idx: usize,
    pub y_pt: crate::units::Pt,
    pub children: Vec<TreeNode>,
}
```

`build_tree`'s `y_pt: e.y_pt` line needs no change (both sides now `units::Pt`).
`to_krilla_node` is the genuine krilla FFI boundary — untag there:

```rust
fn to_krilla_node(node: TreeNode) -> OutlineNode {
    let dest = XyzDestination::new(node.page_idx, Point::from_xy(0.0, node.y_pt.to_f32()));
    // ... unchanged below
```

Test helper and its imports — `outline.rs` currently does
`use crate::draw_primitives::{BookmarkEntry, Pt};`; drop `Pt` (no longer used after this
edit) and tag the helper's `y` parameter:

```rust
use crate::draw_primitives::BookmarkEntry;
// ...
fn entry(page: usize, y: crate::units::Pt, level: u8, label: &str) -> BookmarkEntry {
    BookmarkEntry { page_idx: page, y_pt: y, level, label: label.to_string() }
}
```

Update its 4 call sites (`entry(0, 10.0, 1, "Chapter 1")` etc. at ~L133-136, `entry(0,
10.0, 3, "Stray")` at ~L174, `entry(0, 10.0, 1, "A")` / `entry(0, 50.0, 3, "A.x")` at
~L184) to append `.pt()` to the numeric literal, e.g. `entry(0, 10.0.pt(), 1, "Chapter
1")`. Add `use crate::units::F32Units;` to the test module if not already present (check
first — `draw_primitives.rs`'s test module already imports it; `outline.rs`'s test module
may not).

**Step 4: Fix the `draw_primitives.rs` test** (~L2140-2157) — tag the `record(...)`
literals, untag `y_pt` at the assertion:

```rust
bc.record(1, "Chapter One".to_string(), 100.0.pt());
bc.set_current_page(5);
bc.record(2, "Section 5.1".to_string(), 42.0.pt());

let entries = bc.into_entries();
// ...
assert!((entries[0].y_pt.to_f32() - 100.0).abs() < 1e-5);
```

**Step 5: Build, test, commit**

Run: `cargo build -p fulgur` then `cargo test -p fulgur --lib draw_primitives:: outline:: render::`
Expected: clean build, all tests pass.

```bash
git add crates/fulgur/src/draw_primitives.rs crates/fulgur/src/render.rs crates/fulgur/src/outline.rs
git commit -m "refactor(draw_primitives): type BookmarkEntry.y_pt with units::Pt (byte-neutral, P2a)"
```

---

### Task 4: Retype `clamp_marker_size` to `units::Pt`

**Files:**

- Modify: `crates/fulgur/src/draw_primitives.rs` (`clamp_marker_size` ~L1580-1591, 4 tests
  ~L1865-1889)
- Modify: `crates/fulgur/src/convert/list_marker.rs` (`size_raster_marker` ~L31-42,
  `resolve_list_marker` Svg branch ~L88-99)

**Step 1: Retype `clamp_marker_size`** — internal ops become the existing `units::Pt`
operators (Div\<Self\>→f32 for the scale ratio, Mul\<f32\>→Pt for scaling, PartialOrd for
the comparisons):

```rust
pub(crate) fn clamp_marker_size(
    intrinsic_width: crate::units::Pt,
    intrinsic_height: crate::units::Pt,
    line_height: crate::units::Pt,
) -> (crate::units::Pt, crate::units::Pt) {
    if intrinsic_height <= crate::units::Pt::ZERO {
        return (crate::units::Pt::ZERO, crate::units::Pt::ZERO);
    }
    if intrinsic_height <= line_height {
        (intrinsic_width, intrinsic_height)
    } else {
        let scale = line_height / intrinsic_height; // Pt / Pt = f32
        (intrinsic_width * scale, line_height) // Pt * f32 = Pt
    }
}
```

**Step 2: Fix `list_marker.rs`'s `size_raster_marker`** — retype its return so the two
call sites in `resolve_list_marker` don't need to re-tag `width`/`height` before storing
into the already-`units::Pt` `ImageEntry`/`ListItemMarker` fields:

```rust
fn size_raster_marker(
    data: &Arc<Vec<u8>>,
    format: crate::image::ImageFormat,
    line_height: f32,
) -> Option<(crate::units::Pt, crate::units::Pt)> {
    let (iw, ih) = ImageRender::decode_dimensions(data, format)?;
    let intrinsic_w = px_to_pt(iw as f32).pt();
    let intrinsic_h = px_to_pt(ih as f32).pt();
    Some(crate::draw_primitives::clamp_marker_size(
        intrinsic_w,
        intrinsic_h,
        line_height.pt(),
    ))
}
```

(`line_height: f32` parameter itself is untouched — still fed from further up the call
chain as raw f32; only tagged right at the `clamp_marker_size` call.)

**Step 3: Fix `resolve_list_marker`'s two branches** — drop the now-redundant `.pt()` at
every `width`/`height` use (both `AssetKind::Raster` and `AssetKind::Svg` arms have 4 uses
each: `ImageEntry`/`SvgEntry` width+height, `ListItemMarker::Image` width+height):

```rust
// Raster arm — BEFORE: width.pt() / height.pt() (x4)
let (width, height) = size_raster_marker(data, format, line_height)?;
let entry = crate::drawables::ImageEntry {
    image_data: Arc::clone(data),
    format,
    width,
    height,
    opacity: 1.0,
    visible: true,
};
Some(ListItemMarker::Image { marker: ImageMarker::Raster(entry), width, height })
```

```rust
// Svg arm — tag intrinsic_w/intrinsic_h/line_height at the clamp_marker_size call,
// then drop .pt() at the 4 width/height uses below it
let intrinsic_w = px_to_pt(size.width()).pt();
let intrinsic_h = px_to_pt(size.height()).pt();
let (width, height) =
    crate::draw_primitives::clamp_marker_size(intrinsic_w, intrinsic_h, line_height.pt());
let entry = crate::drawables::SvgEntry {
    tree: Arc::new(tree),
    width,
    height,
    opacity: 1.0,
    visible: true,
};
Some(ListItemMarker::Image { marker: ImageMarker::Svg(entry), width, height })
```

**Step 4: Fix the 4 tests in `draw_primitives.rs`** (~L1865-1889) — tag the 3 literal
args, untag the 2 returned values at each assertion:

```rust
#[test]
fn clamp_marker_size_zero_height_returns_zero_zero() {
    let (w, h) = clamp_marker_size(20.0.pt(), 0.0.pt(), 12.0.pt());
    assert_eq!(w, crate::units::Pt::ZERO);
    assert_eq!(h, crate::units::Pt::ZERO);
}

#[test]
fn clamp_marker_size_negative_height_returns_zero_zero() {
    let (w, h) = clamp_marker_size(20.0.pt(), (-5.0).pt(), 12.0.pt());
    assert_eq!(w, crate::units::Pt::ZERO);
    assert_eq!(h, crate::units::Pt::ZERO);
}

#[test]
fn clamp_marker_size_within_line_height_passes_through() {
    let (w, h) = clamp_marker_size(20.0.pt(), 10.0.pt(), 12.0.pt());
    assert_eq!(w.to_f32(), 20.0);
    assert_eq!(h.to_f32(), 10.0);
}

#[test]
fn clamp_marker_size_oversized_scales_down_preserving_aspect() {
    let (w, h) = clamp_marker_size(40.0.pt(), 20.0.pt(), 10.0.pt());
    assert!((w.to_f32() - 20.0).abs() < 1e-5);
    assert!((h.to_f32() - 10.0).abs() < 1e-5);
}
```

**Step 5: Build, test, commit**

Run: `cargo build -p fulgur` then `cargo test -p fulgur --lib draw_primitives:: list_marker::`
Expected: clean build, all tests pass.

```bash
git add crates/fulgur/src/draw_primitives.rs crates/fulgur/src/convert/list_marker.rs
git commit -m "refactor(draw_primitives): type clamp_marker_size with units::Pt (byte-neutral, P2a)"
```

---

### Task 5: `InlineBoxRenderCtx` margin fields — rename `Pt` alias to plain `f32`

**Files:**

- Modify: `crates/fulgur/src/paragraph.rs` (`InlineBoxRenderCtx` ~L556-565)

**Context:** `margin_left_pt`/`margin_top_pt` are threaded as bare `f32` through ~10
`render.rs` function signatures (`draw_v2_page`, `dispatch_fragment`,
`dispatch_inline_box_content`, etc. — 142 occurrences of the identifier across the file).
`InlineBoxRenderCtx` is the only place holding these values under the `Pt` alias name.
There is no typed neighbor to bridge (same as `engine.rs`/`background.rs` in the P2b
design) — retyping to `units::Pt` here would only add pointless `.to_f32()` noise at its
1-2 read sites, or, if chased "properly" through its callees, balloon into an unrelated
~10-function render.rs ripple. The correct fix is a straight rename to match the type
these values actually have everywhere else.

**Step 1: Retype the two fields**

```rust
pub struct InlineBoxRenderCtx<'a> {
    pub drawables: &'a crate::drawables::Drawables,
    pub geometry: &'a crate::pagination_layout::PaginationGeometryTable,
    pub page_index: u32,
    pub margin_left_pt: f32,
    pub margin_top_pt: f32,
}
```

No other change needed — every reader/writer of `ctx.margin_left_pt`/`.margin_top_pt`
(the `geo_x_pt`/`geo_y_pt` computation and the `dispatch_inline_box_content` call, both in
`paragraph.rs`) already treats it as plain `f32`.

**Step 2: Build**

Run: `cargo build -p fulgur`
Expected: succeeds with no other changes required (this task is a pure signature
type-name swap since `Pt` already means `f32` today).

**Step 3: Commit**

```bash
git add crates/fulgur/src/paragraph.rs
git commit -m "refactor(paragraph): rename InlineBoxRenderCtx margin fields Pt alias to f32 (byte-neutral, P2a)"
```

---

### Task 6: `SvgRender::draw` — retype the last incidental `Pt`-alias usage

**Files:**

- Modify: `crates/fulgur/src/svg.rs` (`draw` signature ~L44-50, internal `Transform::from_translate`
  ~L60, test call site ~L105)

**Context:** `SvgRender` is confirmed dead code (not called from the actual v2 SVG draw
path in `render.rs`; only referenced there in a doc comment). It's still `pub mod svg`
with the crate's last `Pt`-alias usage, so it must be retyped (not left as-is) before the
alias declaration can be deleted in Task 7. Do not delete the struct itself — that's a
separate, out-of-scope cleanup.

**Step 1: Retype the signature and untag at the krilla FFI call**

```rust
pub fn draw(
    &self,
    canvas: &mut Canvas<'_, '_>,
    x: crate::units::Pt,
    y: crate::units::Pt,
    _avail_width: crate::units::Pt,
    _avail_height: crate::units::Pt,
) {
    use crate::draw_primitives::draw_with_opacity;
    use krilla_svg::{SurfaceExt, SvgSettings};

    if !self.visible {
        return;
    }
    draw_with_opacity(canvas, self.opacity, |canvas| {
        let Some(size) = krilla::geom::Size::from_wh(self.width, self.height) else {
            return;
        };
        let transform = krilla::geom::Transform::from_translate(x.to_f32(), y.to_f32());
        canvas.surface.push_transform(&transform);
        let _ = canvas.surface.draw_svg(&self.tree, size, SvgSettings::default());
        canvas.surface.pop();
    });
}
```

Drop the now-unused `use crate::draw_primitives::{Canvas, Pt};` import line down to
`use crate::draw_primitives::Canvas;`.

**Step 2: Fix the one test call site** (~L105, inside `draw_onto_surface`):

```rust
svg.draw(&mut canvas, 10.0.pt(), 20.0.pt(), 400.0.pt(), 400.0.pt());
```

Add `use crate::units::F32Units;` to the test module if not already present (check with
`grep -n "^    use" crates/fulgur/src/svg.rs`).

**Step 3: Build, test, commit**

Run: `cargo build -p fulgur` then `cargo test -p fulgur --lib svg::`
Expected: clean build, all tests pass.

```bash
git add crates/fulgur/src/svg.rs
git commit -m "refactor(svg): type dead SvgRender::draw with units::Pt (byte-neutral, P2a)"
```

---

### Task 7: Delete the `type Pt = f32` alias

**Files:**

- Modify: `crates/fulgur/src/draw_primitives.rs` (delete `pub type Pt = f32;` ~L79 and its
  doc comment)

**Step 1: Confirm no remaining bare references**

Run: `grep -rn '\bPt\b' crates/fulgur/src --include='*.rs' | grep -v 'units::Pt\|crate::units\|// '`
Expected: empty (or only non-code doc-comment prose mentioning "Pt" generically, which is
fine — e.g. `paragraph.rs`'s `approx_pt` doc comment already confirmed harmless).

**Step 2: Delete the alias**

```rust
// draw_primitives.rs — delete these two lines entirely:
// /// Point unit (1/72 inch)
// pub type Pt = f32;
```

**Step 3: Build**

Run: `cargo build -p fulgur`
Expected: succeeds. If it fails, a consumer was missed in Tasks 1-6 — find it via the
compile error, apply the same tag/untag rule, and fix before proceeding (do not
reintroduce the alias).

**Step 4: Check public-reachability (informational only)**

Run: `cargo doc -p fulgur --no-deps 2>&1 | grep -i "draw_primitives::Pt"` (should find
nothing — confirms the public item is gone). This is a legitimate breaking change under
`draw_primitives` being `pub mod`; `cargo-semver-checks` will flag it downstream in CI as
advisory (project is ZeroVer, not a blocker) — no action needed here beyond noting it in
the close reason later.

**Step 5: Commit**

```bash
git add crates/fulgur/src/draw_primitives.rs
git commit -m "refactor(draw_primitives): delete legacy type Pt=f32 alias (P2a)"
```

---

### Task 8: Full verification suite (byte-neutral proof)

**Files:** none (verification only)

**Step 1: Full build + lint**

Run: `cargo build` (whole workspace), `cargo clippy -p fulgur --all-targets -- -D
warnings`, `cargo fmt --check`
Expected: all clean.

**Step 2: Full lib test suite**

Run: `cargo test -p fulgur --lib`
Expected: all pass, count >= baseline (1497 per the last P1e note — should be roughly the
same, this task adds no new tests, only retypes existing ones).

**Step 3: examples_determinism + VRT, byte-identical against baseline**

Run:

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-cli --test examples_determinism
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt
git status --short -- crates/fulgur-vrt/goldens/
```

Expected: `examples_determinism` 11/11 passed, `run_fulgur_vrt` ok, golden git status
empty (byte-identical — the whole point of this refactor). **Never** set
`FULGUR_VRT_UPDATE=1`. If goldens differ, STOP and find the wrong tag/untag site — do not
regenerate goldens to make it pass.

**Step 4: codecov/patch coverage spot-check**

The `.to_f32()`/`.pt()` sites touched in Tasks 1-6 are exactly the pattern that dropped
patch coverage in P1c/P1e (memory `project_units_migration_patch_coverage`). Check whether
any changed line in `transform_rect`, `push_rect`, `clamp_marker_size`, or the
`paragraph.rs` link-rect construction sites is exercised only by VRT (which the coverage
job excludes) and not by an existing `render_smoke.rs` case or in-crate unit test. If so,
add a minimal behavioral case to `crates/fulgur/tests/render_smoke.rs` (pattern:
`Engine::builder().build().render_html(html)` + `assert!(!pdf.is_empty())`) covering a
link inside styled text (exercises the Rect path) and a `list-style-image` marker
(exercises `clamp_marker_size`) if no such case already exists.

Run: `grep -n "link\|list-style-image" crates/fulgur/tests/render_smoke.rs` first to check
what's already covered before adding anything.

**Step 5: Commit (only if Step 4 added a test)**

```bash
git add crates/fulgur/tests/render_smoke.rs
git commit -m "test(render_smoke): cover P2a-touched draw paths for patch coverage"
```

---

### Task 9: Documentation — record the P2a outcome

**Files:**

- Modify: `docs/plans/2026-06-27-engine-layout-api-design.md` (append outcome section,
  matching the P1a/P1c/P1d/P1e precedent already in the file)
- Modify: `.claude/rules/coordinate-system.md` (only if it still describes `Pt` as an f32
  alias anywhere — check first, likely no change needed since it already refers to
  `units::Pt` throughout after the `fulgur-1ino` merge)

**Step 1: Check whether coordinate-system.md needs an update**

Run: `grep -n "Pt\b" .claude/rules/coordinate-system.md`
Expected: no remaining references to the removed alias (it already documents `units::Pt`
post-`fulgur-1ino`). If clean, skip to Step 2.

**Step 2: Append a P2a outcome section** to
`docs/plans/2026-06-27-engine-layout-api-design.md`, after the existing "P1c outcome"
section, following the same style as the others:

```markdown
### P2a outcome (fulgur-2map.8) - `type Pt = f32` alias removed

Phase P2a deleted the legacy `draw_primitives::Pt = f32` alias, byte-neutral
(examples_determinism + VRT goldens unchanged):

- `draw_primitives::Rect` (link-activation rect), `DestinationRegistry` (anchor
  resolution), `BookmarkEntry.y_pt`/`BookmarkCollector` (PDF outline), and
  `clamp_marker_size` (list-style-image marker sizing) retyped to `units::Pt`.
- `InlineBoxRenderCtx`'s margin fields renamed `Pt` -> plain `f32` (no typed neighbor
  exists there - see the epic's P2b sibling issue for the "genuine f32-f32 boundary, no
  transitional gap" reasoning applied consistently).
- `svg::SvgRender::draw` (confirmed dead code, not reachable from the v2 render path)
  retyped for completeness so the alias's last consumer was gone before deletion.
- The "27 hardcoded 0.75 constants" clause from the original P2 scope was already
  resolved as a side effect of P1a-P1e; no action was needed for it.

Split from the original P2 scope: `px_to_pt`/`pt_to_px` internal call-site consolidation
continues as sibling issue P2b (`fulgur-2map.12`), sequenced after this phase.
```

**Step 3: Commit**

```bash
git add docs/plans/2026-06-27-engine-layout-api-design.md
git commit -m "docs(plan): record P2a (Pt alias removal) outcome"
```

---

## Notes for the executor

- Every task after Task 5 depends on the struct/function retyped by earlier tasks
  compiling cleanly — run them in order (1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9).
- Tasks 1-6 are independent of each other in principle (different structs/functions) but
  are sequenced to keep the alias's remaining-consumer count monotonically shrinking, so
  Task 7's "confirm no remaining bare references" grep is a clean go/no-go check.
- If a `cargo build` step in any task surfaces a call site not mentioned in this plan
  (possible — the investigation was thorough but not automated), apply the same two
  fixup rules from the Architecture section rather than guessing; when genuinely
  ambiguous, stop and ask rather than picking `.pt()` vs `.to_f32()` by pattern-matching
  syntax alone (this is the exact mistake the P2b design flags as a silent-bytes-change
  risk).
