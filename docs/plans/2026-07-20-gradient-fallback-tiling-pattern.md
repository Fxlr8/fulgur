# Gradient Fallback Tiling Pattern Implementation Plan

**Goal:** `background.rs` の gradient (linear/radial/conic) の per-tile fallback path を、完全行の矩形部分を 1 Tiling Pattern で描き、残余の部分行のみ per-tile ループに落とす形に置換して、`background-size: 1.33px + background-repeat: repeat` 攻撃で 425MB PDF/1GB RSS を生む OOM を bounded に抑える。

**Architecture:** `try_uniform_grid` が None を返す truncated-grid ケース (MAX_TILES=10000 mid-row 切詰) は、実質「完全行の uniform grid + 最終部分行」の形。新規 pure helper `split_truncated_grid` を追加してこの分解を行い、既存の `draw_gradient_tiling_pattern` を full 部分に、per-tile ループを remainder のみに適用する。linear / radial / conic の 3 arm に同じ pattern を適用。

**Tech Stack:** Rust, krilla (PDF paint / Tiling Pattern), fulgur render pipeline (`background.rs`, `render_smoke.rs`).

**Beads issue:** `fulgur-imk7` (P1、Codex `01f477e4` の実測 fix、conic `6aa94631` 兄弟含む)

**Related code (touch list):**

- Modify: `crates/fulgur/src/background.rs:646-748` (linear/radial/conic fallback arms)
- Modify: `crates/fulgur/src/background.rs:2057-2126` (nearby `try_uniform_grid` — helper 追加場所)
- Test: `crates/fulgur/src/background.rs` の `#[cfg(test)] mod tests` (split_truncated_grid unit tests)
- Test: `crates/fulgur/tests/render_smoke.rs` (exploit smoke test)
- 影響 VRT: `crates/fulgur-vrt/goldens/fulgur/` の gradient 系がある場合 `FULGUR_VRT_UPDATE=1` で再生成 (Pattern 使用で視覚等価だが PDF byte 差が出る可能性)

**Preconditions:**

- worktree: `/home/ubuntu/fulgur/.worktrees/fulgur-imk7`
- branch: `harden/fulgur-imk7-gradient-fallback-pattern`
- baseline: `cargo test --lib -p fulgur` = 1730 passed / 0 failed

---

## Task 1: Add SplitGrid struct + failing test skeleton for empty/degenerate cases

**Files:**

- Modify: `crates/fulgur/src/background.rs` (near `:2057` `try_uniform_grid`)

**Step 1: Write the failing test (degenerate cases)**

Add to `#[cfg(test)] mod tests` in `background.rs`:

```rust
#[test]
fn split_truncated_grid_empty_returns_none_and_all_as_remainder() {
    let tiles: Vec<(f32, f32, f32, f32)> = vec![];
    let split = split_truncated_grid(&tiles);
    assert!(split.full.is_none());
    assert_eq!(split.remainder.len(), 0);
}

#[test]
fn split_truncated_grid_single_tile_returns_none_and_tile_as_remainder() {
    let tiles = vec![(0.0, 0.0, 10.0, 10.0)];
    let split = split_truncated_grid(&tiles);
    assert!(split.full.is_none());
    assert_eq!(split.remainder, tiles.as_slice());
}

#[test]
fn split_truncated_grid_single_row_returns_none_and_row_as_remainder() {
    // Single row is either fast-path caught by try_uniform_grid or irregular;
    // split_truncated_grid does not attempt to split a single row.
    let tiles = vec![
        (0.0, 0.0, 5.0, 5.0),
        (5.0, 0.0, 5.0, 5.0),
        (10.0, 0.0, 5.0, 5.0),
    ];
    let split = split_truncated_grid(&tiles);
    assert!(split.full.is_none());
    assert_eq!(split.remainder, tiles.as_slice());
}
```

**Step 2: Verify tests fail (function undefined)**

```bash
cd /home/ubuntu/fulgur/.worktrees/fulgur-imk7
cargo test -p fulgur --lib background::tests::split_truncated_grid 2>&1 | tail -20
```

Expected: compile error `cannot find function split_truncated_grid` / `cannot find type SplitGrid`.

**Step 3: Add minimal `SplitGrid` + `split_truncated_grid` stub returning `None/all remainder`**

```rust
/// Result of splitting a truncated (`MAX_TILES` mid-row cut) tile grid into
/// a leading uniform-grid rectangle and a trailing partial-row remainder.
///
/// The full rectangle is emitted as a single Tiling Pattern (cheap) and
/// the remainder as per-tile draws. Degenerate inputs (empty, single tile,
/// single row) return `full=None, remainder=all` so callers fall through
/// to the pre-existing per-tile path.
#[derive(Debug)]
struct SplitGrid<'a> {
    full: Option<UniformGrid>,
    remainder: &'a [(f32, f32, f32, f32)],
}

fn split_truncated_grid(tiles: &[(f32, f32, f32, f32)]) -> SplitGrid<'_> {
    // Minimal stub: everything as remainder.
    SplitGrid {
        full: None,
        remainder: tiles,
    }
}
```

Place near `try_uniform_grid` (`:2057`).

**Step 4: Verify tests pass**

```bash
cargo test -p fulgur --lib background::tests::split_truncated_grid 2>&1 | tail -10
```

Expected: 3 passed.

**Step 5: Commit**

```bash
git add crates/fulgur/src/background.rs
git commit -m "test(gradient): add SplitGrid skeleton + degenerate-case tests (fulgur-imk7)"
```

---

## Task 2: Truncated-grid detection (row-major length inspection)

**Files:**

- Modify: `crates/fulgur/src/background.rs` (extend `split_truncated_grid`)

**Step 1: Write failing tests for the truncated-grid split**

Add to `mod tests`:

```rust
#[test]
fn split_truncated_grid_two_full_rows_plus_partial_returns_full_and_remainder() {
    // 3 cols × 2 full rows + 1 partial row of 2 tiles
    // grid tile size 5×5, step 5
    let tiles = vec![
        (0.0, 0.0, 5.0, 5.0), (5.0, 0.0, 5.0, 5.0), (10.0, 0.0, 5.0, 5.0),
        (0.0, 5.0, 5.0, 5.0), (5.0, 5.0, 5.0, 5.0), (10.0, 5.0, 5.0, 5.0),
        (0.0, 10.0, 5.0, 5.0), (5.0, 10.0, 5.0, 5.0),
    ];
    let split = split_truncated_grid(&tiles);
    let grid = split.full.expect("full grid must be detected");
    assert_eq!(grid.count, (3, 2));
    assert_eq!(grid.cell, (5.0, 5.0));
    assert_eq!(grid.step, (5.0, 5.0));
    assert_eq!(grid.origin, (0.0, 0.0));
    assert_eq!(split.remainder.len(), 2);
    assert_eq!(split.remainder[0], (0.0, 10.0, 5.0, 5.0));
    assert_eq!(split.remainder[1], (5.0, 10.0, 5.0, 5.0));
}

#[test]
fn split_truncated_grid_all_rows_same_length_returns_none() {
    // Complete uniform grid — should have been caught by try_uniform_grid.
    // split_truncated_grid returns None so caller falls through.
    let tiles = vec![
        (0.0, 0.0, 5.0, 5.0), (5.0, 0.0, 5.0, 5.0),
        (0.0, 5.0, 5.0, 5.0), (5.0, 5.0, 5.0, 5.0),
    ];
    let split = split_truncated_grid(&tiles);
    assert!(split.full.is_none(),
        "complete grid should be fast-path-caught elsewhere; split returns None");
}

#[test]
fn split_truncated_grid_mismatched_cell_sizes_returns_none() {
    // Non-uniform cell size — cannot be a truncated grid.
    let tiles = vec![
        (0.0, 0.0, 5.0, 5.0),
        (5.0, 0.0, 3.0, 5.0),
        (0.0, 5.0, 5.0, 5.0),
    ];
    let split = split_truncated_grid(&tiles);
    assert!(split.full.is_none());
    assert_eq!(split.remainder, tiles.as_slice());
}

#[test]
fn split_truncated_grid_max_tiles_shape_100x16_plus_partial() {
    // Emulate the MAX_TILES=10000 truncation shape used by the exploit:
    // 100 cols × 16 full rows + 1 partial row (400 tiles) = 10000 total.
    let mut tiles = Vec::with_capacity(10_000);
    for r in 0..16 {
        for c in 0..100 {
            tiles.push((c as f32, r as f32, 1.0, 1.0));
        }
    }
    for c in 0..400 {
        tiles.push((c as f32, 16.0, 1.0, 1.0));
    }
    assert_eq!(tiles.len(), 2000);
    let split = split_truncated_grid(&tiles);
    let grid = split.full.expect("full rectangle must be detected");
    assert_eq!(grid.count, (100, 16));
    assert_eq!(split.remainder.len(), 400);
}
```

**Step 2: Verify tests fail**

```bash
cargo test -p fulgur --lib background::tests::split_truncated_grid 2>&1 | tail -20
```

Expected: the new tests fail (`Some(...)` expected but `None` returned).

**Step 3: Implement the detection**

Replace the `split_truncated_grid` stub body with:

```rust
fn split_truncated_grid(tiles: &[(f32, f32, f32, f32)]) -> SplitGrid<'_> {
    // Fewer than one row-worth of tiles cannot be split.
    if tiles.len() < 2 {
        return SplitGrid { full: None, remainder: tiles };
    }
    let eps = 1e-3_f32;

    // Cell size must be uniform across the whole tile set.
    let (tw0, th0) = (tiles[0].2, tiles[0].3);
    if !tiles
        .iter()
        .all(|t| (t.2 - tw0).abs() < eps && (t.3 - th0).abs() < eps)
    {
        return SplitGrid { full: None, remainder: tiles };
    }

    // Row-major traversal: identify per-row tile counts by y-coordinate
    // blocks. `compute_tile_positions_slow` (background.rs:2018-2035)
    // generates tiles outer=y / inner=x, so consecutive entries with
    // matching y form a row.
    let mut row_starts: Vec<usize> = Vec::new();
    row_starts.push(0);
    let mut cur_y = tiles[0].1;
    for (i, t) in tiles.iter().enumerate().skip(1) {
        if (t.1 - cur_y).abs() >= eps {
            row_starts.push(i);
            cur_y = t.1;
        }
    }
    // Sentinel end index for length computation.
    let mut row_ends = row_starts.clone();
    row_ends.remove(0);
    row_ends.push(tiles.len());
    let row_lens: Vec<usize> = row_starts
        .iter()
        .zip(row_ends.iter())
        .map(|(&s, &e)| e - s)
        .collect();

    // Single row cannot be split by row-length prefix.
    if row_lens.len() < 2 {
        return SplitGrid { full: None, remainder: tiles };
    }

    // Count leading rows whose length matches the first row.
    let full_row_len = row_lens[0];
    let full_row_count = row_lens
        .iter()
        .take_while(|&&len| len == full_row_len)
        .count();

    // All rows equal length → complete grid. Should be caught by
    // try_uniform_grid; return None so caller does not double-emit.
    if full_row_count == row_lens.len() {
        return SplitGrid { full: None, remainder: tiles };
    }

    let full_tile_count = full_row_count * full_row_len;
    let full_tiles = &tiles[..full_tile_count];
    let remainder = &tiles[full_tile_count..];

    // Delegate uniform-grid validation (steps, x-positions) to the
    // existing helper — the truncated leading rectangle must itself
    // be a well-formed uniform grid for Pattern emission to be sound.
    match try_uniform_grid(full_tiles) {
        Some(grid) => SplitGrid { full: Some(grid), remainder },
        None => SplitGrid { full: None, remainder: tiles },
    }
}
```

**Step 4: Verify tests pass**

```bash
cargo test -p fulgur --lib background::tests::split_truncated_grid 2>&1 | tail -10
```

Expected: 7 passed.

**Step 5: Commit**

```bash
git add crates/fulgur/src/background.rs
git commit -m "feat(gradient): detect truncated-grid split point via row-length prefix (fulgur-imk7)"
```

---

## Task 3: Linear gradient fallback — Pattern-first refactor

**Files:**

- Modify: `crates/fulgur/src/background.rs:609-675` (linear gradient arm)

**Step 1: Write failing smoke test for linear-gradient bound**

Add to `crates/fulgur/tests/render_smoke.rs`:

```rust
#[test]
fn linear_gradient_nonuniform_tile_repeat_is_bounded() {
    // Attack shape: 1.33px × 1.33px tile with repeat + capped-large stop count.
    // Before the fix this produces a ~425MB PDF from 300B HTML; the
    // MAX_GRADIENT_STOPS=256 cap × 10_000 MAX_TILES fallback expansion.
    // After the fix, the truncated grid's leading rectangle is emitted
    // as a single Tiling Pattern; only the partial remainder row draws
    // per-tile.
    let mut stops = String::new();
    for i in 0..300 {
        if i > 0 { stops.push_str(", "); }
        let pct = (i as f32) * 100.0 / 299.0;
        let color = if i % 2 == 0 { "red" } else { "blue" };
        stops.push_str(&format!("{color} {pct:.4}%"));
    }
    let html = format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{{margin:0;padding:0}}
body{{background-size:1.33px 1.33px;background-repeat:repeat;height:1130px;
     background-image:linear-gradient(45deg,{stops})}}
</style></head><body></body></html>"#
    );
    let engine = fulgur::Engine::builder().build();
    let pdf = engine.render(&html).expect("render must succeed");
    // Pre-fix: ~425_000_000 bytes. Post-fix budget: 5 MB (comfortable
    // headroom above the ~KB expected for a Pattern-emitted layer).
    assert!(
        pdf.len() < 5_000_000,
        "linear gradient with tiny repeat + many stops must be bounded, got {} bytes",
        pdf.len()
    );
    assert!(!pdf.is_empty());
}
```

**Step 2: Verify test fails**

```bash
cargo test -p fulgur --test render_smoke linear_gradient_nonuniform_tile_repeat_is_bounded 2>&1 | tail -10
```

Expected: FAIL — actual `pdf.len() >> 5_000_000`.

**Step 3: Refactor the linear-gradient arm**

Locate `background.rs:609-675` (`BgImageContent::LinearGradient { .. }` arm). Replace the current `try_uniform_grid → Pattern | else per-tile loop` shape with:

```rust
BgImageContent::LinearGradient {
    direction,
    stops,
    repeating,
} => {
    // Try full-uniform first (unchanged fast path).
    let split = if let Some(grid) = try_uniform_grid(&tiles) {
        SplitGrid { full: Some(grid), remainder: &[] }
    } else {
        // Truncated-grid: emit the complete-row rectangle as one Pattern,
        // only the trailing partial row falls through to per-tile draws.
        // Bounds the 10_000 tile × 256 stop fallback that used to
        // produce ~425 MB PDFs from ~300 B HTML.
        split_truncated_grid(&tiles)
    };

    if let Some(grid) = split.full {
        let angle = match direction {
            crate::draw_primitives::LinearGradientDirection::Angle(a) => *a,
            crate::draw_primitives::LinearGradientDirection::Corner(corner) => {
                corner_to_angle_rad(*corner, grid.cell.0, grid.cell.1)
            }
        };
        draw_gradient_tiling_pattern(canvas, grid, |surface, _tw, _th| {
            draw_linear_gradient(
                surface, angle, stops, *repeating,
                0.0, 0.0, grid.cell.0, grid.cell.1,
            );
        });
    }

    // Per-tile fallback for the partial-row remainder (or the full tile
    // list if split declined). Corner direction needs per-tile angle
    // recomputation because it depends on tile aspect (CSS Images §3.1.1).
    match direction {
        crate::draw_primitives::LinearGradientDirection::Angle(a) => {
            let angle = *a;
            for (tx, ty, tw, th) in split.remainder {
                draw_linear_gradient(
                    canvas.surface, angle, stops, *repeating,
                    *tx, *ty, *tw, *th,
                );
            }
        }
        crate::draw_primitives::LinearGradientDirection::Corner(corner) => {
            for (tx, ty, tw, th) in split.remainder {
                let angle = corner_to_angle_rad(*corner, *tw, *th);
                draw_linear_gradient(
                    canvas.surface, angle, stops, *repeating,
                    *tx, *ty, *tw, *th,
                );
            }
        }
    }
}
```

**Step 4: Verify smoke test passes**

```bash
cargo test -p fulgur --test render_smoke linear_gradient_nonuniform_tile_repeat_is_bounded 2>&1 | tail -10
```

Expected: PASS. Also run neighbours to catch regressions:

```bash
cargo test -p fulgur --test render_smoke 2>&1 | tail -10
```

Expected: all render_smoke green.

**Step 5: Commit**

```bash
git add crates/fulgur/src/background.rs crates/fulgur/tests/render_smoke.rs
git commit -m "fix(gradient): fold linear-gradient truncated-grid fallback into single Pattern (fulgur-imk7)"
```

---

## Task 4: Radial gradient fallback — same treatment

**Files:**

- Modify: `crates/fulgur/src/background.rs:677-714` (radial gradient arm)

**Step 1: Write failing smoke test**

Add to `render_smoke.rs`:

```rust
#[test]
fn radial_gradient_nonuniform_tile_repeat_is_bounded() {
    let mut stops = String::new();
    for i in 0..300 {
        if i > 0 { stops.push_str(", "); }
        let pct = (i as f32) * 100.0 / 299.0;
        let color = if i % 2 == 0 { "red" } else { "blue" };
        stops.push_str(&format!("{color} {pct:.4}%"));
    }
    let html = format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{{margin:0;padding:0}}
body{{background-size:1.33px 1.33px;background-repeat:repeat;height:1130px;
     background-image:radial-gradient(circle at center,{stops})}}
</style></head><body></body></html>"#
    );
    let engine = fulgur::Engine::builder().build();
    let pdf = engine.render(&html).expect("render must succeed");
    assert!(pdf.len() < 5_000_000, "radial gradient bounded, got {} bytes", pdf.len());
}
```

**Step 2: Verify FAIL**

```bash
cargo test -p fulgur --test render_smoke radial_gradient_nonuniform_tile_repeat_is_bounded 2>&1 | tail -10
```

**Step 3: Refactor the radial-gradient arm**

Same shape as Task 3, replacing the `RadialGradient` arm. Radial does not have a per-tile angle recomputation branch — the `Some(grid)` path draws once and the remainder is a single loop:

```rust
BgImageContent::RadialGradient {
    shape,
    size,
    position_x,
    position_y,
    stops,
    repeating,
} => {
    let split = if let Some(grid) = try_uniform_grid(&tiles) {
        SplitGrid { full: Some(grid), remainder: &[] }
    } else {
        split_truncated_grid(&tiles)
    };
    if let Some(grid) = split.full {
        draw_gradient_tiling_pattern(canvas, grid, |surface, tw, th| {
            draw_radial_gradient(
                surface, *shape, size, position_x, position_y, stops, *repeating,
                0.0, 0.0, tw, th,
            );
        });
    }
    for (tx, ty, tw, th) in split.remainder {
        draw_radial_gradient(
            canvas.surface, *shape, size, position_x, position_y, stops, *repeating,
            *tx, *ty, *tw, *th,
        );
    }
}
```

**Step 4: Verify PASS**

```bash
cargo test -p fulgur --test render_smoke radial_gradient_nonuniform_tile_repeat_is_bounded 2>&1 | tail -10
cargo test -p fulgur --test render_smoke 2>&1 | tail -10
```

**Step 5: Commit**

```bash
git add crates/fulgur/src/background.rs crates/fulgur/tests/render_smoke.rs
git commit -m "fix(gradient): fold radial-gradient truncated-grid fallback into single Pattern (fulgur-imk7)"
```

---

## Task 5: Conic gradient fallback — same treatment

**Files:**

- Modify: `crates/fulgur/src/background.rs:715-751` (conic gradient arm)

**Step 1: Write failing smoke test**

Add to `render_smoke.rs`:

```rust
#[test]
fn conic_gradient_nonuniform_tile_repeat_is_bounded() {
    let html = r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;padding:0}
body{background-size:1.33px 1.33px;background-repeat:repeat;height:1130px;
     background-image:conic-gradient(from 0deg,red,yellow,green,blue,red)}
</style></head><body></body></html>"#;
    let engine = fulgur::Engine::builder().build();
    let pdf = engine.render(html).expect("render must succeed");
    // Pre-fix: ~46 MB. Post-fix budget: 5 MB (Pattern emits one wedge
    // set for the leading rectangle).
    assert!(pdf.len() < 5_000_000, "conic gradient bounded, got {} bytes", pdf.len());
}
```

**Step 2: Verify FAIL**

```bash
cargo test -p fulgur --test render_smoke conic_gradient_nonuniform_tile_repeat_is_bounded 2>&1 | tail -10
```

**Step 3: Refactor the conic-gradient arm**

Same pattern as radial:

```rust
BgImageContent::ConicGradient {
    from_angle,
    position_x,
    position_y,
    stops,
    repeating,
} => {
    let split = if let Some(grid) = try_uniform_grid(&tiles) {
        SplitGrid { full: Some(grid), remainder: &[] }
    } else {
        split_truncated_grid(&tiles)
    };
    if let Some(grid) = split.full {
        draw_gradient_tiling_pattern(canvas, grid, |surface, tw, th| {
            draw_conic_gradient(
                surface, *from_angle, position_x, position_y, stops, *repeating,
                0.0, 0.0, tw, th,
            );
        });
    }
    for (tx, ty, tw, th) in split.remainder {
        draw_conic_gradient(
            canvas.surface, *from_angle, position_x, position_y, stops, *repeating,
            *tx, *ty, *tw, *th,
        );
    }
}
```

(Argument list matches the existing per-tile call at `:735-748`.)

**Step 4: Verify PASS**

```bash
cargo test -p fulgur --test render_smoke conic_gradient_nonuniform_tile_repeat_is_bounded 2>&1 | tail -10
cargo test -p fulgur --test render_smoke 2>&1 | tail -10
```

**Step 5: Commit**

```bash
git add crates/fulgur/src/background.rs crates/fulgur/tests/render_smoke.rs
git commit -m "fix(gradient): fold conic-gradient truncated-grid fallback into single Pattern (fulgur-imk7)"
```

---

## Task 6: Full verification (lib / clippy / fmt / VRT)

**Step 1: Full lib test**

```bash
cargo test -p fulgur --lib 2>&1 | tail -5
```

Expected: 1730+ passed (new: split_truncated_grid tests). 0 failed.

**Step 2: Full render_smoke test**

```bash
cargo test -p fulgur --test render_smoke 2>&1 | tail -5
```

Expected: existing tests + 3 new gradient tests all pass.

**Step 3: fmt / clippy**

```bash
cargo fmt --all --check
cargo clippy -p fulgur --all-targets -- -D warnings
```

Expected: no output / no warnings.

**Step 4: VRT (gradient goldens may need update)**

Run VRT to see if any golden diverges:

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" \
  cargo test -p fulgur-vrt 2>&1 | tail -20
```

If gradient VRT fails:

- Inspect the pdftocairo diff images the harness produces.
- If visually identical (only PDF byte structure differs), regenerate:

  ```bash
  FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" \
    FULGUR_VRT_UPDATE=1 cargo test -p fulgur-vrt 2>&1 | tail -20
  ```

- Review the golden diff (`git diff crates/fulgur-vrt/goldens/`) — should be gradient-only.

**Step 5: Re-run exploit measurement (out-of-tree, sanity)**

```bash
cargo build --release --bin fulgur 2>&1 | tail -3
SP=/tmp/claude-1000/-home-ubuntu-fulgur/6c60686c-6e08-4b4b-ab1b-08657e915a44/scratchpad/gradient-dos
BIN=/home/ubuntu/fulgur/.worktrees/fulgur-imk7/target/release/fulgur
export FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf"
for name in attack_nonuniform attack_stops attack_2layers_distinct attack_conic_nonuniform; do
  t0=$(date +%s.%N)
  /usr/bin/time -v "$BIN" render "$SP/$name.html" -o "/tmp/${name}_after.pdf" >/dev/null 2>"/tmp/${name}_after.time"
  t1=$(date +%s.%N)
  real=$(awk 'BEGIN{printf "%.3f", '$t1'-'$t0'}')
  rss=$(grep -Po 'Maximum resident set size \(kbytes\): \K\d+' "/tmp/${name}_after.time")
  size=$(stat -c %s "/tmp/${name}_after.pdf")
  printf "%-32s  time=%7ss  rss=%9sKB  pdf=%12s bytes\n" "$name" "$real" "$rss" "$size"
done
```

Expected: `attack_stops` (previously 2.17s / 975MB / 425MB) drops to < 200ms / < 100MB / < 5MB. `attack_2layers_distinct` proportional. `attack_conic_nonuniform` drops from 46MB to KB range.

**Step 6: Commit any golden updates**

```bash
git status
git add crates/fulgur-vrt/goldens/
git commit -m "test(vrt): update gradient goldens after truncated-grid Pattern fold (fulgur-imk7)"
```

---

## Rollback

Each task is a single commit; revert with `git revert <sha>` if a specific arm regresses. The `split_truncated_grid` helper is pure and covered by unit tests; reverting its callers restores the pre-fix per-tile fallback exactly.

## Non-goals

- **Document-wide layer/element cap** for distinct-layer amplification (Scenario #6, 872MB with 2 distinct layers): out of scope. Per-layer fix here reduces the coefficient by ~10⁵×; the multiplicative residual is filed for a separate broad-hardening pass.
- **`MAX_GRADIENT_STOPS` change**: already effective, unchanged.
- **`fast path` (`try_uniform_grid` on complete grid)**: unchanged, only fallback touched.
