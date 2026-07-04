# fulgur-sipv.4: Type gradient coords to `Pt` — Implementation Plan

**Goal:** Migrate `draw_primitives::BgLengthPercentage::Length(f32)` to
`Length(units::Pt)` and collapse the surrounding `px_to_pt` / `pt_to_px`
call sites to typed `.in_pt()` / `.in_px()`, eliminating 5 raw-`f32`
conversion sites in the gradient subsystem. **Byte-neutral** per epic
`fulgur-sipv` protocol.

**Architecture:** `BgLengthPercentage::Length` holds an absolute length that
is always PDF-pt-valued (producers convert CSS-px → pt at construction). Making
the payload a `Pt` lets the three producer sites collapse to
`…px().as_px().in_pt()` and lets the two draw-side `pt_to_px` conversions become
`…as_pt().in_px()`. `resolve_gradient_stops`'s `line_length` param is retyped to
`Px` (its natural unit — it divides against CSS-px `LengthPx` stop positions) so
the call sites stay clean with no reintroduced `.to_f32()`. The four `resolve_*`
consumers keep their f32 (pt-space) return type — this subtask does **not** type
the draw geometry — so their `Length(v)` arms become `v.to_f32()`.

**Tech Stack:** Rust, `crate::units::{Px, Pt, F32Units}` newtypes, Krilla,
Stylo. Verification: `cargo test -p fulgur --lib`, `cargo test -p fulgur-vrt`
(byte compare), `cargo clippy -D warnings`, `cargo fmt`.

---

## Byte-neutrality invariant (why this is safe)

`convert::px_to_pt(v) = v * PX_TO_PT` is bit-identical to `Px::in_pt = Pt(self.0 * PX_TO_PT)`;
`convert::pt_to_px(v) = v / PX_TO_PT` is bit-identical to `Pt::in_px = Px(self.0 / PX_TO_PT)`
(both reference `crate::units::PX_TO_PT`, same operand order). `*v` (f32) and
`v.to_f32()` (Pt→f32) are the same bits. `px / line_length` (f32/f32) and
`px.as_px() / line_length` (`Px / Px` → `self.0 / rhs.0`) are the same division.
**No arithmetic operand order changes anywhere.** Final proof is the VRT byte
compare; never run `FULGUR_VRT_UPDATE=1`.

## Patch-coverage note

Every changed PROD line is exercised by a **non-VRT lib test** (codecov patch
runs `--exclude fulgur-vrt`):

| Changed line | Covering lib test |
|---|---|
| `convert/style/background.rs:347` (circle radius) | `radial_gradient_circle_explicit_radius` |
| `:534` (`convert_lp_to_bg` length) | `background-size:50px 40px` case in convert tests |
| `:548` (`try_convert_lp_to_bg` length) | `radial_gradient_ellipse_explicit_radii` |
| `background.rs:1012` (`LengthPx` arm) | `resolve_gradient_stops_length_px_converted_to_fraction` |
| `:1218` / `:1344` (draw locals) | gradient render lib tests (linear/radial) |
| `:1695/1703/1800/1808` (`resolve_*` arms) | `resolve_point/length/lp/position` unit tests |

No new PROD test is required. Test-side edits must NOT reintroduce cold-path
`.to_f32()`: compare extracted `Length(v)` as `v == X.as_pt()` (Pt `PartialEq`),
not `v.to_f32() == X`.

---

## Task 1: Retype the enum payload

**Files:** Modify `crates/fulgur/src/draw_primitives.rs:716`

Change `Length(f32)` → `Length(crate::units::Pt)` (use the path already imported
in the module, or `units::Pt`). Update the doc comment ("Absolute length in
points" stays accurate).

**Verify:** `cargo build -p fulgur` — expect compile errors at every producer
and every `Length(v) => *v` consumer (the compiler is the inventory).

## Task 2: Collapse the three producers (`convert/style/background.rs`)

- `:347` `let len_pt = px_to_pt(r.0.px());` → `let len_pt = r.0.px().as_px().in_pt();`
- `:534` `Length(lp.to_length().map(|l| px_to_pt(l.px())).unwrap_or(0.0))`
  → `Length(lp.to_length().map(|l| l.px().as_px().in_pt()).unwrap_or(crate::units::Pt::ZERO))`
- `:548` `.map(|l| BgLengthPercentage::Length(px_to_pt(l.px())))`
  → `.map(|l| BgLengthPercentage::Length(l.px().as_px().in_pt()))`

Add `use crate::units::F32Units;` (for `.as_px`) if not already in scope; drop
the now-unused `px_to_pt` import if it becomes dead.

## Task 3: Retype the four `resolve_*` consumer arms (`background.rs`)

`resolve_point:1695`, `resolve_length:1703`, `resolve_lp:1800`,
`resolve_position:1808`: `BgLengthPercentage::Length(v) => *v` →
`BgLengthPercentage::Length(v) => v.to_f32()`. Return types stay `f32`.

## Task 4: Collapse the two draw-side conversions + retype `resolve_gradient_stops`

- `resolve_gradient_stops:991` param `line_length: f32` → `line_length: Px`.
  - body `:999` `if line_length <= 0.0` → `if line_length <= Px::ZERO`
  - body `:1012` `GradientStopPosition::LengthPx(px) => Some(px / line_length)`
    → `Some(px.as_px() / line_length)`
- `draw_linear_gradient:1218` `let length_px = crate::convert::pt_to_px(length);`
  → `let length_px = length.as_pt().in_px();`
- `draw_radial_gradient:1344` `let rx_px = crate::convert::pt_to_px(rx);`
  → `let rx_px = rx.as_pt().in_px();`

Add `use crate::units::{F32Units, Px};` to `background.rs` as needed. (`length`
and `rx` stay `f32` — fully typing them cascades into `cx_box - sin*half`
f32 geometry, out of this subtask's scope; tagging at the boundary satisfies
"consistently" and stays byte-neutral.)

## Task 5: Fix test constructors / call sites

- `BgLengthPercentage::Length(N.0)` literals (background.rs ~14 test sites:
  2216, 2317, 2336, 2337, 2351, 2364, 2413, 2438, 3440, 3456, 4924, 4925,
  4932, 4940) → `Length(N.0_f32.as_pt())`.
- `resolve_gradient_stops(&stops, N.0, …)` literals (~22 test sites in the two
  test mods) → second arg `N.0_f32.as_px()`.

## Task 6: Verification gates (all must pass)

```bash
cargo fmt                       # normalize (fmt may re-single-line shrunken asserts)
cargo fmt --check
cargo build -p fulgur
cargo clippy -p fulgur --all-targets -- -D warnings
cargo test -p fulgur --lib
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt
git status --short crates/fulgur-vrt/goldens   # MUST be empty (byte-identical)
```

## Task 7: Commit

```bash
git add -A
git commit -m "refactor(fulgur): type gradient BgLengthPercentage coords to units::Pt"
```
