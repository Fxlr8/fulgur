# fulgur-sipv.6: Type absolute-positioning CB geometry to `Px` — Implementation Plan

**Goal:** Eliminate the ~11 `px_to_pt`/`pt_to_px` calls in the absolute-positioning
CB / pseudo-image sizing paths (`convert/positioned.rs`, `convert/pseudo.rs`) by
typing the CSS-px geometry to `units::Px` and the pt basis to `units::Pt`.
**Byte-neutral** per epic `fulgur-sipv` protocol.

**Framing correction (Px, not Pt):** the issue title says "to Pt" but the issue's
own CONSUMER AUDIT is correct: `AbsCb.padding_box_size` / `border_top_left` /
`cb_padding_box` live in **CSS px (Taffy)** space, not pt — `sz.width -
pt_to_px(border)` is px − px = px, consumers are named `cb_w_px`/`bl_px`, and the
sink `Fragment.x` is already `units::Px`. Target = **`units::Px`**.

**Round-trip deferral:** the audit's point 3 (px→pt→px round-trip via
`positioned.rs:448` `px_to_pt` → `resolve_pseudo_size:258` `pt_to_px`) is
**preserved, not deleted**, in this PR. Deleting it removes a real `×0.75 ÷0.75`
float op — `(x*0.75)/0.75 != x` in general — which is a behavioural change, not a
type migration, and would violate the byte-neutral protocol. Typing it as
`w_px.in_pt().in_px()` is bit-identical to `pt_to_px(px_to_pt(w_px))`. The
round-trip cleanup is tracked as a separate (non-byte-neutral) follow-up.

---

## Task 0: `Px::max` / `Px::min` (done)

`units.rs`: add `Px::max` / `Px::min` mirroring `Pt` (the `.max(0.0)` clamps at
`cb_padding_box` are the first `Px` clamp need), update the `Px::ZERO` doc, add
`px_max_min_mirror_f32` test (covers the new method bodies — codecov patch).

## Task 1: `AbsCb` + `cb_padding_box` (positioned.rs)

- `AbsCb.padding_box_size: (Px, Px)`, `border_top_left: (Px, Px)`.
  `parent_offset_in_cb_bp` **stays `(f32, f32)`** (Taffy location; tagged at its
  single use site — smaller diff, avoids touching the 3 `AbsCb` construction
  sites + the offset-accumulation loop).
- `cb_padding_box(node) -> ((Px, Px), (Px, Px))`:
  - keep `bl_pt`/`br_pt`/`bt_pt`/`bb_pt` as `Pt` (drop `.to_f32()`)
  - `pb_w = (sz.width.as_px() - (bl_pt + br_pt).in_px()).max(Px::ZERO)`
  - `((pb_w, pb_h), (bl_pt.in_px(), bt_pt.in_px()))`
- `resolve_cb_for_absolute`: `padding_box_size.0 <= 0.0` → `<= Px::ZERO`;
  `padding_box_size.0 = vw` → `vw.as_px()` (viewport params stay f32).

## Task 2: inset-correction math (positioned.rs)

- `resolve_inset_px(inset, basis: Px) -> Option<Px>`: `Length::new(basis.to_f32())`
  (sanctioned Stylo px FFI), result `.px().as_px()`.
- `maybe_apply_abs_pseudo_inset_correction(..., pseudo_w: Pt, pseudo_h: Pt, ...)`:
  - `pseudo_w_px = pseudo_w.in_px()` (was `pt_to_px(pseudo_w_pt)`)
  - `parent_x_px = parent_frag.x` (already `Px`; drop `.to_f32()`)
  - `left/top/right/bottom: Option<Px>`; `x_in_pp_px`/`y_in_pp_px`: `Px`
    (`left.unwrap_or(Px::ZERO)`)
  - `ox_px.as_px()` / `oy_px.as_px()` at the one use site (parent_offset stays f32)
  - `Fragment { x: new_x_px, y: new_y_px, width: pseudo_w_px, height: pseudo_h_px }`
    (all `Px` now; drop the `.as_px()` tags)
  - update the caller to pass the just-built `ImageEntry` w/h as `Pt`.

## Task 3: pseudo-image sizing (positioned.rs + pseudo.rs)

- `try_build_absolute_pseudo_image` (positioned.rs:446-455):
  - `(w_px, h_px) = cb.padding_box_size` (`Px`); basis `(w_px.in_pt(), h_px.in_pt())`
  - else branch: `parent.final_layout.size.width.as_px().in_pt()`
  - `build_pseudo_image_entry(pseudo, basis_w_pt: Pt, basis_h_pt: Pt, ...)`
- `build_pseudo_image_entry(parent_content_width: Pt, parent_content_height: Pt)`:
  - caller `build_block_pseudo_image_entries` (pseudo.rs:153) passes
    `parent_cb.width.as_pt()` (`ContentBox.width` is f32, out of scope)
- `resolve_pseudo_size(size, parent_width: Pt) -> Option<f32>` (return stays
  `Option<f32>` — `make_image_entry` takes `Option<f32>`):
  - `basis_px = parent_width.in_px()`; `Length::new(basis_px.to_f32())`
  - `Some(resolved.px().as_px().in_pt().to_f32())`
- `build_inline_pseudo_image` (pseudo.rs:175-176): **signature stays f32**; tag at
  the call: `resolve_pseudo_size(&styles.clone_width(), parent_content_width.as_pt())`
  (avoids cascading into its callers).

## Task 4: Verification gates

```bash
cargo fmt && cargo fmt --check
cargo build -p fulgur
cargo clippy -p fulgur --all-targets -- -D warnings
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur   # lib + integration
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt
git status --short crates/fulgur-vrt/goldens   # MUST be empty
# patch coverage: llvm-cov nextest --workspace --exclude fulgur-vrt, intersect added ∩ DA:N,0 == 0
```

Every changed PROD site is covered by non-VRT lib tests (`cb_padding_box_*`,
`try_build_absolute_pseudo_image_*`, `smoke_abs_pseudo_content_url_*`,
`resolve_pseudo_size` LengthPercentage tests) — verified pre-edit.

## Task 5: PR + follow-up issue

PR (title EN / body JA), note the Px reframe + round-trip deferral. `bd create`
a follow-up for the round-trip rationalization (non-byte-neutral cleanup).
