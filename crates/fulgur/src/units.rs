//! Typed coordinate units: [`Px`] (CSS pixels) and [`Pt`] (PDF points).
//!
//! fulgur's pipeline mixes two coordinate spaces — CSS px (Blitz/Taffy) and
//! PDF pt (Krilla). Historically these were bare `f32` distinguished only by
//! `_px` / `_pt` field-name suffixes, which made the classic 4/3 scale bug
//! easy. These newtypes put the unit in the type so the compiler rejects
//! `Px + Pt`. Both are `#[repr(transparent)]` over `f32`, so they are
//! zero-cost at runtime; the migration that flips coordinate fields to these
//! types lands in later phases (tracked in the fulgur-2map epic). Note
//! `draw_primitives::Pt` is currently a `type Pt = f32` alias that this
//! migration upgrades to [`Pt`]; until that lands these newtypes are not
//! re-exported at the crate root.
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
//! use fulgur::units::F32Units;
//! let _ = 1.0_f32.px() + 1.0_f32.pt();
//! ```
//!
//! ```compile_fail
//! use fulgur::units::F32Units;
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
        assert_eq!(Pt(6.0) / 2.0, Pt(3.0));
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
        assert_eq!(core::mem::size_of::<Px>(), core::mem::size_of::<f32>());
        assert_eq!(core::mem::size_of::<Pt>(), core::mem::size_of::<f32>());
    }

    #[test]
    fn pt_const_is_single_source() {
        assert_eq!(PX_TO_PT, 0.75);
    }
}
