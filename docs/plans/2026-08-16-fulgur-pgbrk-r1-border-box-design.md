# fulgur-pgbrk R1: inline-root fragments must describe the border box

**Goal:** Make an inline root's recorded fragments cover its whole border box —
leading border/padding included — so the push-whole decision stops
under-measuring, split paragraphs stop painting a page's worth of lines into
the bottom margin, continuation pages stop re-applying `padding-top`, and the
R3 overflow guard can see all of it.

**Parent review:** [2026-08-16-fulgur-pgbrk-page-fragmentation-review.md](./2026-08-16-fulgur-pgbrk-page-fragmentation-review.md)
(item R1).

**Predecessor:** [2026-08-16-fulgur-pgbrk-r3-overflow-detection-design.md](./2026-08-16-fulgur-pgbrk-r3-overflow-detection-design.md).

**Spec baseline:** [CSS Fragmentation Module Level 3](https://www.w3.org/TR/css-break-3/),
§5.4 (`box-decoration-break`).

**Tech stack:** Rust, `crates/fulgur/src/pagination_layout.rs`,
`crates/fulgur/src/render.rs`.

---

## Measured behaviour on the current tree

All four observations below are from the debug CLI at commit `28926fe4`, page
`600px × 500px`, `margin: 50px` — content strip `37.5 … 337.5pt`, paper edge
`375pt`.

**1. R1 reproduces.** A `<p style="padding:150px 0;line-height:20px">` holding
120 probe words, nested two `<div>`s deep after a `100px` spacer, renders **110
of 120 words** with exit code `0` on a single page. Lines run down to
`yMax=372.375pt`: three of them paint into the bottom margin, and the tail
clears the paper edge and is discarded.

**2. `padding-top` on an inline root is honoured in paint.** Padded vs unpadded
first line: `226.875pt` vs `114.375pt`, a delta of exactly `150px × 0.75`. The
CLAUDE.md gotcha about inline-root `padding-top` being ignored does not hold for
this shape.

**3. The recorded fragment is anchored and measured from different edges.**
`fragment_inline_root` writes `y = cursor` — the border-box top — but
`height = last_bottom_local - frag_top_local`. Parley's line coordinates are
already border-box relative (`line_metrics[0].0` is the leading
`border-top + padding-top`), so subtracting `frag_top_local` discards exactly
that lead-in. `padding-bottom` was never in the line metrics to begin with. The
review's measurement — a `460px` box recorded as `y=100, height=160` — is this
subtraction plus the missing trailing edge.

**4. Continuation pages re-apply `padding-top`.** A padded paragraph forced to
split by lines paints its first line at `yMin=84.375pt` on page 1 **and on page
2**. That is `box-decoration-break: clone` where CSS defaults to `slice` (§5.4).
The same render puts page 1's last line at `yMax=364.875pt` — `27.375pt` past
the strip — so the split path overshoots even when it works.

Symptoms 1, 2 and 4 are one root cause: the fragment height is short by
`lead_in + lead_out`. Symptom 4 in particular is not an independent paint-side
bug. `paragraph_lines_for_page` derives `consumed` from prior fragment heights,
so a first fragment short by `lead_in` displaces every later page's baselines by
that same amount.

### Why the R3 guard is blind to this

`find_overflowing_fragments` tests `f.y + f.height > page_height_px + 0.5`
against the fragmenter's own numbers. Those numbers under-report the box by
`lead_in + lead_out`, so the guard reads a fragment sitting comfortably inside
the strip while Parley paints lines off the paper. The one defect that
under-measures the quantity the guard measures is the one defect the guard
cannot report. Fixing R1 restores its sight as a consequence, with no change to
the guard itself.

---

## The model

The fragmenter's numbers become border-box. Per fragment, following
`box-decoration-break: slice` (§5.4, the CSS initial value):

| fragment | height |
| --- | --- |
| only (no split) | `lead_in + lines + lead_out` |
| first of N | `lead_in + lines` |
| middle of N | `lines` |
| last of N | `lines + lead_out` |

The reverse reconciliation — moving the painter into line space — was rejected:
backgrounds, borders and shadows genuinely need the border box, so it relocates
the discrepancy rather than removing it.

`lead_in` is read from `line_metrics[0].0`, which already carries it.
`lead_out` has to come from Taffy (`final_layout.padding.bottom +
final_layout.border.bottom`), because Parley's metrics stop at the last line
box.

> **Verify at implementation time:** that `line_metrics[0].0` equals
> `border-top + padding-top` for a padded inline root. The paint measurements
> above imply it, but they do not isolate it from the first line's ascent. If it
> turns out not to hold, read both edges from Taffy instead — the rest of the
> design is unchanged.

### Where the two values live

On `PaginationGeometry`, not on `Fragment`:

```rust
pub struct PaginationGeometry {
    pub fragments: Vec<Fragment>,
    pub is_repeat: bool,
    /// border-top + padding-top, carried by the FIRST fragment only.
    pub content_lead_in: crate::units::Px,
    /// padding-bottom + border-bottom, carried by the LAST fragment only.
    pub content_lead_out: crate::units::Px,
}
```

They describe the box, not a slice — "the first fragment carries `lead_in`" is
already implied by position in the vector, so per-fragment storage would
duplicate a fact the vector encodes. The practical weight agrees: `Fragment {`
is constructed at 45 sites across five files, `PaginationGeometry {` at 6, and
every other producer goes through `entry().or_default()`. Both fields are
`Px::ZERO` for every non-inline-root node, which is every existing caller.

---

## Changes

### `pagination_layout.rs`

A helper beside `collect_inline_line_metrics`:

```rust
/// Border-box metrics for an inline root: the decoration above the first
/// line box, the line-box extent, and the decoration below the last.
fn inline_root_box_metrics(
    node: &blitz_dom::Node,
    line_metrics: &[(f32, f32)],
) -> (f32, f32, f32); // (lead_in, lines_h, lead_out)
```

It replaces the two hand-duplicated blocks at `:889` (body-direct) and `:2209`
(nested). Those two have to agree and currently agree only by inspection —
Risk 1 in the parent review — so unifying them here costs nothing extra.

Both sites then compute `box_total_h = lead_in + lines_h + lead_out` and use it
for:

- `avoid_is_fulfillable` — "can this box ever fit a fragmentainer?"
- the push-whole test (`cursor_y + box_total_h > page_height_px`)

`fragment_inline_root` takes `lead_in` and `lead_out`, and changes in three
places: the split test adds `lead_in` while `fragment_start_idx == 0`; emitted
heights follow the table above; the returned `cursor_y` advances past
`lead_out`. It records both values on the geometry entry.

### `render.rs`

`paragraph_lines_for_page` (`:3425`) partitions lines by summing fragment
heights and comparing against `ShapedLine.height`, which is pure line-box
space. Once fragment heights carry decoration, that walk needs the decoration
subtracted back out before it partitions: `lead_in` off fragment 0, `lead_out`
off the last. Its parameter changes from `fragments: &[Fragment]` to
`&PaginationGeometry` rather than threading two more `f32`s through its six
call sites.

No other render change is needed. Symptom 4 resolves through `consumed`, and
background/border painting already reads `frag.height` only when `is_split()`
(`:1821`, `:2249`), falling back to `layout_size.height` otherwise — so
single-fragment paragraphs are unaffected by the height change.

---

## Testing

Per CLAUDE.md's coverage rule, the fragmenter logic is lib-level and belongs in
`#[cfg(test)] mod tests` in `pagination_layout.rs`.

- `inline_root_box_metrics`: lead-in from line metrics, lead-out from Taffy,
  zero-padding case, single-line case, empty-metrics case.
- `fragment_inline_root`: each row of the height table — unsplit, first,
  middle, last — plus `cursor_y` advancing past `lead_out`.
- The push-whole decision on a padded paragraph that fits only once its
  decoration is counted.
- `paragraph_lines_for_page`: a three-page padded paragraph partitions to the
  same line sets as its unpadded twin.
- The R1 repro as a lib-level assertion via `find_overflowing_fragments` — no
  `pdftotext` needed, unlike the existing `render_smoke` pagination test.
- A padded variant in `render_smoke.rs` alongside
  `leading_child_that_must_break_does_not_lose_content`.

VRT is expected to move for any fixture containing a padded or bordered
paragraph that splits across pages. Verify with the stash / re-run / diff
protocol from the R3 design (compare the failing fixture list *including byte
sizes*); do not regenerate goldens on macOS, where 29 of 64 differ for
unrelated environment reasons.

---

## Out of scope

**`fulgur-cli` installs no logger.** `crates/fulgur-cli/Cargo.toml` depends on
`fulgur`, `clap`, `serde_json` and `which` — there is no `log` implementation,
so every `log::warn!` in the library is dropped. That includes R3's overflow
warning and the pre-existing warnings in `asset.rs`, `blitz_adapter.rs` and
`column_css.rs`. The R3 design assumed otherwise ("`fulgur-cli` already installs
one"), which makes the production half of R3 inert for CLI users — the exact
audience that filed the original bug report. Tracked as a separate commit on
this branch.

**R2 / R6** (widow relaxation, author `orphans` / `widows` values) follow R1 and
share `fragment_inline_root`'s signature. Landing R1 first means that signature
is edited once for decoration and once for constraints, rather than three times.

---

## Verification commands

```bash
cargo test -p fulgur --lib
cargo test -p fulgur --lib fragment_inline_root
cargo test -p fulgur --lib -- --ignored        # open gaps: expected to FAIL
cargo test -p fulgur --test render_smoke
cargo clippy -p fulgur && cargo fmt --check
npx markdownlint-cli2 '**/*.md'
```
