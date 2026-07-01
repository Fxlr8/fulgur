# fulgur-9vw5: Fix 4/3 over-translation for absolute-length CSS transform Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the 4/3 over-translation bug in `compute_transform`/`op_to_matrix` (`blitz_adapter.rs`) so absolute-length `translate()`, `matrix()` tx/ty, and absolute-length `transform-origin` fold px→pt with a real `×0.75` conversion instead of a no-op type tag, while leaving all percentage-based paths byte-identical.

**Architecture:** Introduce one helper, `resolve_length_component(lp, basis_pt) -> Pt`, that branches on `LengthPercentage::has_percentage()`: percentage results keep today's pt-basis self-consistent tag (`.px().pt()`, unchanged), pure absolute-length results get the real fold (`.px().in_pt()`). Wire it into the three `Translate*` arms and the `transform-origin` resolution in `op_to_matrix`/`compute_transform`; fix `Matrix(m).e/.f` (always absolute) directly with `.px().in_pt()`, no branch needed. `record_transform`'s pt-basis dims feed is untouched.

**Tech Stack:** Rust, stylo 0.8.0 (`style::values::computed::LengthPercentage`), `fulgur::units::{F32Units, Px, Pt}`.

**Design reference:** Full root-cause analysis and rejected-alternative rationale (Approach A) is saved on the beads issue — run `bd show fulgur-9vw5` for the design field. Do not duplicate it into `docs/plans/`; this plan only contains the executable steps.

---

### Task 1: Update `transform_integration.rs` to describe the correct behavior

**Files:**
- Modify: `crates/fulgur/tests/transform_integration.rs`

Three existing tests currently assert the *buggy* (unconverted) values. Two new tests pin the issue's worked example and the newly-in-scope absolute `transform-origin` fix. One new regression-guard test locks in that percentage paths do not change.

**Step 1: Fix `translate_px` (lines 61-75) to expect the real px→pt fold**

Replace:
```rust
#[test]
fn translate_px() {
    let html = make_html("transform: translate(10px, 20px);");
    let entry = entry_from(&html);
    // For pure translations, T(ox, oy) * M * T(-ox, -oy) = M regardless of origin,
    // so the effective matrix at any draw point equals the raw matrix (plus the
    // draw-point's own translation, which we cancel by passing (0, 0)).
    let m = effective_matrix(&entry, 0.0, 0.0);
    approx(m.a, 1.0, 1e-5, "translate.a");
    approx(m.b, 0.0, 1e-5, "translate.b");
    approx(m.c, 0.0, 1e-5, "translate.c");
    approx(m.d, 1.0, 1e-5, "translate.d");
    approx(m.e.to_f32(), 10.0, 1e-5, "translate.e");
    approx(m.f.to_f32(), 20.0, 1e-5, "translate.f");
}
```

With:
```rust
#[test]
fn translate_px() {
    let html = make_html("transform: translate(10px, 20px);");
    let entry = entry_from(&html);
    // For pure translations, T(ox, oy) * M * T(-ox, -oy) = M regardless of origin,
    // so the effective matrix at any draw point equals the raw matrix (plus the
    // draw-point's own translation, which we cancel by passing (0, 0)).
    let m = effective_matrix(&entry, 0.0, 0.0);
    approx(m.a, 1.0, 1e-5, "translate.a");
    approx(m.b, 0.0, 1e-5, "translate.b");
    approx(m.c, 0.0, 1e-5, "translate.c");
    approx(m.d, 1.0, 1e-5, "translate.d");
    // 10px * 0.75 = 7.5pt, 20px * 0.75 = 15.0pt (real px->pt fold, not a bare tag).
    approx(m.e.to_f32(), 7.5, 1e-5, "translate.e");
    approx(m.f.to_f32(), 15.0, 1e-5, "translate.f");
}
```

**Step 2: Fix `matrix_preserved_with_origin_zero` (lines 119-136)**

Replace:
```rust
#[test]
fn matrix_preserved_with_origin_zero() {
    let html = make_html("transform: matrix(1, 2, 3, 4, 5, 6); transform-origin: 0 0;");
    let entry = entry_from(&html);
    // With origin (0, 0) the conjugation collapses to the identity on both
    // sides, so the stored raw matrix should round-trip verbatim.
    assert_eq!(
        entry.matrix,
        Affine2D {
            a: 1.0,
            b: 2.0,
            c: 3.0,
            d: 4.0,
            e: 5.0_f32.pt(),
            f: 6.0_f32.pt(),
        }
    );
}
```

With:
```rust
#[test]
fn matrix_preserved_with_origin_zero() {
    let html = make_html("transform: matrix(1, 2, 3, 4, 5, 6); transform-origin: 0 0;");
    let entry = entry_from(&html);
    // With origin (0, 0) the conjugation collapses to the identity on both
    // sides, so the stored raw matrix should round-trip verbatim. matrix()'s
    // tx,ty (5, 6) are always absolute CSS px per spec, so they fold to
    // pt (5*0.75=3.75, 6*0.75=4.5) same as an equivalent translate() would.
    assert_eq!(
        entry.matrix,
        Affine2D {
            a: 1.0,
            b: 2.0,
            c: 3.0,
            d: 4.0,
            e: 3.75_f32.pt(),
            f: 4.5_f32.pt(),
        }
    );
}
```

**Step 3: Fix `composition_right_to_left` (lines 150-161)**

Replace:
```rust
#[test]
fn composition_right_to_left() {
    let html = make_html("transform: translate(10px, 0) rotate(90deg); transform-origin: 0 0;");
    let entry = entry_from(&html);
    let m = effective_matrix(&entry, 0.0, 0.0);
    // CSS transforms apply right-to-left: rotate first, then translate.
    // point (1, 0) -> rotate90 -> (0, 1) -> translate(10, 0) -> (10, 1).
    let x = m.a * 1.0 + m.c * 0.0 + m.e.to_f32();
    let y = m.b * 1.0 + m.d * 0.0 + m.f.to_f32();
    approx(x, 10.0, 1e-4, "compose.x");
    approx(y, 1.0, 1e-4, "compose.y");
}
```

With:
```rust
#[test]
fn composition_right_to_left() {
    let html = make_html("transform: translate(10px, 0) rotate(90deg); transform-origin: 0 0;");
    let entry = entry_from(&html);
    let m = effective_matrix(&entry, 0.0, 0.0);
    // CSS transforms apply right-to-left: rotate first, then translate.
    // point (1, 0) -> rotate90 -> (0, 1) -> translate(10px=7.5pt, 0) -> (7.5, 1).
    let x = m.a * 1.0 + m.c * 0.0 + m.e.to_f32();
    let y = m.b * 1.0 + m.d * 0.0 + m.f.to_f32();
    approx(x, 7.5, 1e-4, "compose.x");
    approx(y, 1.0, 1e-4, "compose.y");
}
```

**Step 4: Add new test pinning the issue's worked example**

Add after `translate_px`:
```rust
#[test]
fn translate_px_absolute_length_folds_to_pt() {
    // The issue's worked example: translate(20px) must land at 20 * 0.75 =
    // 15pt, not the buggy unconverted 20pt (a 4/3 over-shift).
    let html = make_html("transform: translate(20px, 0);");
    let entry = entry_from(&html);
    let m = effective_matrix(&entry, 0.0, 0.0);
    approx(m.e.to_f32(), 15.0, 1e-5, "translate20px.e");
    approx(m.f.to_f32(), 0.0, 1e-5, "translate20px.f");
}
```

**Step 5: Add new test for absolute-length `transform-origin`**

Add after `rotate_90_at_default_center_origin_fixes_center`:
```rust
#[test]
fn transform_origin_absolute_length_folds_to_pt() {
    // Same root-cause bug as translate: an absolute-length transform-origin
    // must fold px->pt for real (20px -> 15pt, 10px -> 7.5pt), not tag
    // unconverted. `TransformEntry.origin` is read directly (not through
    // effective_matrix), so any non-identity op works here to keep
    // compute_transform from folding to `None`; scale(2) is the simplest.
    let html = make_html("transform: scale(2); transform-origin: 20px 10px;");
    let entry = entry_from(&html);
    approx(entry.origin.x.to_f32(), 15.0, 1e-5, "origin.x");
    approx(entry.origin.y.to_f32(), 7.5, 1e-5, "origin.y");
}
```

**Step 6: Add regression-guard test for unchanged percentage `translate`**

Add after `translate_px_absolute_length_folds_to_pt`:
```rust
#[test]
fn translate_percentage_unchanged_self_consistent() {
    // Percentage translate must keep resolving against the pt-basis feed
    // exactly as before (self-consistent, not a bug): .t is 100x100 CSS px
    // = 75x75 pt, so translate(50%, 50%) -> (37.5, 37.5) pt, unchanged by
    // this fix (only the absolute-length path changes).
    let html = make_html("transform: translate(50%, 50%);");
    let entry = entry_from(&html);
    let m = effective_matrix(&entry, 0.0, 0.0);
    approx(m.e.to_f32(), 37.5, 1e-5, "translate50pct.e");
    approx(m.f.to_f32(), 37.5, 1e-5, "translate50pct.f");
}
```

**Step 7: Run tests to verify the expected failures**

Run: `cargo test -p fulgur --test transform_integration 2>&1 | tail -40`

Expected: `translate_px`, `matrix_preserved_with_origin_zero`, `composition_right_to_left`, `translate_px_absolute_length_folds_to_pt`, `transform_origin_absolute_length_folds_to_pt` FAIL (current code has not been touched yet — these assert the *fixed* values against the *buggy* implementation). `translate_percentage_unchanged_self_consistent` and all pre-existing origin/rotate/scale/skew tests PASS (they don't touch the absolute-length path).

**Step 8: Commit the test changes on their own**

```bash
git add crates/fulgur/tests/transform_integration.rs
git commit -m "test: pin correct px->pt fold for absolute transform translate/origin (fulgur-9vw5)"
```

---

### Task 2: Implement the real px→pt fold

**Files:**
- Modify: `crates/fulgur/src/blitz_adapter.rs:2879-2979` (`compute_transform`, `op_to_matrix`)

**Step 1: Add the `resolve_length_component` helper**

In `blitz_adapter.rs`, immediately after `compute_transform`'s closing `}` (currently line 2927) and before `fn op_to_matrix` (currently line 2929), insert:

```rust
/// Resolve a `translate`/`transform-origin` length-percentage component
/// against `basis_pt` (the pt-basis dims `record_transform` feeds — see
/// the "PR 8i note" in `convert::record_transform`). Percentages resolve
/// self-consistently against the pt basis and land in pt units
/// unconverted; pure absolute lengths are genuine CSS px per Stylo's
/// `resolve()` contract and need the real px->pt fold. `calc(px + %)`
/// mixes both inside one `resolve()` call — treated as the percentage
/// path (documented limitation, not fixed here; see fulgur-9vw5 design).
fn resolve_length_component(
    lp: &style::values::computed::LengthPercentage,
    basis_pt: f32,
) -> crate::units::Pt {
    use crate::units::F32Units;
    use style::values::computed::Length;
    let resolved = lp.resolve(Length::new(basis_pt)).px();
    if lp.has_percentage() {
        resolved.pt()
    } else {
        resolved.in_pt()
    }
}
```

**Step 2: Rewrite `compute_transform`'s origin resolution to use the helper**

Replace (currently lines 2914-2924):
```rust
    let origin = styles.clone_transform_origin();
    let origin_x = origin
        .horizontal
        .resolve(Length::new(border_box_width))
        .px()
        .pt();
    let origin_y = origin
        .vertical
        .resolve(Length::new(border_box_height))
        .px()
        .pt();
```

With:
```rust
    let origin = styles.clone_transform_origin();
    let origin_x = resolve_length_component(&origin.horizontal, border_box_width);
    let origin_y = resolve_length_component(&origin.vertical, border_box_height);
```

**Step 3: Remove now-unused imports from `compute_transform`**

`compute_transform`'s body no longer calls `.px()`/`.pt()` or `Length::new` directly (both moved into the helper). Delete these two lines from the top of `compute_transform` (currently lines 2884-2885):
```rust
    use crate::units::F32Units;
    use style::values::computed::Length;
```

**Step 4: Rewrite `op_to_matrix`'s `Matrix`/`Translate`/`TranslateX`/`TranslateY` arms**

Replace (currently lines 2938-2956):
```rust
    match op {
        Matrix(m) => Affine2D {
            a: m.a,
            b: m.b,
            c: m.c,
            d: m.d,
            e: m.e.pt(),
            f: m.f.pt(),
        },
        Translate(x, y) => Affine2D::translation(
            x.resolve(Length::new(w)).px().pt(),
            y.resolve(Length::new(h)).px().pt(),
        ),
        TranslateX(x) => {
            Affine2D::translation(x.resolve(Length::new(w)).px().pt(), crate::units::Pt::ZERO)
        }
        TranslateY(y) => {
            Affine2D::translation(crate::units::Pt::ZERO, y.resolve(Length::new(h)).px().pt())
        }
```

With:
```rust
    match op {
        // matrix()'s tx,ty are always absolute <number> (CSS px-equivalent
        // per spec, never a percentage), so they always take the real fold.
        Matrix(m) => Affine2D {
            a: m.a,
            b: m.b,
            c: m.c,
            d: m.d,
            e: m.e.px().in_pt(),
            f: m.f.px().in_pt(),
        },
        Translate(x, y) => {
            Affine2D::translation(resolve_length_component(x, w), resolve_length_component(y, h))
        }
        TranslateX(x) => {
            Affine2D::translation(resolve_length_component(x, w), crate::units::Pt::ZERO)
        }
        TranslateY(y) => {
            Affine2D::translation(crate::units::Pt::ZERO, resolve_length_component(y, h))
        }
```

**Step 5: Remove now-unused `Length` import from `op_to_matrix`**

`op_to_matrix` no longer calls `Length::new` directly (moved into the helper), but still needs `F32Units` for `m.e.px()`/`m.f.px()`. Delete only this line from the top of `op_to_matrix` (currently line 2935):
```rust
    use style::values::computed::Length;
```
Keep `use crate::units::F32Units;` (still used by the `Matrix` arm).

**Step 6: Build and run the targeted tests**

Run: `cargo build -p fulgur 2>&1 | tail -20`
Expected: builds clean, no unused-import warnings (confirms Steps 3 and 5 removed the right imports).

Run: `cargo test -p fulgur --test transform_integration 2>&1 | tail -30`
Expected: all tests pass, including the 5 that were failing after Task 1 Step 7.

**Step 7: Run the full crate test suite**

Run: `cargo test -p fulgur 2>&1 | tail -60`
Expected: all tests pass (no other test touches `compute_transform`/`op_to_matrix`, but this confirms no incidental breakage elsewhere in the crate).

**Step 8: Run VRT and examples_determinism to confirm zero incidental impact**

Run: `cargo test -p fulgur-vrt 2>&1 | tail -40`
Run: `cargo test -p fulgur-cli --test examples_determinism 2>&1 | tail -40`
Expected: both pass with **no** `FULGUR_VRT_UPDATE` needed — confirmed during design that no VRT golden or example uses `transform`, so this is a zero-diff double-check, not a re-blessing step.

**Step 9: Lint and format**

Run: `cargo clippy -p fulgur --all-targets -- -D warnings 2>&1 | tail -40`
Run: `cargo fmt --check 2>&1 | tail -40`
Expected: both clean. If `cargo fmt` reports diffs, run `cargo fmt` (no `--check`) and re-diff only `blitz_adapter.rs`.

**Step 10: Commit the implementation**

```bash
git add crates/fulgur/src/blitz_adapter.rs
git commit -m "fix: fold px->pt for absolute transform translate/matrix/origin (fulgur-9vw5)"
```

---

### Task 3: Close out

**Step 1: Verify the acceptance criteria from the beads issue**

Run: `bd show fulgur-9vw5` and confirm every bullet in the `ACCEPTANCE` field is satisfied by Tasks 1-2's test runs above.

**Step 2: Final full-suite run**

Run: `cargo test --workspace 2>&1 | tail -80`
Expected: all green. This is the last check before handing off to `superpowers:verification-before-completion` / `superpowers:finishing-a-development-branch`.
