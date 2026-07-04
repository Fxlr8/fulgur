//! Regression guard for fulgur-2map.9 (P3a): confirms that `Drawables`-reachable
//! `column_css` types stay nameable via `fulgur::column_css::*` from outside the
//! crate. If `column_css` (or one of these types) is ever made private/pub(crate)
//! again, this file fails to compile — that's the point.

use fulgur::column_css::{ColumnFill, ColumnRuleSpec, ColumnRuleStyle, ColumnStyleProps, PageName};

#[test]
fn column_css_public_types_are_externally_nameable() {
    let spec = ColumnRuleSpec::default();
    assert_eq!(spec.style, ColumnRuleStyle::default());

    let props = ColumnStyleProps {
        fill: Some(ColumnFill::default()),
        page: Some(PageName::Auto),
        ..ColumnStyleProps::default()
    };
    assert_eq!(props.fill, Some(ColumnFill::Balance));
}

/// Regression guard for fulgur-2map.10 (P3b): `LayoutOutput`'s field types
/// (`Drawables`, `PaginationGeometryTable`) must stay nameable via external
/// `fulgur::…` paths. `LayoutOutput.geometry` exposes `PaginationGeometryTable`
/// (→ `PaginationGeometry` → `Fragment` → `units::Px`) publicly for the first
/// time; if any of that chain is ever privatized, this fails to compile.
#[test]
fn layout_output_field_types_are_externally_nameable() {
    use fulgur::pagination_layout::PaginationGeometryTable;
    use fulgur::{Engine, LayoutOutput};

    let out: LayoutOutput = Engine::builder()
        .build()
        .layout("<p>x</p>")
        .expect("layout");
    let _geometry: &PaginationGeometryTable = &out.geometry;
    let _drawables: &fulgur::drawables::Drawables = &out.drawables;
    assert!(!out.geometry.is_empty());

    // Exercise the public `Debug` + `Clone` derives: consumers rely on them,
    // and invoking them here attributes the `#[derive(Debug, Clone)]` region
    // on `LayoutOutput` for codecov/patch (it is otherwise never called).
    let cloned = out.clone();
    assert!(!format!("{cloned:?}").is_empty());
}
