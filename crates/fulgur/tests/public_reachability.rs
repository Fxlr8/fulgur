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
