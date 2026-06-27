# Px/Pt newtype units (fulgur-2map.1 / P0) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Introduce `Px` and `Pt` newtype coordinate units in fulgur core, with
same-unit arithmetic, px↔pt conversion, constructor sugar, and an FFI escape —
proving unit mix-ups (`Px + Pt`) are a compile error. No field/call-site
migration in this task.

**Architecture:** A new `pub mod units` holds two `#[repr(transparent)]`
newtypes over `f32` (zero runtime cost). The field is **private** so all
construction goes through `.px()` / `.pt()` and all raw-`f32` escapes go
through `.to_f32()` — every boundary crossing is visible. `PX_TO_PT = 0.75`
lives here as the crate's single conversion constant. The newtypes are **not**
re-exported at the crate root in P0 — the existing `draw_primitives::Pt = f32`
alias would make a root `Pt` ambiguous, so the root re-export and alias removal
wait for P1a (see Step 4). Compile-fail doctests assert cross-unit arithmetic
does not compile
(chosen over `trybuild` to avoid a new dev-dependency and brittle `.stderr`
snapshots).

**Tech Stack:** Rust, `core::ops` operator traits, declarative macro for
per-type operator DRY, `compile_fail` doctests.

**Determinism:** byte-neutral. Nothing is migrated to the new types yet, so
`examples_determinism` + VRT goldens are unaffected (the types are unused by
the render path).

---

### Task 1: `units` module — types, operators, conversions (unit-tested)

**Files:**

- Create: `crates/fulgur/src/units.rs`
- Modify: `crates/fulgur/src/lib.rs` (add `pub mod units;` only; no crate-root
  re-export in P0 — see Step 4)

**Step 1: Write the failing unit tests**

Create `crates/fulgur/src/units.rs` with ONLY the test module first (it will
fail to compile because the types don't exist yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_to_pt_and_back() {
        assert_eq!(Px(4.0).in_pt(), Pt(3.0));
        assert_eq!(Pt(3.0).in_px(), Px(4.0));
    }

    #[test]
    fn same_unit_arithmetic() {
        assert_eq!(Pt(1.0) + Pt(2.0), Pt(3.0));
        assert_eq!(Px(5.0) - Px(2.0), Px(3.0));
        assert_eq!(Pt(2.0) * 3.0, Pt(6.0));
        assert_eq!(-Px(1.5), Px(-1.5));
        let mut a = Pt(1.0);
        a += Pt(2.0);
        a -= Pt(0.5);
        assert_eq!(a, Pt(2.5));
        let s: Pt = [Pt(1.0), Pt(2.0), Pt(3.0)].into_iter().sum();
        assert_eq!(s, Pt(6.0));
    }

    #[test]
    fn ctor_sugar_and_escape() {
        assert_eq!(4.0_f32.px(), Px(4.0));
        assert_eq!(3.0_f32.pt(), Pt(3.0));
        assert_eq!(Pt(3.0).to_f32(), 3.0);
        assert_eq!(Px(4.0).to_f32(), 4.0);
    }

    #[test]
    fn repr_transparent_is_zero_cost() {
        assert_eq!(
            core::mem::size_of::<Px>(),
            core::mem::size_of::<f32>()
        );
        assert_eq!(core::mem::size_of::<Pt>(), core::mem::size_of::<f32>());
    }

    #[test]
    fn pt_const_is_single_source() {
        assert_eq!(PX_TO_PT, 0.75);
    }
}
```

**Step 2: Run to verify it fails**

Run: `cargo test -p fulgur --lib units 2>&1 | tail -20`
Expected: compile error (cannot find type `Px` / `Pt`, etc.).

**Step 3: Write the implementation** (prepend above the test module)

```rust
//! Typed coordinate units: [`Px`] (CSS pixels) and [`Pt`] (PDF points).
//!
//! fulgur's pipeline mixes two coordinate spaces — CSS px (Blitz/Taffy) and
//! PDF pt (Krilla). Historically these were bare `f32` distinguished only by
//! `_px` / `_pt` field-name suffixes, which made the classic 4/3 scale bug
//! easy. These newtypes put the unit in the type so the compiler rejects
//! `Px + Pt`. Both are `#[repr(transparent)]` over `f32`, so they are
//! zero-cost at runtime; the migration that flips coordinate fields to these
//! types lands in later phases (tracked in the fulgur-2map epic).
//!
//! - Construct with [`F32Units::px`] / [`F32Units::pt`] (e.g. `frag.x.px()`).
//! - Convert with [`Px::in_pt`] / [`Pt::in_px`] — the only place [`PX_TO_PT`]
//!   is used.
//! - Arithmetic is same-unit only; mixing units is a compile error.
//! - Drop to raw `f32` with [`Px::to_f32`] / [`Pt::to_f32`] **only** at FFI
//!   boundaries (resvg / tiny-skia / krilla).
//!
//! Cross-unit arithmetic does not compile:
//!
//! ```compile_fail
//! use fulgur::units::{F32Units};
//! let _ = 1.0_f32.px() + 1.0_f32.pt();
//! ```
//!
//! ```compile_fail
//! use fulgur::units::{F32Units};
//! let _ = 1.0_f32.pt() + 1.0_f32.px();
//! ```

/// 1 CSS px = 0.75 PDF pt. The single source of this constant in the crate.
pub const PX_TO_PT: f32 = 0.75;

/// A length in CSS pixels (Blitz / Taffy space).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Default)]
pub struct Px(f32);

/// A length in PDF points (Krilla space). 1 px = [`PX_TO_PT`] pt.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Default)]
pub struct Pt(f32);

/// Same-unit arithmetic for a coordinate newtype. Cross-unit ops are
/// intentionally NOT implemented so the compiler rejects unit mix-ups.
macro_rules! impl_unit_arith {
    ($t:ident) => {
        impl core::ops::Add for $t {
            type Output = $t;
            #[inline]
            fn add(self, rhs: $t) -> $t {
                $t(self.0 + rhs.0)
            }
        }
        impl core::ops::Sub for $t {
            type Output = $t;
            #[inline]
            fn sub(self, rhs: $t) -> $t {
                $t(self.0 - rhs.0)
            }
        }
        impl core::ops::Mul<f32> for $t {
            type Output = $t;
            #[inline]
            fn mul(self, rhs: f32) -> $t {
                $t(self.0 * rhs)
            }
        }
        impl core::ops::Div<f32> for $t {
            type Output = $t;
            #[inline]
            fn div(self, rhs: f32) -> $t {
                $t(self.0 / rhs)
            }
        }
        impl core::ops::Neg for $t {
            type Output = $t;
            #[inline]
            fn neg(self) -> $t {
                $t(-self.0)
            }
        }
        impl core::ops::AddAssign for $t {
            #[inline]
            fn add_assign(&mut self, rhs: $t) {
                self.0 += rhs.0;
            }
        }
        impl core::ops::SubAssign for $t {
            #[inline]
            fn sub_assign(&mut self, rhs: $t) {
                self.0 -= rhs.0;
            }
        }
        impl core::iter::Sum for $t {
            #[inline]
            fn sum<I: Iterator<Item = $t>>(iter: I) -> $t {
                $t(iter.map(|v| v.0).sum())
            }
        }
    };
}

impl_unit_arith!(Px);
impl_unit_arith!(Pt);

impl Px {
    /// Raw `f32` value. Use only at FFI boundaries.
    #[inline]
    pub const fn to_f32(self) -> f32 {
        self.0
    }

    /// Convert CSS px → PDF pt.
    #[inline]
    pub fn in_pt(self) -> Pt {
        Pt(self.0 * PX_TO_PT)
    }
}

impl Pt {
    /// Raw `f32` value. Use only at FFI boundaries.
    #[inline]
    pub const fn to_f32(self) -> f32 {
        self.0
    }

    /// Convert PDF pt → CSS px.
    #[inline]
    pub fn in_px(self) -> Px {
        Px(self.0 / PX_TO_PT)
    }
}

/// Constructor sugar so `value.px()` / `value.pt()` reads naturally and the
/// newtype field can stay private.
pub trait F32Units {
    /// Tag this `f32` as CSS pixels.
    fn px(self) -> Px;
    /// Tag this `f32` as PDF points.
    fn pt(self) -> Pt;
}

impl F32Units for f32 {
    #[inline]
    fn px(self) -> Px {
        Px(self)
    }
    #[inline]
    fn pt(self) -> Pt {
        Pt(self)
    }
}
```

**Step 4: Wire into the crate**

In `crates/fulgur/src/lib.rs`, add ONLY the module declaration alongside the
other `pub mod` lines (after `pub mod template;`):

```rust
pub mod units;
```

**Do NOT re-export at the crate root in P0.** `draw_primitives.rs:79` already
has `pub type Pt = f32;` (the alias this migration upgrades to a newtype).
Re-exporting `units::Pt` at the crate root while that alias still means `f32`
would create two `Pt` meanings and invite mis-imports. The root re-export and
removal of the `draw_primitives::Pt` alias happen in P1a (the first migration
step). In P0 the newtypes are reachable as `crate::units::{Px, Pt}` and that is
sufficient for the unit tests + compile-fail doctests.

**Step 5: Run unit tests to verify they pass**

Run: `cargo test -p fulgur --lib units 2>&1 | tail -20`
Expected: the 5 `units::tests::*` tests PASS.

**Step 6: Commit**

```bash
git add crates/fulgur/src/units.rs crates/fulgur/src/lib.rs
git commit -m "feat(units): add Px/Pt newtype coordinate units (no migration)"
```

---

### Task 2: Compile-fail doctests + lint/format gate

**Files:**

- (doctests already authored in the `units.rs` module doc in Task 1)

**Step 1: Run the doctests (incl. compile_fail)**

Run: `cargo test -p fulgur --doc units 2>&1 | tail -20`
Expected: the two `compile_fail` doctests PASS (i.e. the snippets correctly
fail to compile), and there are no `should-have-failed` failures.

**Step 2: Clippy**

Run: `cargo clippy -p fulgur -- -D warnings 2>&1 | tail -20`
Expected: no warnings. (Watch for `clippy::should_implement_trait` on the
`px`/`pt` methods — they are on a custom trait, not inherent, so this should
not fire; if it does, the trait-method form already addresses it.)

**Step 3: Format check**

Run: `cargo fmt -p fulgur --check 2>&1 | tail -5`
Expected: clean (no diff).

**Step 4: Determinism sanity (byte-neutral)**

Nothing is migrated, so the render path cannot have changed. Confirm the
library still builds and a representative smoke path is unaffected:

Run: `cargo test -p fulgur --test render_smoke 2>&1 | tail -10`
Expected: PASS (unchanged).

**Step 5: Commit (if Step 2/3 required any fixups)**

```bash
git add -A
git commit -m "test(units): assert cross-unit arithmetic is a compile error"
```

(If Task 1's commit already covered everything and Steps 2–4 needed no
changes, skip this commit.)

---

## Acceptance (mirrors fulgur-2map.1)

- `Px` / `Pt` exist in `crate::units` with same-unit `Add` / `Sub` /
  `Mul<f32>` / `Div<f32>` / `Neg` (+ `AddAssign` / `SubAssign` / `Sum`),
  `in_pt` / `in_px` conversions, `F32Units` `px()` / `pt()` sugar, and
  `to_f32()` escape; both `#[repr(transparent)]`.
- `PX_TO_PT = 0.75` defined exactly once.
- Compile-fail doctests prove `Px + Pt` and `Pt + Px` do not compile.
- `cargo build`, `cargo clippy -p fulgur -- -D warnings`,
  `cargo fmt --check` all clean.
- byte-neutral: nothing migrated; `examples_determinism` + VRT unaffected.

## Notes / deviations from the issue design field

- **`compile_fail` doctests instead of `trybuild`**: same guarantee (the
  mix-up does not compile) with no new dev-dependency and no rustc-version-
  brittle `.stderr` snapshots. Aligns with the crate's pure-Rust / minimal-dep
  posture.
- **Private newtype field**: forces construction via `.px()` / `.pt()` and
  escapes via `.to_f32()`, keeping every boundary crossing visible.
- **`Div<f32>` added** beyond the design's minimal operator list — layout math
  needs scaling-down (e.g. centering); adding operators later is non-breaking,
  but including the obvious one now avoids churn in P1.
