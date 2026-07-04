# Public `Engine::layout()` + `LayoutOutput` Implementation Plan

**Goal:** Expose the fulgur layout engine (parse → style → layout → `Drawables`)
as a public `Engine::layout(&self, html) -> Result<LayoutOutput { drawables,
geometry }>`, closing the fulgur-2map epic, without changing any PDF byte output.

**Architecture:** Extract the monolithic `render_pass` body into a private
`layout_to_drawables(&self, html, anchor_map) -> Result<LayoutArtifacts>` helper
(everything up to and including `dom_to_drawables`). `render_pass` becomes a thin
wrapper that calls the helper and drives the unchanged `render_v2`. The new
public `layout()` calls the same helper with `render`'s 2-pass loop and returns
just `{ drawables, geometry }`. One layout path — PDF and layout()/image/OCR
never diverge. The extraction is a pure code-move, so goldens stay byte-identical.

**Tech Stack:** Rust, existing fulgur crate (`engine.rs`, `render_v2`,
`convert::dom_to_drawables`, `pagination_layout`, `drawables`, `units`).

**Design source:** beads `fulgur-2map.10` design field +
`docs/plans/2026-06-27-engine-layout-api-design.md` §3–§4.

---

## Pre-flight (already done, recorded here for the executor)

Clean baseline on `origin/main` (a96d8808) BEFORE any edit was GREEN:

- `examples_determinism`: 11 passed, 0 failed.
- VRT (`run_fulgur_vrt` etc.): all passed.
- `git status --short crates/fulgur-vrt/goldens/`: empty.

This is the load-bearing byte-neutrality baseline. A moved golden after the
refactor means the extraction was not a pure code-move — investigate, never
`FULGUR_VRT_UPDATE=1`.

---

## Task 1: Add public `Engine::layout()` + `LayoutOutput` (TDD)

**Files:**

- Modify: `crates/fulgur/src/engine.rs` (add structs, extract helper, add
  `layout()`)
- Modify: `crates/fulgur/src/lib.rs:70` (re-export `LayoutOutput`)
- Test: `crates/fulgur/tests/render_smoke.rs` (two `layout()` smoke tests)
- Test: `crates/fulgur/tests/public_reachability.rs` (geometry-type reachability
  guard)

### Step 1: Write the failing tests

Append to `crates/fulgur/tests/render_smoke.rs`:

```rust
/// Smoke: public `Engine::layout()` single-pass path. A plain document
/// (no `target-*`) takes the `!needs_pass_two` early return. Asserts the
/// renderer-agnostic layout output carries drawables + geometry, so an
/// out-of-core consumer (image / OCR) has something to compose.
#[test]
fn layout_single_pass_returns_drawables_and_geometry() {
    let out = Engine::builder()
        .build()
        .layout("<html><body><p>hello layout</p></body></html>")
        .expect("layout single-pass");
    assert!(!out.geometry.is_empty(), "geometry should record fragments");
    assert!(
        !out.drawables.paragraphs.is_empty(),
        "the <p> should produce a paragraph draw payload"
    );
}

/// Smoke: public `Engine::layout()` 2-pass path. A `target-counter()` in
/// `::after` forces `needs_pass_two`, so `layout()` falls through the early
/// return and re-lays out with the pass-1 `AnchorMap`. Covers the else-arm
/// of the branch for codecov/patch (the single-pass test covers the `return`).
#[test]
fn layout_two_pass_target_counter_returns_drawables_and_geometry() {
    let html = r##"<!doctype html>
<html><head><style>
  a::after { content: " (p." target-counter(attr(href), page) ")"; }
  h2 { page-break-before: always; }
</style></head>
<body>
  <a href="#a">Chapter A</a>
  <h2 id="a">Chapter A</h2>
  <p>aaa</p>
</body></html>"##;
    let out = Engine::builder()
        .build()
        .layout(html)
        .expect("layout 2-pass");
    assert!(!out.geometry.is_empty(), "geometry should record fragments");
    assert!(!out.drawables.paragraphs.is_empty());
    // Pass-2-SPECIFIC guard: non-empty checks alone pass even if pass 2 never
    // fires. Assert the RESOLVED content — `#a` lands on page 2 via
    // `page-break-before: always`, so `target-counter(attr(href), page)` in the
    // `a::after` resolves to " (p.2)". On a regression to single-pass this stays
    // a fixed-width placeholder, never the literal resolved page number. Reads
    // the pre-raster run text (`ShapedGlyphRun::text`) via the shared
    // `paragraph_run_text` helper, so it is font-independent.
    assert!(
        out.drawables
            .paragraphs
            .values()
            .any(|p| paragraph_run_text(p).contains("(p.2)")),
        "pass-2 target-counter should resolve to \"(p.2)\""
    );
}
```

Append to `crates/fulgur/tests/public_reachability.rs`:

```rust
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
}
```

### Step 2: Run the tests to verify they fail (RED)

```bash
cargo test -p fulgur --test render_smoke layout_ 2>&1 | tail -20
cargo test -p fulgur --test public_reachability 2>&1 | tail -20
```

Expected: compile error — `no method named layout` / `LayoutOutput` unresolved.

### Step 3: Implement in `crates/fulgur/src/engine.rs`

**3a. Add the two structs.** Put `LayoutOutput` (public) near `RenderPassOutput`
(after line 37) and `LayoutArtifacts` (private) beside it:

```rust
/// Renderer-agnostic layout result produced by [`Engine::layout`].
///
/// Carries the parse → style → layout → `Drawables` output that both PDF
/// rendering and out-of-core consumers (image rasterization, OCR label
/// generation) build on, without pulling PDF serialization into core.
///
/// Unit contract: `drawables` coordinates are PDF pt; `geometry` fragments are
/// CSS px (`units::Px`). See the crate `units` module and
/// `.claude/rules/coordinate-system.md`.
pub struct LayoutOutput {
    pub drawables: crate::drawables::Drawables,
    pub geometry: crate::pagination_layout::PaginationGeometryTable,
}

/// Full per-pass output of [`Engine::layout_to_drawables`] — a superset of the
/// public [`LayoutOutput`] holding every side-channel `render::render_v2`
/// needs. `fonts` / `system_fonts` are intentionally absent: both are re-derived
/// from `&self` at the render call site, byte-identically to the old inline path.
struct LayoutArtifacts {
    drawables: crate::drawables::Drawables,
    pagination_geometry: crate::pagination_layout::PaginationGeometryTable,
    gcpm: crate::gcpm::GcpmContext,
    running_store: crate::gcpm::running::RunningElementStore,
    string_set_for_render: HashMap<usize, Vec<(String, String)>>,
    counter_ops_for_render: BTreeMap<usize, Vec<crate::gcpm::CounterOp>>,
    html_title: Option<String>,
    implicit_href_map: BTreeMap<usize, String>,
    collected_anchor_map: AnchorMap,
    needs_pass_two: bool,
}
```

Note: confirm the `GcpmContext` path compiles — it is defined in
`crates/fulgur/src/gcpm/mod.rs`, so `crate::gcpm::GcpmContext`. If the compiler
disagrees, use `crate::gcpm::parser::GcpmContext`.

**3b. Rename `render_pass` → `layout_to_drawables`, change ONLY the tail.**
Rename the current method (engine.rs:106) and its return type:

```rust
fn layout_to_drawables(
    &self,
    html: &str,
    anchor_map: Option<&AnchorMap>,
) -> Result<LayoutArtifacts> {
```

Leave the entire body (parse, GCPM passes, pagination, anchor/implicit maps,
`ConvertContext` build, `dom_to_drawables`) UNCHANGED. Replace ONLY the tail —
the current `let drawables = …; let html_title = …; let pdf = render_v2(…)?;
Ok(RenderPassOutput { … })` block (engine.rs:598-619) with:

```rust
        let drawables = crate::convert::dom_to_drawables(&doc, &mut convert_ctx);
        let html_title = crate::blitz_adapter::extract_html_title(&doc);
        // Reclaim the post-convert geometry without partially moving
        // `convert_ctx`, then drop it so its `&running_store` borrow ends and
        // `running_store` can be moved into the artifacts.
        let pagination_geometry = std::mem::take(&mut convert_ctx.pagination_geometry);
        drop(convert_ctx);
        Ok(LayoutArtifacts {
            drawables,
            pagination_geometry,
            gcpm,
            running_store,
            string_set_for_render,
            counter_ops_for_render,
            html_title,
            implicit_href_map,
            collected_anchor_map,
            needs_pass_two: needs_anchor_map_for_pass_two,
        })
    }
```

Rationale for `mem::take` + `drop`: `render_v2` previously borrowed
`&convert_ctx.pagination_geometry`, but the helper must MOVE the geometry out
while `convert_ctx` still holds `running_store: &running_store`. `mem::take`
(PaginationGeometryTable is `BTreeMap`, so `Default`) leaves `convert_ctx` fully
valid; `drop(convert_ctx)` then ends the `&running_store` borrow so it can move.
No computation changes — `dom_to_drawables` already populated the table.

**3c. Add the thin `render_pass` wrapper** immediately after
`layout_to_drawables`:

```rust
/// Single render pass: lay out via [`layout_to_drawables`], then serialize the
/// pages via the unchanged [`render::render_v2`]. `render`'s 2-pass loop is
/// untouched. See [`RenderPassOutput`] for the returned fields.
fn render_pass(&self, html: &str, anchor_map: Option<&AnchorMap>) -> Result<RenderPassOutput> {
    let LayoutArtifacts {
        drawables,
        pagination_geometry,
        gcpm,
        running_store,
        string_set_for_render,
        counter_ops_for_render,
        html_title,
        implicit_href_map,
        collected_anchor_map,
        needs_pass_two,
    } = self.layout_to_drawables(html, anchor_map)?;

    // Re-derive fonts / system_fonts from `&self` — byte-identical to the
    // value `layout_to_drawables` used for parsing.
    let fonts = self
        .assets
        .as_ref()
        .map(|a| a.fonts.as_slice())
        .unwrap_or(&[]);

    let pdf = crate::render::render_v2(
        &self.config,
        &pagination_geometry,
        &drawables,
        &gcpm,
        &running_store,
        fonts,
        self.system_fonts,
        &string_set_for_render,
        &counter_ops_for_render,
        html_title,
        self.serialize_settings.clone(),
        anchor_map,
        &implicit_href_map,
    )?;
    Ok(RenderPassOutput {
        pdf,
        anchor_map: collected_anchor_map,
        needs_pass_two,
    })
}
```

**3d. Add the public `layout()`** inside `impl Engine` (near `render`, e.g.
after `render_file`):

```rust
/// Lay out `html` and return the renderer-agnostic per-node draw payloads
/// (`drawables`) plus the pagination geometry, without serializing a PDF.
///
/// Page shape (canvas size, margins, orientation) comes from the builder
/// configuration, exactly as [`render`](Engine::render) uses it. Documents
/// with `target-counter()` / `target-counters()` / `target-text()` run the
/// same internal 2-pass resolution as `render`, so the returned `drawables`
/// carry resolved cross-reference values, not fixed-width placeholders.
///
/// This is the shared layout path behind `render`; a downstream image
/// rasterizer or OCR-label generator can consume [`LayoutOutput`] without
/// pulling PDF serialization into core.
pub fn layout(&self, html: &str) -> Result<LayoutOutput> {
    // Pass 1, mirroring `render`'s 2-pass loop.
    let pass1 = self.layout_to_drawables(html, None)?;
    if !pass1.needs_pass_two {
        return Ok(LayoutOutput {
            drawables: pass1.drawables,
            geometry: pass1.pagination_geometry,
        });
    }
    // Pass 2: re-lay-out with the pass-1 AnchorMap so `target-*` resolve.
    let pass2 = self.layout_to_drawables(html, Some(&pass1.collected_anchor_map))?;
    Ok(LayoutOutput {
        drawables: pass2.drawables,
        geometry: pass2.pagination_geometry,
    })
}
```

**3e. Re-export from `crates/fulgur/src/lib.rs:70`:**

```rust
pub use engine::{Engine, EngineBuilder, LayoutOutput};
```

### Step 4: Run the tests to verify they pass (GREEN)

```bash
cargo test -p fulgur --test render_smoke layout_ 2>&1 | tail -20
cargo test -p fulgur --test public_reachability 2>&1 | tail -20
```

Expected: both `layout_*` smoke tests + the reachability guard PASS.

### Step 5: Build / clippy / fmt clean

```bash
cargo build -p fulgur 2>&1 | tail -5
cargo clippy -p fulgur --all-targets -- -D warnings 2>&1 | tail -15
cargo fmt --check 2>&1 | tail -5
```

Expected: no errors, no warnings, no fmt diff.

### Step 6: Full lib test suite

```bash
cargo test -p fulgur --lib 2>&1 | tail -10
cargo test -p fulgur --test render_smoke 2>&1 | tail -5
cargo test -p fulgur --test public_reachability 2>&1 | tail -5
```

Expected: all green (existing ~340 lib tests unaffected — `render_pass`'s
observable behavior is unchanged).

### Step 7: Commit

```bash
git add crates/fulgur/src/engine.rs crates/fulgur/src/lib.rs \
        crates/fulgur/tests/render_smoke.rs \
        crates/fulgur/tests/public_reachability.rs \
        docs/plans/2026-07-04-engine-layout-public-api.md
git commit -m "feat(engine): expose public Engine::layout() + LayoutOutput (fulgur-2map.10)"
```

---

## Task 2: Prove byte-neutrality (goldens unchanged)

**Files:** none modified — verification only.

### Step 1: Run the load-bearing golden suites

```bash
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
cargo test -p fulgur-cli --test examples_determinism 2>&1 | tail -5
cargo test -p fulgur-vrt 2>&1 | tail -15
```

Expected: same GREEN as the pre-flight baseline (determinism 11 passed; VRT all
passed).

### Step 2: Confirm goldens are byte-identical

```bash
git status --short -- crates/fulgur-vrt/goldens/
```

Expected: EMPTY. A non-empty result means the extraction reassociated or dropped
a computation — investigate the diff; do NOT run `FULGUR_VRT_UPDATE=1`.

---

## Out of scope (do not touch)

- `fulgur-image` / OCR crates, rasterize / encode, CLI `rasterize` subcommand,
  the glyph-composition single-source helper (design doc §9–§10).
- Builder `PageSize` / `Margin` typing (permanently out of epic scope).
- Docs for the `layout()` API → tracked separately as `fulgur-2map.11` (P3c),
  blocked on this task.
