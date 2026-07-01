# release-plz Base Setup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `release-plz.toml` configuring the 7-crate fulgur workspace (publish control, version-group lockstep, exclusions) and validate it locally with `release-plz update`, without performing a real version bump or touching CI.

**Architecture:** `release-plz.toml` lives at the workspace root next to `Cargo.toml`. `[workspace]` sets shared defaults; five `[[package]]` blocks override per-crate `publish`/`version_group`, and two set `release = false` to fully exclude test/fixture crates. Validation runs `release-plz update` (the only local, no-network preview command — it has no `--dry-run` flag and writes files directly) inside this worktree, then the resulting Cargo.toml/Cargo.lock/CHANGELOG.md edits are inspected and reverted so only `release-plz.toml` itself is committed.

**Tech Stack:** release-plz CLI (Rust, installed via `cargo install`), TOML config.

**Scope boundary:** This plan does NOT wire release-plz into CI (fulgur-f7o2), does NOT decide the changelog source (fulgur-b0nb), and does NOT reconcile release-plz's bump detection with the ZeroVer minor-fixed policy (fulgur-q7mc). The dry-run is expected to propose a non-ZeroVer-compliant bump — that's a documented finding for q7mc, not something to fix here.

---

### Task 1: Write `release-plz.toml`

**Files:**

- Create: `release-plz.toml` (workspace root, alongside `Cargo.toml`)

**Background — verified facts (do not re-derive, just use):**

Root `Cargo.toml` workspace members (`Cargo.toml:3`):
`crates/fulgur`, `crates/fulgur-cli`, `crates/fulgur-ruby`, `crates/fulgur-vrt`, `crates/fulgur-wasm`, `crates/fulgur-wpt`, `crates/pyfulgur`

Per-crate `publish` field as it stands today in each `Cargo.toml`:

| Crate | `publish` today | Role |
|---|---|---|
| `fulgur` | unset (defaults to publishable) | crates.io, version-synced |
| `fulgur-cli` | unset (defaults to publishable) | crates.io, version-synced |
| `fulgur-wasm` | `false` | not on crates.io, version-synced |
| `pyfulgur` | `false` | not on crates.io, version-synced |
| `fulgur-ruby` | `false` | not on crates.io, version-synced |
| `fulgur-vrt` | `false`, version `0.0.0` | excluded entirely |
| `fulgur-wpt` | `false`, version `0.0.0` | excluded entirely |

release-plz errors if `release-plz.toml` sets `publish = true` for a package whose own `Cargo.toml` has `publish = false` — so the three binding crates must have `publish = false` in `release-plz.toml` too, not be left at a conflicting default.

**Step 1: Write the file**

```toml
[workspace]
release = true

[[package]]
name = "fulgur"
version_group = "core"
publish = true

[[package]]
name = "fulgur-cli"
version_group = "core"
publish = true

[[package]]
name = "fulgur-wasm"
version_group = "core"
publish = false

[[package]]
name = "pyfulgur"
version_group = "core"
publish = false

[[package]]
name = "fulgur-ruby"
version_group = "core"
publish = false

[[package]]
name = "fulgur-vrt"
release = false

[[package]]
name = "fulgur-wpt"
release = false
```

**Step 2: Sanity-check the TOML parses**

Run: `python3 -c "import tomllib; tomllib.load(open('release-plz.toml','rb')); print('ok')"`
Expected: `ok`

(`tomllib` is stdlib on Python ≥3.11; if unavailable, `cargo install --quiet toml-test-or-similar` is overkill — just eyeball it or let Step 1 of Task 2 below be the real validator, since `release-plz update` will fail loudly on malformed TOML.)

Note: this check only validates TOML *syntax* — it does not validate that the keys/structure match release-plz's own config schema. release-plz's `Config` struct uses `#[serde(deny_unknown_fields)]` and accepts only `workspace`, `changelog`, and `package` as top-level keys, so a syntactically-valid file can still be rejected by release-plz itself (e.g. an unrecognized top-level key like `$schema` would fail to load even though `tomllib` parses it fine). That schema-level check only happens when Task 2 actually runs `release-plz update`.

**Step 3: Commit is deferred**

Do not commit yet — Task 2's validation run needs `release-plz.toml` present but uncommitted is fine. We commit once validation confirms the config is correct (end of Task 2).

---

### Task 2: Install release-plz and validate via local dry-run

**Files:**

- Modify (temporarily, then reverted): `crates/*/Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` (whichever `release-plz update` touches)
- Read: output of `release-plz update`

**Step 1: Install the CLI**

Run: `cargo install release-plz --locked`
Expected: builds and installs; `release-plz --version` then prints a `0.3.x` version string. This compiles from source and may take a few minutes — run with a generous timeout or in the background.

**Step 2: Confirm the working tree is otherwise clean before running update**

Run: `git status --short`
Expected: only `release-plz.toml` shows as untracked (`??`). If anything else is dirty, stop and figure out why before proceeding — `release-plz update` needs a clean baseline to produce a readable diff.

**Step 3: Run the local preview**

Run: `release-plz update`
Expected: it exits 0 and reports version bumps for `fulgur`, `fulgur-cli`, `fulgur-wasm`, `pyfulgur`, `fulgur-ruby` (same target version across all five, since they share `version_group = "core"`), and does **not** touch `fulgur-vrt` / `fulgur-wpt`.

**Step 4: Inspect what changed**

Run: `git status --short && git diff --stat`
Expected: `Cargo.lock`, the five lockstep crates' `Cargo.toml` (version bump), and likely a `CHANGELOG.md` per publishable crate (release-plz's own changelog generator, independent of the current PR-based one — this divergence is expected and is b0nb's decision point, not something to reconcile here).

Look specifically for:

- Did `fulgur-vrt`/`fulgur-wpt` get left alone? (must be yes — confirms `release = false` worked)
- Did the five lockstep crates land on the *same* version number? (must be yes — confirms `version_group` worked)
- What bump size did it choose (patch/minor/major)? Record this verbatim — it's very likely **not** the ZeroVer "always minor" the project wants, since release-plz infers bump size from conventional-commit parsing / semver-check, and this repo doesn't reliably use conventional commit prefixes. That mismatch is the exact gap fulgur-q7mc exists to close.

**Step 5: Revert the file mutations, keep only the config**

Run: `git checkout -- Cargo.lock crates/fulgur/Cargo.toml crates/fulgur-cli/Cargo.toml crates/fulgur-wasm/Cargo.toml crates/pyfulgur/Cargo.toml crates/fulgur-ruby/Cargo.toml`

Then check for any `CHANGELOG.md` files `release-plz update` created or modified (`git status --short`) and revert those too the same way (`git checkout -- <path>` for tracked files it modified, `rm` for any new ones it created from scratch). Re-run `git status --short` afterward.

Expected: only `release-plz.toml` remains as an untracked addition; everything else matches `main`.

**Step 6: Commit**

```bash
git add release-plz.toml
git commit -m "chore(release): add release-plz.toml for workspace base config"
```

**Step 7: Record dry-run findings in the beads issue**

Append the Step 4 observations (bump size chosen, confirmation that vrt/wpt were excluded, confirmation that the 5 crates landed on the same version, any surprises e.g. semver_check output) to `fulgur-7b3l`'s notes:

```bash
bd update fulgur-7b3l --append-notes "release-plz update dry-run 結果: <observed bump size>, vrt/wpt 除外 <確認結果>, 5 crate lockstep <確認結果>, semver_check 出力 <あれば>"
```

Use single-quoted content or a file if the findings text contains backticks or `${...}` (these go through eval — see project memory `feedback_bd_description_quoting`).

---

### Verification

- `release-plz.toml` exists at the workspace root and is the only change in the final commit (`git show --stat HEAD`).
- `cargo build --workspace` still succeeds (config addition must not affect the Rust build).
- `fulgur-7b3l` notes contain the recorded dry-run findings.

---

### Addendum (post-execution correction)

Task 1's Step 1 snippet, as originally written, does **not** achieve the 5-crate
lockstep Task 2 Step 3/4 expects. The actual dry-run (`release-plz update`)
showed only `fulgur`/`fulgur-cli` proposed for a bump — `fulgur-wasm`/
`pyfulgur`/`fulgur-ruby` silently dropped out of `version_group = "core"`,
because release-plz filters packages by their own `Cargo.toml` `publish` field
*before* applying `version_group`, and these three have `publish = false`.

The fix, applied as a follow-up commit rather than amending Task 1's snippet
in place: add `git_only = true` to the three non-publishing crates'
`[[package]]` blocks. This tells release-plz to resolve their "current
version" from git tags instead of the crates.io registry, which keeps them
inside the publishable set so `version_group` can lock them to the rest.
Verified empirically: all 5 crates landed on the same proposed version after
the fix.

Caveat carried forward (not resolved by this task): the binding crates have
no `<crate>-vX.Y.Z`-style git tags in this repo's history (only generic
`vX.Y.Z` workspace tags), so `git_only`'s tag-based baseline treated them as
"initial release" before `version_group` force-aligned them. This was only
exercised through `release-plz update` (the local calculation phase) — the
real `release-plz release` (tagging/publish) phase is unverified against this
tag scheme. That's a question for whichever issue ends up owning fulgur's
release tagging strategy (likely fulgur-f7o2), not something resolved here.

If you're copying the Task 1 TOML snippet for a similar setup elsewhere,
include the `git_only = true` lines from the start rather than reproducing
this two-step discovery.
