# fulgur-sipv small leaves — VerticalAlign + table/inline-baseline → Pt

**Goal:** Eliminate the remaining `px_to_pt`/`pt_to_px` in two clean, self-contained leaf
subsystems by typing their data to `units::Pt`, byte-neutral. Part of epic `fulgur-sipv`.

**Scope:** `fulgur-sipv.1` (VerticalAlign) + `fulgur-sipv.3` (table cw/ch + inline baseline).
`fulgur-sipv.2` (BoxShadow) is **descoped** — consumer audit showed the shadow-draw path
(`draw_single_box_shadow(x: f32,…)` in `background.rs`) is f32, so typing `BoxShadow` fields
to `Pt` would add ~10 `.to_f32()` in the draw arithmetic (net +6 conversions, draw layer
still f32). It is blocked on a draw-layer typing foundation; see the issue notes.

**Consumer audit (why these two are clean):** both feed already-`Pt` sinks, so migrating the
type lets the manual re-tags drop rather than forcing `.to_f32()`.

---

## Byte-neutral gate (same as P2b)

Each edit is either dropping a `.pt()`/`.to_f32()` re-tag (sink already `Pt`) or swapping
`px_to_pt(x)` → `x.px().in_pt()`. No arithmetic reassociation. After each task:

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo build -p fulgur && cargo clippy -p fulgur --all-targets -- -D warnings && cargo fmt --check
cargo test -p fulgur --lib && cargo test -p fulgur-cli --test examples_determinism
cargo test -p fulgur-vrt && git status --short -- crates/fulgur-vrt/goldens/   # MUST be empty
```

Baseline captured GREEN before edits (lib 1579, determinism 11, VRT clean). NEVER
`FULGUR_VRT_UPDATE=1`.

---

## Task 1 — VerticalAlign::Length(f32) → Length(Pt) (`fulgur-sipv.1`)

**Files:** `paragraph.rs` (enum def + consumer + test), `blitz_adapter.rs` (producer + test).

| Site | Current | Fate |
|------|---------|------|
| `paragraph.rs:118` | `Length(f32)` | → `Length(Pt)` (enum variant) |
| `blitz_adapter.rs:1247` | `VerticalAlign::Length(px_to_pt(px))` | → `VerticalAlign::Length(px.px().in_pt())` |
| `paragraph.rs:947` | `Length(v) => baseline - v.pt() - img.height` | drop `.pt()` → `baseline - v - img.height` (`baseline`/`img.height` are `Pt` — verify) |
| `blitz_adapter.rs:5143-5146` (test) | `Length(v) => …`, panic msg `Length(6.0)` | read `v` as `Pt`; assert via `.to_f32()` or `_f32.pt()` |
| `paragraph.rs:1323` (test) | `Length(3.0)` | → `Length(3.0_f32.pt())` |

Bring `Pt`/`F32Units` into scope where needed. `px_to_pt(px)` = `Pt(px*0.75)` = `px.px().in_pt()` (byte-identical). Commit: `refactor(paragraph): type VerticalAlign::Length to Pt (fulgur-sipv.1)`.

## Task 2 — table cw/ch + inline baseline → Pt (`fulgur-sipv.3`)

**2a table cw/ch** (`convert/table.rs`): `cw`/`ch` are used only in zero-compares.

| Site | Current | Fate |
|------|---------|------|
| `table.rs:111-112` | `let cw = px_to_pt(child_node.final_layout.size.width);` | → `child_node.final_layout.size.width.px().in_pt()` (Pt) |
| `table.rs:120,126` | `ch == 0.0 && cw == 0.0` | → `ch == Pt::ZERO && cw == Pt::ZERO` |

**2b inline baseline** (`convert/inline_root.rs`): type both fns' return `Option<f32>` → `Option<Pt>`.

| Site | Current | Fate |
|------|---------|------|
| `inline_root.rs:328,349` | `-> Option<f32>` (both fns) | → `-> Option<Pt>` |
| `inline_root.rs:364-365` | `top_inset = border_widths[0].to_f32() + padding[0].to_f32()` | drop `.to_f32()` → `border_widths[0] + padding[0]` (Pt) |
| `inline_root.rs:367` | `Some(top_inset + line.baseline.to_f32())` | drop `.to_f32()` → `Some(top_inset + line.baseline)` (Pt+Pt, order preserved) |
| `inline_root.rs:391` | `Some(px_to_pt(child.final_layout.location.y) + inner)` | → `Some(child.final_layout.location.y.px().in_pt() + inner)` (Pt+Pt) |
| `inline_root.rs:575` (consumer) | `.map(\|bo\| height - bo.pt())` | drop `.pt()` → `height - bo` (`height` is `Pt`, confirmed) |
| tests `1269,1286,1317,1338,1375` | `Some(12.0)` / `Some(9.0)` etc. | → `Some(12.0_f32.pt())` etc.; `1253,1357` `is_none()` unchanged |

Commit: `refactor(convert): type table cw/ch and inline baseline to Pt (fulgur-sipv.3)`.

## Task 3 — verify + PR

Full cadence green, goldens untouched. New PROD lines (paragraph baseline arm, inline
baseline chain, table cell walk) are exercised by existing non-VRT tests (paragraph/inline/
table integration + the baseline unit tests above); confirm via the coverage check if in
doubt. Open one PR for both subtasks; close `fulgur-sipv.1` + `fulgur-sipv.3` on merge.
