# P3a: Public-Reachability Gaps Implementation Plan

**Goal:** Make `column_css`'s public-facing types (`ColumnRuleSpec` and friends, already
reachable as *values* through `Drawables.multicol_rules`) nameable from outside the crate,
without leaking the CSS parser's internal implementation types, and document the `usvg`
version-pin requirement for `SvgEntry.tree` consumers. This unblocks `fulgur-2map.10`
(public `Engine::layout()`).

**Architecture:** `column_css` becomes a `pub mod` (was `pub(crate) mod`). The 10
CSS-parser-internal items inside it (`ColumnStyleTable`, `StyleRule`, `SimpleSelector`,
`CompoundSelector`, `ComplexSelector`, and 5 parse/match functions) are downgraded from
`pub` to `pub(crate)` in the same change, so only the 5 already-Drawables-reachable data
types (`ColumnRuleSpec`, `ColumnStyleProps`, `ColumnRuleStyle`, `ColumnFill`, `PageName`)
become genuinely externally nameable. Verified empirically in a throwaway trial build: this
does not regress `blitz_adapter::extract_column_style_table` (still builds warning-free) and
removes exactly the 10 pre-existing `unreachable_pub` warnings on `column_css` items (verified
via `RUSTFLAGS="-W unreachable_pub"`), leaving only 4 pre-existing, unrelated `net.rs`
warnings untouched.

**Tech Stack:** Rust, cargo, existing fulgur integration test conventions
(`crates/fulgur/tests/*.rs`).

---

## Task 1: Public-reachability regression test + visibility fix (TDD)

**Files:**

- Create: `crates/fulgur/tests/public_reachability.rs`
- Modify: `crates/fulgur/src/lib.rs:39`
- Modify: `crates/fulgur/src/column_css.rs` (10 items: lines 172, 177, 186, 201, 209, 530,
  730, 850, 894, 978 — line numbers as of this plan's writing; re-grep before editing since
  earlier edits in this task may shift later ones)

**Step 1: Write the failing test**

Create `crates/fulgur/tests/public_reachability.rs`:

```rust
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
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fulgur --test public_reachability 2>&1 | head -20`

Expected: compile error `E0603` — `` module `column_css` is private `` (because
`crates/fulgur/src/lib.rs:39` still says `pub(crate) mod column_css;`).

**Step 3: Implement the minimal visibility change**

In `crates/fulgur/src/lib.rs:39`, change:

```rust
pub(crate) mod column_css;
```

to:

```rust
pub mod column_css;
```

In `crates/fulgur/src/column_css.rs`, downgrade exactly these 10 declarations from `pub` to
`pub(crate)` (do not touch any other `pub` item in the file — the 5 data types
`ColumnRuleStyle`, `ColumnRuleSpec`, `ColumnFill`, `PageName`, `ColumnStyleProps` stay `pub`):

```rust
pub(crate) type ColumnStyleTable = BTreeMap<usize, ColumnStyleProps>;
pub(crate) struct StyleRule {
pub(crate) enum SimpleSelector {
pub(crate) struct CompoundSelector {
pub(crate) struct ComplexSelector {
pub(crate) fn parse_declaration_block(css: &str) -> ColumnStyleProps {
pub(crate) fn parse_selector_list(input: &str) -> Vec<ComplexSelector> {
pub(crate) fn matches_complex(
pub(crate) fn parse_stylesheet(source: &str) -> Vec<StyleRule> {
pub(crate) fn build_column_style_table(
```

(Only the declaration line's leading `pub` token changes to `pub(crate)`; struct/enum bodies,
fields, and function bodies are untouched.)

**Step 4: Run test to verify it passes**

Run: `cargo test -p fulgur --test public_reachability 2>&1 | tail -10`

Expected: `test column_css_public_types_are_externally_nameable ... ok`

**Step 5: Run the full lib test suite to check for fallout**

Run: `cargo test -p fulgur --lib 2>&1 | tail -10`

Expected: `1544 passed; 0 failed` (same count as the worktree baseline recorded before this
plan — if the count differs, stop and investigate before continuing).

**Step 6: Confirm no `unreachable_pub` regressions and no new warnings**

Run:

```bash
touch crates/fulgur/src/lib.rs
RUSTFLAGS="-W unreachable_pub" cargo build -p fulgur --message-format=short 2>&1 | grep -c warning
```

Expected: `5` (4 individual pre-existing `net.rs` warnings + 1 summary line — matches the
verified pre-fix-minus-column_css baseline; if it's still 15, the downgrade didn't take
effect somewhere).

**Step 7: Commit**

```bash
git add crates/fulgur/src/lib.rs crates/fulgur/src/column_css.rs crates/fulgur/tests/public_reachability.rs
git commit -m "fix(column_css): make module pub, keep parser internals pub(crate)

Drawables.multicol_rules already leaks ColumnRuleSpec (and 4 sibling
types) as values; column_css being pub(crate) made them unnameable by
external consumers. Make the module pub while downgrading the 10
CSS-parser-internal items to pub(crate) so only the already-reachable
data types become nameable.

fulgur-2map.9"
```

---

## Task 2: Document the usvg version-pin requirement on `SvgEntry.tree`

**Files:**

- Modify: `crates/fulgur/src/drawables.rs:115-117` (doc comment directly above the `SvgEntry`
  struct and/or above the `tree` field — re-check exact line numbers after Task 1's edits
  land, since this file itself is untouched by Task 1 but line numbers in this plan were
  captured before Task 1 ran)

**Step 1: Add the doc comment**

Current code (`crates/fulgur/src/drawables.rs`, immediately before `pub struct SvgEntry`):

```rust
/// SVG draw payload for v2. Mirrors the fields `SvgRender` holds.
#[derive(Debug, Clone)]
pub struct SvgEntry {
    pub tree: std::sync::Arc<usvg::Tree>,
```

Change to:

```rust
/// SVG draw payload for v2. Mirrors the fields `SvgRender` holds.
///
/// `tree` is `Arc<usvg::Tree>` — an external-crate type. Consumers that
/// construct, inspect, or pattern-match on `usvg::Tree` directly (rather
/// than treating it as opaque) must depend on the exact `usvg` version
/// series this crate resolves (see the `usvg` entry in this crate's
/// `Cargo.toml`); a mismatched `usvg` version will not type-unify even if
/// semver-compatible, since `usvg::Tree` is not part of `usvg`'s own
/// stability guarantees across our pinned range.
#[derive(Debug, Clone)]
pub struct SvgEntry {
    pub tree: std::sync::Arc<usvg::Tree>,
```

**Step 2: Verify the doc builds cleanly**

Run: `cargo doc -p fulgur --no-deps 2>&1 | tail -20`

Expected: no new warnings (no broken intra-doc links; the comment above uses plain text, not
`[...]` links, so this should be a no-op check).

**Step 3: Commit**

```bash
git add crates/fulgur/src/drawables.rs
git commit -m "docs(drawables): document usvg version-pin requirement on SvgEntry.tree

fulgur-2map.9"
```

---

## Task 3: Full verification pass

No code changes in this task — confirms the epic's byte-neutrality requirement and runs the
full acceptance-criteria checklist from the beads issue.

**Step 1: Build and lint**

```bash
cargo build -p fulgur 2>&1 | tail -10
cargo clippy -p fulgur --lib 2>&1 | tail -20
cargo fmt --check 2>&1 | tail -20
```

Expected: clean build, no new clippy warnings beyond pre-existing baseline, `fmt --check`
passes (no diff).

**Step 2: Full fulgur test suite**

```bash
cargo test -p fulgur 2>&1 | tail -20
```

Expected: all tests pass, including the new `public_reachability` integration test.

**Step 3: VRT golden byte-identity check**

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt 2>&1 | tail -30
```

Expected: all VRT reftests pass (no PDF byte diffs against `goldens/fulgur/**/*.pdf`) — this
is a pure visibility + doc-comment change, so no draw-path behavior changes and no golden
updates should be needed. If any VRT test fails, stop and investigate before proceeding (do
NOT run with `FULGUR_VRT_UPDATE=1` to paper over an unexpected diff).

**Step 4: CLI examples determinism check**

```bash
cargo test -p fulgur-cli --test examples_determinism 2>&1 | tail -30
```

Expected: passes, no byte diffs.

**Step 5: Report**

If all four checks pass, this plan's implementation is complete and matches every acceptance
criterion recorded on `fulgur-2map.9`. No commit needed for this task (verification-only,
unless `cargo fmt` step 1 found and needs fixing — if so, `git add -u && git commit -m "style: cargo fmt"`
before proceeding).

---

## Post-plan

Update the beads issue: `bd update fulgur-2map.9 --status=in_review` (or hand off per
`superpowers:finishing-a-development-branch`), and confirm the follow-up refactor issue
`fulgur-fx6v` (splitting `column_css.rs` into `types.rs` + `parser.rs`) remains filed and
non-blocking.
