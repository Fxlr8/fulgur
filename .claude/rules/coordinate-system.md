# Coordinate System and Unit Conversion Rules

Most coordinate bugs in fulgur trace back to either "forgetting to convert CSS px ↔ PDF pt"
or "misunderstanding Krilla's Y direction."
Always consult this file when writing or reviewing rendering code.

## Unit layers in the pipeline

```text
Blitz/Taffy (CSS px) ──px_to_pt()──► Pageable tree / Krilla (PDF pt)
                     ◄──pt_to_px()──
```

| Layer | Unit | Notes |
|-------|------|-------|
| Blitz input (viewport, width) | CSS px | convert with `pt_to_px(v)` before passing |
| Taffy `final_layout` | CSS px | extract via `layout_in_pt()` / `size_in_pt()` |
| Pageable tree internals | PDF pt | |
| Krilla Surface | PDF pt | |
| `PageSize::custom(w, h)` | **mm** | `config.rs:22` — converted to pt internally |
| `Margin::uniform(v)` | **pt** | |
| `Margin::uniform_mm(v)` | **mm** | |

Conversion constant: `1 CSS px = 0.75 PDF pt` (`PX_TO_PT = 0.75`, `convert.rs:35`)

## Blitz boundary conversion rules (most important)

Forgetting a conversion produces a **4/3× or 3/4× scale bug**.

```rust
// WRONG: pass pt value directly
parse_html_with_local_resources(html, config.content_width(), ...)

// CORRECT: convert pt → CSS px first
parse_html_with_local_resources(html, pt_to_px(config.content_width()), ...)
```

```rust
// WRONG: use Taffy layout values directly
let width = node.final_layout.size.width;

// CORRECT: go through px_to_pt / size_in_pt
let (width, height) = size_in_pt(node.final_layout.size);
let (x, y, width, height) = layout_in_pt(&node.final_layout);
```

Helper definitions: `convert.rs:37-63`

## Exception: `compute_transform` arguments

`blitz_adapter::compute_transform(styles, border_box_width, border_box_height)` is the one
place that runs Stylo length resolution in **pt space, not px**. The parameters are still
bare `f32`, but the `convert/mod.rs` call site feeds them **pt-valued** dims —
`size_in_pt(node.final_layout.size)` (see the "PR 8i note" near `convert/mod.rs:605`).
There is **no later px → pt fold**: the matrix and origin are produced and consumed as pt.

Why feeding pt works: Stylo's `LengthPercentage::resolve` is unit-agnostic — a `Length` is
just a number. Resolving against the pt-valued basis makes `%` translates and
`transform-origin` come out correct against a pt basis. Absolute lengths ignore the basis
and pass through as their numeric value, which the render path then treats as pt.

The returned `Affine2D.e`/`.f` (translate components) and the `Point2` origin are now typed
`units::Pt` (pt space) and are consumed directly by `render::draw_under_transform`.
**Do not add `.in_pt()` to them** — they are already pt.

Known consequence (out of scope for the typing migration): an absolute-length
`translate(Npx)` ends up as `N` **pt**, a latent 4/3 over-shift, because the px value is
reinterpreted as pt with no conversion. That is a behavior bug tracked separately — the
`Pt` typing reflects today's reality, it does not change or fix it.

## Stylo length-percentage resolution

For **layout-space** resolves (positioned insets, pseudo offsets, border-radius,
viewport-relative lengths), `LengthPercentage::resolve(basis: Length)` — `basis` must be in
**CSS px**. Passing a pt value produces a 3/4× error.

```rust
// WRONG: pt basis
inset.resolve(Length::new(cb_width_pt)).px()

// CORRECT: px basis (layout values are CSS px)
inset.resolve(Length::new(cb_width_px)).px()
```

The `transform` path is the deliberate exception to this rule: it resolves against a **pt**
basis and uses the result directly as pt — see *Exception: `compute_transform` arguments*
above.

## Krilla / Pageable coordinate system

- **Origin: top-left, Y axis: downward (Y-down)**
- PDF spec (ISO 32000) uses bottom-left origin with Y up, but Krilla flips internally
- All fulgur code assumes top-left origin with Y growing down

`Quadrilateral` vertex order (`pageable.rs:297`):
bottom-left → bottom-right → top-right → top-left (in Y-down coordinates)

## CSS transform matrix composition (`Affine2D`)

```rust
// A.mul(&B) returns the matrix product A × B (point p transforms as A * B * p)
// CSS transform lists apply right-to-left (first operation is innermost)
let composed = t_origin.mul(&m).mul(&t_neg_origin);
// = T(ox, oy) · M · T(-ox, -oy)
```

`Affine2D`'s `(a, b, c, d, e, f)` maps to `krilla::geom::Transform::from_row(a, b, c, d, e, f)`.

## PDF text coordinates in inspect.rs

`Td`/`TD` operands are offsets in text coordinate space, not page space.
Apply the linear part of the text matrix `(a, b, c, d)` to convert to user-space displacement.

```rust
// WRONG: add offset directly to page coordinates
tx += dx; ty += dy;

// CORRECT: transform through text matrix linear part
tlm_e += dx * tm_a + dy * tm_c;
tlm_f += dx * tm_b + dy * tm_d;
```

`BT` resets both the text matrix and text line matrix to identity (PDF §9.4.1).
Track the CTM stack (`q`/`Q`) to obtain final page coordinates.

## References

- `crates/fulgur/src/convert.rs:29-63` — conversion constants and helper definitions
- `crates/fulgur/src/config.rs:22` — `PageSize::custom` mm definition
- `docs/plans/2026-04-17-viewport-pt-to-css-px.md` — deep-dive on the px/pt boundary bug
- PR #90 (superseded) / beads fulgur-9ul — history of the viewport pt/px misidentification fix
