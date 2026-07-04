# fulgur-sipv.5 — list font-metrics → Pt

**Goal:** Drop the 7 `px_to_pt` in the list-marker font-metric flow by typing five coupled
signatures to `units::Pt`, byte-neutral. Consumer audit verdict: CLEAN — all fulgur sinks are
`Pt`; the only `.to_f32()` additions are at the **skrifa external boundary** (permanent, correct,
not churn).

## Byte-neutral gate

Baseline GREEN before edits (lib 1597, determinism 11, VRT clean). After each logical step:
build + clippy -D warnings + fmt + lib + determinism + VRT + `git status goldens` empty. Never
`FULGUR_VRT_UPDATE=1`. No arithmetic reassociation.

## The five coupled signatures (all in `convert/list_marker.rs`)

1. `size_raster_marker(…, line_height: Pt)` — drop `.pt()` on `clamp_marker_size(_,_, line_height)`.
2. `resolve_list_marker(node, line_height: Pt, …)` — `line_height <= Pt::ZERO`; SVG-branch
   `clamp_marker_size(_,_, line_height)` drop `.pt()`.
3. `resolve_inside_image_marker(node, first_line_height: Pt, …)` — same shape.
4. `extract_marker_lines(…) -> (Vec<ShapedLine>, Pt, Pt)` — `line_height_pt`/`line_width`/`max_width`
   become `Pt`: `metrics.line_height.px().in_pt()`, `g.advance.px().in_pt()`, `== Pt::ZERO`,
   `max_width.max(line_width)` (Pt::max), early `return (vec![], Pt::ZERO, Pt::ZERO)`.
5. `shape_marker_with_skrifa(…, font_size: Pt, …)` — the **skrifa boundary**:
   `Size::new(font_size.to_f32())`, `x_advance: advance / font_size.to_f32()` (acceptable f32 seam);
   `font_size: font_size.pt()` → `font_size` (drop tag).

## Call sites (compiler-driven — change sigs, then fix what breaks)

- `list_item.rs:82,86,134,140,145` — `px_to_pt(…stylo.px())` → `…stylo.px().px().in_pt()`; the
  `12.0` fallback literals → `12.0_f32.pt()`; `font_size_pt`/`line_height`/`fs` locals become `Pt`
  (Pt·f32 arithmetic stays Pt). Their `.pt()` sinks into `ShapedLine.height`/marker fields drop.
- `list_item.rs:29` (`extract_marker_lines`) → `marker_width.pt()`/`marker_line_height.pt()` at
  `:36`/`:43` drop; `resolve_list_marker(marker_line_height)` passes `Pt`.
- `inline_root.rs:91` (`resolve_inside_image_marker`) — `first_line_height` is
  `paragraph.lines[0].height.to_f32()`; drop the `.to_f32()` → pass `Pt` directly.
- Tests in `list_marker.rs` (`shape_marker_with_skrifa`/`size_raster_marker`) pass `12.0` →
  `12.0_f32.pt()`; assertions on the returned `Pt` tuple use `.to_f32()`.

## Verify + PR

Full cadence, goldens untouched. New PROD lines covered by existing list_style_image / inside-marker
integration + the list_marker unit tests. One PR; close `fulgur-sipv.5` on merge.
