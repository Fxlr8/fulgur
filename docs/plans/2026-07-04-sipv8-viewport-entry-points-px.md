# fulgur-sipv.8 — Type Blitz viewport entry points to `units::Px`

**Goal:** Eliminate every remaining `convert::pt_to_px` call site at the Blitz/Taffy
viewport boundary by typing the two entry-point signatures `parse_and_layout` and
`run_pass_with_break_styles` to `units::Px`, collapsing each call site to
`…as_pt().in_px()` (or `.to_f32()` at a still-f32 sink).

**Architecture:** Interpretation **A** (issue-faithful, minimal boundary): type ONLY the
two functions the issue names. All other f32 sinks
(`parse_html_with_local_resources`, `set_viewport_size_px`, `relayout_position_fixed`,
`ConvertContext.viewport_size_px`, `append_position_fixed_fragments`,
`run_pass_with_break_and_running`, `run_pass`) keep `f32`, so call sites feeding them
end in `.to_f32()`. This mirrors the pattern sipv.7 already merged.

**Tech stack:** Rust, `crate::units::{Px, Pt, F32Units}`. Conversion identity
(confirmed at `convert/mod.rs:61-69`): `pt_to_px(v) == v / PX_TO_PT == Pt(v).in_px()`,
`px_to_pt(v) == v * PX_TO_PT == Px(v).in_pt()`. No reciprocal-multiply → every swap is
byte-identical.

**Byte-neutral protocol (epic fulgur-sipv, from P2b/sipv.7):** baseline GREEN before any
edit; VRT goldens byte-identical after (`git status` on `crates/fulgur-vrt/goldens`
empty; NEVER `FULGUR_VRT_UPDATE=1`); `cargo build`/`clippy -D warnings`/`fmt --check`
clean; `cargo test -p fulgur --lib` (incl. `render_smoke`) green; `examples_determinism`
green. Preserve arithmetic operand order (no reassociation).

## The two idioms (advisor's highest-risk trap — categorize per site, NO global sed)

| Source value | Correct swap | Ends in |
|---|---|---|
| raw f32 already in **px** (bare literal `595.0`, `800.0`) | `595.0_f32.as_px()` | `Px` |
| raw f32 in **pt** wrapped in `pt_to_px(x)` | `x.as_pt().in_px()` | `Px` (÷0.75) |
| `Pt`-typed value fed via `.in_px().to_f32()` (sipv.7) | `.in_px()` | `Px` (drop `.to_f32()`) |
| Px result but sink is still f32 | append `.to_f32()` | `f32` |
| Px result but sink is u32 | append `.to_f32() as u32` | `u32` |

Miscategorizing a bare px literal as `.as_pt().in_px()` injects a spurious ÷0.75;
using `.as_px()` on a `pt_to_px` site drops the ÷0.75. Both rebless goldens. Check each.

---

## Task 1: Type `run_pass_with_break_styles(page_height_px: Px)` + all callers (one atomic commit)

**Files:**

- Modify: `crates/fulgur/src/pagination_layout.rs`
  - `212-217`: param `page_height_px: f32` → `page_height_px: crate::units::Px`; body
    `run_pass_inner(doc, page_height_px, …)` → `run_pass_inner(doc, page_height_px.to_f32(), …)`.
  - Test callers passing **bare px literals** → `.as_px()`:
    `5307` (800.0), `5361` (800.0), `5408` (250.0), `5455` (150.0), `5495` (250.0),
    `5530` (250.0), `5570` (250.0), `5610` (250.0), `5646` (250.0), `5693` (800.0),
    `5763` (800.0), `5850` (400.0), `5883` (800.0), `5920` (100.0), `6077` (200.0),
    `6124` (200.0), `6313` (100.0), `6352` (100.0), `6407` (100.0), `6461` (100.0).
    (Line numbers are pre-edit anchors; grep `run_pass_with_break_styles(` to enumerate.)
  - `4402`: `pt_to_px(720.0)` → `720.0_f32.as_pt().in_px()` (pt-source, ÷0.75). Then the
    `use crate::convert::pt_to_px;` at `4368` becomes unused → remove it (verify no other
    use in that `#[test]` fn body).
- Modify: `crates/fulgur/src/engine.rs`
  - `872` (`build_drawables_for_testing_no_gcpm`) and `937`
    (`build_drawables_and_geometry_for_testing_no_gcpm`):
    `crate::convert::pt_to_px(self.config.content_height())` →
    `self.config.content_height().as_pt().in_px()` (NO `.to_f32()` — sink is now `Px`).
- Modify: `crates/fulgur/src/render.rs`
  - `3703` (feeds `run_pass_with_break_styles`): `rect.height.in_px().to_f32()` →
    `rect.height.in_px()` (drop `.to_f32()`; `rect.height` is `Pt`).

**Note:** `run_pass` (test-only) and `run_pass_with_break_and_running` (production body
path) intentionally stay `f32` — only the `styles` variant had `pt_to_px` sites to kill.
`F32Units` must be in scope where `.as_px()`/`.as_pt()` is used (add
`use crate::units::F32Units;` to the test module / check existing imports).

**Step 1 — verify baseline green** (done before editing; see baseline task).

**Step 2 — apply the edits above.**

**Step 3 — build + lib test:**
Run: `cargo build -p fulgur && cargo test -p fulgur --lib 2>&1 | tail`
Expected: compiles, all lib tests pass. Watch for a byte-diff surfacing as a failing
pagination fragmentation assertion (would mean an idiom miscategorization at 4402).

**Step 4 — commit:**
`refactor(fulgur): type run_pass_with_break_styles page_height to units::Px`

---

## Task 2: Type `parse_and_layout(viewport_width: Px, viewport_height: Px)` + all callers (one atomic commit)

**Files:**

- Modify: `crates/fulgur/src/blitz_adapter.rs`
  - `144-163`: params `viewport_width: f32, viewport_height: f32` →
    `viewport_width: crate::units::Px, viewport_height: crate::units::Px`. Body:
    - `parse_inner(html, viewport_width, viewport_height as u32, …)` →
      `parse_inner(html, viewport_width.to_f32(), viewport_height.to_f32() as u32, …)`
      (preserve the `as u32` truncation on the SAME f32 value).
    - `relayout_position_fixed(&mut doc, viewport_width, viewport_height)` →
      `relayout_position_fixed(&mut doc, viewport_width.to_f32(), viewport_height.to_f32())`.
  - Test callers passing **bare px literals** → `.as_px()` each arg:
    `3674` (400.0, 600.0), `5095/5105/5118/5140/5160/5177/5372` (400.0, 2000.0).
- Modify: `crates/fulgur/src/render.rs`
  - `3586-3592` (measure): `pt_to_px(content_width)`→`content_width.as_pt().in_px()`,
    `pt_to_px(page_size.height)`→`page_size.height.as_pt().in_px()`.
  - `3621-3627` (height measure): `pt_to_px(fixed_width)`→`fixed_width.as_pt().in_px()`,
    `pt_to_px(page_size.height)`→`page_size.height.as_pt().in_px()`.
    (`content_width`, `fixed_width`, `page_size.height` are f32 **pt** here.)
  - `3693-3699` (render): `rect.width.in_px().to_f32()`→`rect.width.in_px()`,
    `rect.height.in_px().to_f32()`→`rect.height.in_px()` (drop `.to_f32()`; both `Pt`).
- Modify test callers passing **bare px literals** → `.as_px()`:
  - `crates/fulgur/src/convert/inline_root.rs:1077` (595.0, 842.0)
  - `crates/fulgur/src/convert/replaced.rs:345` (595.0, 842.0)
  - `crates/fulgur/src/convert/mod.rs:1202` (595.0, 842.0)
  - `crates/fulgur/src/convert/positioned.rs`: `474`, `630`, `667`, `700`, `733`, `768`,
    `807`, `1047`, `1078`, `1108`, `1135`, `1175`, `1215`, `1280` (each `595.0, 842.0`).
    Grep `parse_and_layout(` in the file to confirm all sites and multi-line arg layout.

**Step 1 — apply edits.**

**Step 2 — build + lib test:**
Run: `cargo build -p fulgur && cargo test -p fulgur --lib 2>&1 | tail`
Expected: compiles green. Ensure `F32Units` is in scope in each edited test module.

**Step 3 — commit:**
`refactor(fulgur): type parse_and_layout viewport dims to units::Px`

---

## Task 3: Remaining engine.rs viewport `pt_to_px` sites + pagination 4469 + dead_code gate (one atomic commit)

These feed still-f32 sinks → each ends in `.to_f32()` (or `.to_f32() as u32`). After this
commit, production `pt_to_px` callers = 0.

**Files:**

- Modify: `crates/fulgur/src/engine.rs` — replace `crate::convert::pt_to_px(X)` with
  `X.as_pt().in_px().to_f32()`; the two `… as u32` sites keep `as u32`:
  - `210` width → `.to_f32()`; `211` `page_height()` → `.to_f32() as u32` (feeds
    `parse_html_with_local_resources`).
  - `274` `resolved_content_width_px = …pt.as_pt().in_px().to_f32()`;
    `275` `resolved_content_height_px = …pt.as_pt().in_px().to_f32()`
    (f32 locals reused at 278/463/525-527/548-574/664 — must stay f32).
  - `851` `.to_f32()`; `852` `.to_f32() as u32` (parse_html, testing helper A).
  - `865/866` `.to_f32()` (`relayout_position_fixed`).
  - `889/890` `.to_f32()` (`ConvertContext.viewport_size_px: Some((f32,f32))`).
  - `916` `.to_f32()`; `917` `.to_f32() as u32` (parse_html, testing helper B).
  - `930/931` `.to_f32()` (`relayout_position_fixed`).
  - `947/948` `content_w_px/content_h_px = …to_f32()` (`append_position_fixed_fragments`).
- Modify: `crates/fulgur/src/pagination_layout.rs`
  - `4469`: `pt_to_px(800.0)` → `800.0_f32.as_pt().in_px().to_f32()` (feeds
    `run_pass_with_break_and_running`, which stays f32). Remove the now-unused
    `use crate::convert::pt_to_px;` at `4444`.
- Modify: `crates/fulgur/src/convert/mod.rs`
  - `pt_to_px` (`67-69`) now has zero non-test callers (only the `1623` roundtrip test).
    Gate it: add `#[cfg(test)]` above `#[inline]`/the fn. Leave `px_to_pt` untouched
    (shadow.rs still uses it until sipv.2). Confirm `cargo build` (no `--tests`) is warning
    -clean.

**Step 1 — apply edits.**

**Step 2 — full verification (the gate):**

```bash
# no pt_to_px symbols remain except the definition + roundtrip test
grep -rn "convert::pt_to_px\|pt_to_px(" crates/fulgur/src   # expect only mod.rs def + :1623 test
cargo build -p fulgur                 # non-test build: pt_to_px must not warn dead_code
cargo build -p fulgur --tests
cargo clippy -p fulgur --all-targets -- -D warnings
cargo fmt --check
cargo test -p fulgur --lib 2>&1 | tail
```

Expected: all clean/green.

**Step 3 — commit:**
`refactor(fulgur): type remaining engine viewport feeds to units::Px`

---

## Task 4: Byte-identity + determinism + coverage gate

**Step 1 — VRT byte-identical (the critical gate for this boundary):**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt 2>&1 | tail
git status --short crates/fulgur-vrt/goldens    # MUST be empty
```

If any golden diff appears → an idiom was miscategorized (4/3 scale bug). Do NOT
`FULGUR_VRT_UPDATE=1`; bisect the offending site instead.

**Step 2 — examples determinism:**
Run: `FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-cli --test examples_determinism 2>&1 | tail`
Expected: green.

**Step 3 — coverage check (preempt codecov/patch, per project_units_migration_patch_coverage):**
The changed PROD lines live in already-covered functions (parse_and_layout / run_pass have
many test callers; engine 208-280 via `render_smoke`). The classic artifact (assert-arg
`.to_f32()` cold region) does NOT apply — the new `.to_f32()` sit in hot call-arg
positions, not panic args. Verify empirically with the CI-equivalent command; if any added
line intersects an lcov `DA:N,0`, add a targeted non-VRT test (do NOT `FULGUR_VRT_UPDATE`).

```bash
cargo llvm-cov nextest --workspace --exclude fulgur-vrt --lcov --output-path /tmp/sipv8.lcov 2>&1 | tail
```

**Step 4 — final commit / open PR** (title English, body Japanese per project convention).

---

## Out of scope (state in PR body to preempt reviewer)

- `run_pass_with_break_and_running` / `run_pass` stay `f32` — only the `styles` variant had
  `pt_to_px` sites; 4469 feeds the running variant purely for symbol elimination.
- `fn px_to_pt` and `fn pt_to_px` definitions are NOT deleted — `px_to_pt` is still used by
  `shadow.rs` (sipv.2, still open). Both defs get removed when sipv.2 lands (px_to_pt's last
  caller). `pt_to_px` is `#[cfg(test)]`-gated here as interim.
