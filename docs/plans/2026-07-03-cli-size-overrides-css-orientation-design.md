# CLI `--size` overrides CSS `@page` orientation (fulgur-u4k5)

## Problem

Passing `--size` sets `config.overrides.page_size = true`, which makes the CLI
dimensions win over CSS `@page { size }`. However, orientation leaks through:
an orientation-only CSS rule such as `@page { size: landscape }` is still
applied via `resolve_page_settings` / `resolve_landscape_from_css`, even when
`--size` is given.

Concretely, `fulgur render --size 200ptx400pt` against landscape CSS produces a
`400x200` (rotated) MediaBox. The `--size` help text promises priority over CSS
`@page { size }`, but that promise does not hold on the orientation axis, and
there is no CLI flag to cancel a CSS orientation.

## Key constraint: keyword/custom distinction is already gone at the config layer

`main.rs` maps both `--size A4` and `--size 200ptx400pt` through
`parse_page_size` into the same bare `PageSize { width, height }`. By the time
`resolve_page_settings` runs, `config.page_size` carries no record of whether it
originated from a keyword or explicit dimensions. The library/builder layer has
no concept of "keyword" either (`PageSize::A4` is just a dimension constant).

This rules out a purely local fix that treats keyword and custom sizes
differently (the issue's option (c)): that would require threading a new
size-kind intent from the CLI into `config`. Any orientation policy that the
config layer can express must treat keyword and custom sizes identically.

## Decision

Adopt **full CLI priority** (the issue's option (b)): when `--size` is given,
the CLI owns page geometry including orientation. CSS `@page` orientation is
ignored in the override path; orientation is requested explicitly with
`--landscape` (or `builder.landscape(true)`).

The intentional magic of `--size A4 + CSS landscape = landscape A4` is dropped.
Its only cost is one extra flag: `--size A4 --landscape` reproduces the result
explicitly. This trade-off was confirmed with the maintainer.

### Rejected alternatives

- **(c) keyword-only magic** — preserves `--size A4 + CSS landscape` but not
  `--size 200ptx400pt + CSS landscape`. Impossible as a local `page_settings.rs`
  change (see the constraint above); requires new CLI→config plumbing for a
  behavior the maintainer judged not worth keeping.
- **(a) `--portrait` flag as the primary fix** — does not by itself make
  `--size` authoritative; it only adds a manual override. Split out as separate
  scope (see below).

## Implementation

Single change in `crates/fulgur/src/gcpm/page_settings.rs`, override branch
(currently lines 87-94):

```rust
let (size, landscape) = if config.overrides.page_size {
    // --size given: the CLI fully owns page geometry, including
    // orientation. CSS @page orientation is intentionally ignored;
    // request landscape with --landscape / builder.landscape(true).
    (config.page_size, config.landscape)
} else {
    // ... unchanged non-override path (CSS is respected) ...
};
```

The old branch chose `config.landscape` when `overrides.landscape` was set and
otherwise fell back to `resolve_landscape_from_css(css_size, config.landscape)`.
Both arms collapse to `config.landscape`, so:

- `resolve_landscape_from_css` (lines 152-158) becomes dead and is **deleted**.
- All call sites (`render.rs:104`, `render.rs:188`, `engine.rs:174`) route
  through the same `resolve_page_settings`, so fixing the function fixes every
  render path.

### Behavior matrix (after the change)

| Input | Result |
|-------|--------|
| `--size 200ptx400pt` + CSS landscape | portrait `400` tall (bug fixed) |
| `--size A4` | portrait A4 |
| `--size A4 --landscape` | landscape A4 |
| `--size A4` + CSS landscape | portrait A4 (CSS ignored) |
| no `--size` + CSS landscape | landscape (non-override path, unchanged) |

The fix lives in the library `resolve_page_settings`, so it also affects
library callers, not just the CLI: `Engine::builder().page_size(...)` sets
`overrides.page_size`, so a caller that sets a page size without
`.landscape(true)` now renders portrait even under CSS `@page { size:
landscape }` (previously landscape). This is consistent with option (b)
(override fully owns geometry) and is acceptable under the 0.x lockstep minor
bump.

### Ancillary changes

1. **Help text** (`main.rs`, the `size` arg doc): state that `--size` takes
   priority over CSS `@page` orientation as well, and that `--landscape`
   requests landscape. Removes the current over-promise.
2. **Doc comment** (`page_settings.rs` priority model, lines 44-49): note that
   the override path ignores CSS orientation.
3. **New unit test** `test_cli_override_ignores_css_landscape`: override +
   CSS `KeywordWithOrientation("A4", true)` → `landscape == false`.

## Testing

- Unit test in `page_settings.rs` (pure function; covered by the lib test suite
  per the codecov scope note in `CLAUDE.md`).
- Existing tests are unaffected: no current test asserts
  `override + CSS landscape → landscape`. `test_page_size_landscape_from_css`
  uses the non-override path; `test_cli_override_beats_css` does not check
  orientation.

## Out of scope (separate issues)

- **`--portrait` flag** — a general CLI way to force portrait in the *no-`--size`*
  case (size from CSS, orientation forced from CLI). Requires reworking the CLI
  orientation wiring into a tri-state (`auto`/`portrait`/`landscape`). Filed
  separately.
- **Invalid `--size` footgun** — a bad `--size` value warns and falls back to A4
  while still suppressing CSS `@page`. The issue already notes this as a
  separate axis (`--size` authority); consider a non-zero exit separately.
