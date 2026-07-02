# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.22.0](https://github.com/fulgur-rs/fulgur/compare/v0.21.0...v0.22.0) - 2026-07-02

### Fixed

- *(gcpm)* bound target-text first-letter allocation to matched prefix

## [0.21.0](https://github.com/fulgur-rs/fulgur/compare/v0.20.0...v0.21.0) - 2026-07-02

### Added

- *(units)* add Pt/Px abs() for P1b paragraph migration

### Fixed

- *(column_css)* downgrade extract_column_style_table + ConvertContext.column_styles to pub(crate)
- *(column_css)* make module pub, keep parser internals pub(crate)

### Other

- Merge pull request #532 from fulgur-rs/fulgur-2map.9-public-reachability-gaps
- *(drawables)* sharpen usvg version-identity wording (code review)
- *(drawables)* document usvg version-pin requirement on SvgEntry.tree
- *(column_css)* drop intra-doc link to private ColumnStyleProps::merge
- *(plan)* record P2a (Pt alias removal) outcome; fix stale Pt references in units.rs and CLAUDE.md
- *(draw_primitives)* delete legacy type Pt=f32 alias (P2a)
- *(svg)* type dead SvgRender::draw with units::Pt (byte-neutral, P2a)
- *(paragraph)* rename InlineBoxRenderCtx margin fields Pt alias to f32 (byte-neutral, P2a)
- *(draw_primitives)* type clamp_marker_size with units::Pt (byte-neutral, P2a)
- *(draw_primitives)* type BookmarkEntry.y_pt with units::Pt (byte-neutral, P2a)
- *(draw_primitives)* type DestinationRegistry with units::Pt (byte-neutral, P2a)
- *(draw_primitives)* type Rect with units::Pt (byte-neutral, P2a)
- *(transform)* cover translateX/translateY op_to_matrix arms (codecov patch)
- *(transform)* type Affine2D translate components (e,f) with units::Pt (byte-neutral)
- *(transform)* type Point2 (transform-origin) with units::Pt (byte-neutral)
- Merge remote-tracking branch 'origin/main' into fulgur-2map.7-drawables-aggregate-px-pt
- *(drawables)* type body_offset_pt with units::Pt (byte-neutral, P1e)
- *(multicol)* type ParagraphSlice origin/size with units::Pt (byte-neutral, P1e)
- *(list)* type ListItemMarker/marker_line_height with units::Pt (byte-neutral, P1e)
- *(replaced)* correct device-px provenance comment for auto-sized pseudo url() (P1e Task 2 review, refs fulgur-t82j)
- *(replaced)* type ImageEntry/SvgEntry width/height with units::Pt (byte-neutral, P1e)
- *(table)* type TableEntry width/cached_height with units::Pt (byte-neutral, P1e)
- apply /simplify — drop unused Px::abs, tighten link-rect test to exact quad, Pt::ZERO idiom in render_smoke
- *(paragraph)* close P1b patch coverage — debug-format assert args, inline-image link-rect test
- *(paragraph)* address review — drop _pt suffix on Pt-typed locals, tidy eps/tag (byte-neutral)
- *(paragraph)* migrate ShapedLine/ShapedGlyphRun/InlineImage/InlineBoxItem coords to units::Pt (byte-neutral, P1b)
- Merge pull request #515 from fulgur-rs/fulgur-2map.6-multicol-geometry-px-pt
