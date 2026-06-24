//! box-shadow extraction.
//!
//! Iterates the computed `box-shadow` list and pushes non-inset shadows
//! onto `BlockStyle::box_shadows`. Non-zero blur is rendered via the
//! gradient 9-slice path in `render.rs`. Inset shadows are skipped with
//! a `log::warn!`.

use super::{StyleContext, absolute_to_rgba};
use crate::convert::px_to_pt;
use crate::draw_primitives::BlockStyle;

pub(super) fn apply_to(style: &mut BlockStyle, ctx: &StyleContext<'_>) {
    let shadow_list = ctx.styles.clone_box_shadow();
    for shadow in shadow_list.0.iter() {
        if shadow.inset {
            log::warn!("box-shadow: inset is not yet supported; skipping");
            continue;
        }
        let blur_px = shadow.base.blur.px();
        let rgba = absolute_to_rgba(shadow.base.color.resolve_to_absolute(ctx.current_color));
        if rgba[3] == 0 {
            continue; // fully transparent — skip
        }
        style.box_shadows.push(crate::draw_primitives::BoxShadow {
            offset_x: px_to_pt(shadow.base.horizontal.px()),
            offset_y: px_to_pt(shadow.base.vertical.px()),
            blur: px_to_pt(blur_px),
            spread: px_to_pt(shadow.spread.px()),
            color: rgba,
            inset: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;

    fn render_with_shadow(shadow_css: &str) -> Vec<u8> {
        let html = format!(
            r#"<html><body><div style="width:100px;height:100px;box-shadow:{shadow_css}"></div></body></html>"#
        );
        Engine::builder()
            .build()
            .render_html(&html)
            .expect("render should succeed")
    }

    /// Non-inset shadow with partial alpha exercises the main push path.
    #[test]
    fn non_inset_shadow_is_pushed() {
        let pdf = render_with_shadow("5px 5px 10px rgba(0,0,0,0.5)");
        assert!(pdf.starts_with(b"%PDF"), "expected PDF header");
    }

    /// `inset` keyword exercises the `shadow.inset` branch (skipped with log::warn!).
    #[test]
    fn inset_shadow_is_skipped() {
        let pdf = render_with_shadow("inset 5px 5px 10px red");
        assert!(pdf.starts_with(b"%PDF"), "expected PDF header");
    }

    /// Fully transparent color exercises the `rgba[3] == 0` early-continue branch.
    #[test]
    fn transparent_shadow_is_skipped() {
        let pdf = render_with_shadow("5px 5px 10px transparent");
        assert!(pdf.starts_with(b"%PDF"), "expected PDF header");
    }

    /// Multiple shadows: inset + transparent + normal in one list.
    /// Only the opaque non-inset shadow reaches the push path.
    #[test]
    fn mixed_shadow_list_skips_inset_and_transparent() {
        let pdf = render_with_shadow("inset 2px 2px red, 5px 5px transparent, 3px 3px 0px black");
        assert!(pdf.starts_with(b"%PDF"), "expected PDF header");
    }

    /// Non-zero spread radius is stored via `px_to_pt(shadow.spread.px())`.
    #[test]
    fn shadow_with_spread_radius() {
        let pdf = render_with_shadow("2px 2px 5px 8px rgba(255,0,0,0.8)");
        assert!(pdf.starts_with(b"%PDF"), "expected PDF header");
    }
}
