# fulgur-pgbrk R3: page-overflow detection

**Goal:** Detect every fragment placed past the bottom of its page's content
strip, warn on it in production, and make it a blanket test invariant — so that
silent content loss becomes impossible to ship unnoticed, and so the remaining
pagination bugs (R1, R2) become testable in-process.

**Parent review:** [2026-08-16-fulgur-pgbrk-page-fragmentation-review.md](./2026-08-16-fulgur-pgbrk-page-fragmentation-review.md)
(item R3).

**Spec baseline:** [CSS Fragmentation Module Level 3](https://www.w3.org/TR/css-break-3/).

**Tech stack:** Rust, `crates/fulgur/src/pagination_layout.rs`.

---

## Why this goes first

R1–R7 in the parent review are not seven independent changes. They cluster by
which function signature they touch:

- **R1 + R2 + R6** all land in `fragment_inline_root` and its two duplicated
  call sites (`:741`, `:2061`). Done separately, that signature is edited three
  times and the same tests re-blessed three times.
- **R4 + R5** both need one thing: `fragment_block_subtree` returning a
  `SubtreeResult::RequestBreakBefore` channel instead of `(u32, f32)`.
- **R3** is orthogonal to both, and is the only item that closes the *class* of
  defect rather than an instance.

R3 is also test infrastructure, not just a diagnostic. Today `render_smoke`'s
pagination test hard-requires `pdftotext` (poppler), because `fulgur::inspect`
reports byte-identical output for a broken and a correct render — the lost
glyphs are in the content stream, merely painted outside the page box. A
predicate over the geometry table sees that difference in-process, which turns
every R1/R2 test from "shell out to poppler" into a cheap assertion.

Order: **R3 → (R1 + R2 + R6) → (R4 + R5)**, with the parent review's Risk 1
`break_decision` extraction folded into whichever bundle first touches
`would_split_block_subtree`.

---

## The predicate

One free function, next to `run_pass_inner`:

```rust
/// One fragment that was placed past the bottom of its page's content strip.
pub(crate) struct FragmentOverflow {
    pub node_id: usize,
    pub page_index: u32,
    /// px by which the fragment's bottom exceeds the content strip.
    pub overshoot_px: f32,
}

/// Every fragment whose bottom falls below the content strip, in
/// deterministic (node, page) order.
pub(crate) fn find_overflowing_fragments(
    table: &PaginationGeometryTable,
    page_height_px: f32,
) -> Vec<FragmentOverflow>
```

The test is `f.y + f.height > page_height_px + 0.5`, matching the epsilon
convention already used inline in this file.

`PaginationGeometryTable` is a `BTreeMap`, so iteration order is deterministic
and the result needs no sort. That matters because the warn text reaches
user-visible logs, and determinism is a project invariant (CLAUDE.md).

### Insertion point

The tail of `run_pass_inner`. All three public entry points — `run_pass`,
`run_pass_with_break_styles`, `run_pass_with_break_and_running` — funnel through
it, so the check reaches all 77 existing call sites with no signature change and
no call-site edits.

### Relationship to the existing test helper

`assert_no_fragment_starts_below_page` (`pagination_layout.rs:7377`) is rewritten
as a thin wrapper over `find_overflowing_fragments`. This is **not** a pure
rename: the old helper tests `f.y > page_h` (the fragment *starts* below the
strip), the new one tests `f.y + f.height > page_h` (the fragment *ends* below
it). The new predicate is strictly stronger and is expected to be where most new
test failures come from.

---

## Production behaviour

At the end of `run_pass_inner`, one `tracing::warn!` per record:

```rust
for o in find_overflowing_fragments(&table, page_height_px) {
    tracing::warn!(
        node_id = o.node_id,
        page = o.page_index,
        overshoot_px = o.overshoot_px,
        "fragment placed past the page content strip; content may be clipped",
    );
}
```

`tracing` writes to whatever subscriber the host installs and never touches fd 1
directly, satisfying CLAUDE.md's rule that `crates/fulgur` must not write to
stdout under any circumstance. A library consumer with no subscriber gets
silence; `fulgur-cli` already installs one.

No public API change, no `Engine` result field, no CLI flag. Those were
considered and deferred — see [Rejected alternatives](#rejected-alternatives).

---

## Test behaviour

The same loop, under `#[cfg(test)]`, panics instead of warning. This makes the
invariant blanket across every existing pagination test with zero per-test
edits.

The tradeoff, named explicitly: test and production builds diverge in behaviour
at this one point. Accepted, because the divergence is only "the same condition
is fatal instead of logged" — the ordinary shape of an invariant assertion.

### Known-failing fixtures

The check has true positives on day one. There is **no allowlist**. Failing
fixtures join the `#[ignore]` convention already established by the
`css_break3_*` block: an ignored test that fails under `--ignored` is a runnable
statement of an open gap. No new mechanism is introduced.

Known members of that set:

- Any R1 / R2 repro, until those land.
- `oversized_unbreakable_leading_leaf_at_page_top_emits_once` (`:7852`).

The second one needs more than an `#[ignore]`. It currently *asserts* that the
overflowing output is correct — a 900px probe on a 400px page emitting one
fragment at `y≈0` with `height=900`. Since R7 holds that the nested path should
slice like the body-direct path (fulgur-sbw2), that test's expectations must be
rewritten to expect slicing before being ignored. Ignoring it as-written would
pin the defect under a new name.

The size of the failing set is not measured here. It affects the size of the
cleanup, not the shape of this design, and the resolution is `#[ignore]`
regardless of the count.

---

## Testing

Per CLAUDE.md's coverage rule, this is lib-level logic and belongs in
`#[cfg(test)] mod tests` in `pagination_layout.rs`. No VRT fixture is needed —
the predicate is invisible to rendering.

Unit tests for `find_overflowing_fragments`:

- Empty table yields no records.
- A fragment ending exactly at `page_height_px` yields no record (epsilon
  boundary).
- A fragment ending `0.4px` past yields no record; `0.6px` past yields one.
- A fragment that *starts* below the strip is caught (the old helper's case).
- Multiple overflows across nodes and pages come back in deterministic order.
- `overshoot_px` is the bottom minus the strip height, not the fragment height.

---

## Rejected alternatives

**Two-tier detection (content strip vs. paper edge).** Considered reporting
overflow past the page box separately from overflow past the content strip, on
the theory that the first is silent content loss and the second is merely
overlap with the footer. Rejected: paper-edge overflow is a strict subset of
content-strip overflow, so the second predicate adds zero detection coverage.
Any severity distinction is a message field computed from an already-known
number, not a separate pass.

**Suppressing "spec-legal" monolithic overflow.** §4.1 permits monolithic
content taller than the fragmentainer to overflow. Rejected as a suppression
rule: the spec's other permitted option is slicing, and fulgur already slices in
the body-direct path. The overflow in the nested path is therefore a fulgur
inconsistency (R7), so the predicate firing there is a true positive, not noise.

**Allowlist of known-failing fixtures.** Rejected as unnecessary machinery.
`#[ignore]` already carries that meaning in this file.

**Per-test opt-in assertion.** Rejected: leaves existing coverage unchecked and
requires deciding, per test, whether the invariant applies — when it always
applies.

**`Engine` result count plus `fulgur-cli --strict-pagination`.** Deferred. It is
a public API commitment, and it should not be made before R1 and R2 have reduced
the population of real overflows a strict mode would trip on.

---

## Verification commands

```bash
cargo test -p fulgur --lib
cargo test -p fulgur --lib find_overflowing_fragments
cargo test -p fulgur --lib -- --ignored     # open gaps: expected to FAIL
cargo clippy -p fulgur && cargo fmt --check
npx markdownlint-cli2 '**/*.md'
```

VRT is not expected to move. Per the parent review, `cargo test -p fulgur-vrt`
shows 29/64 differing on macOS independent of this work — an environment/goldens
mismatch. Verify any change the same way (stash, re-run, diff the failing list
including byte sizes) before concluding it moved VRT.
