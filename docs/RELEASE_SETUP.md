# Release Setup: Trusted Publishing

One-time configuration for publishing pyfulgur (PyPI) and fulgur (RubyGems) via
OIDC Trusted Publishing, plus Fulgur's versioning policy and day-to-day release
procedure.

## Versioning policy (ZeroVer)

Fulgur follows [ZeroVer](https://0ver.org) while on the `0.x` line. See
[`docs/plans/2026-04-26-versioning-and-release-simplification-design.md`](plans/2026-04-26-versioning-and-release-simplification-design.md)
for the full rationale.

Key points:

- **Each normal release bumps the minor** (`0.5.14` → `0.6.0` → `0.7.0`). This
  matches `release-prepare.yml`'s auto-bump behaviour when the `version` input
  is left empty.
- **Patch numbers are reserved for hotfixes.** If `0.6.0` ships with a critical
  bug, cut `0.6.1` by passing `version=0.6.1` explicitly to
  `release-prepare.yml`'s `workflow_dispatch`.
- **External form: `0.6`. Internal form: `0.6.0`.**
  - Internal (Cargo.toml, npm, PyPI, RubyGems, git tag, CHANGELOG section
    header): `0.6.0` — required by semver / registry validators.
  - External (README, blog, announcements, branding): "Fulgur 0.6".
- **Workspace stays in lockstep** — `fulgur`, `fulgur-cli`, `fulgur-wasm`,
  `fulgur-ruby`, and `pyfulgur` share the same version string. Independent
  binding versioning is future work.
- **No API stability guarantees until `1.0`.** Each minor on the `0.x` line is
  free to introduce breaking changes.

## Skip bindings (core-only release)

To ship a core-only release (crates.io + GitHub Release + CLI binary) and
suppress PyPI / RubyGems / npm publish, run `release-prepare.yml` with
`skip_bindings=true`:

```bash
gh workflow run release-prepare.yml --field version=0.6.1 --field skip_bindings=true
```

What happens:

- `release-prepare.yml` attaches the `release:skip-bindings` label to the
  release PR.
- After merge, `release.yml` skips `publish-npm` via an `if:` guard.
- `release-python.yml` and `release-ruby.yml` run a `check-skip-label` job that
  resolves tag → commit → associated PR labels and skips the `publish` job when
  the label is present.
- crates.io publish, GitHub Release publish, and CLI binary uploads are
  unconditional — the CLI binary is treated as a core release artifact.

If you later need to publish bindings against an already-tagged core release,
trigger `release-python.yml` / `release-ruby.yml` via `workflow_dispatch`. That
escape hatch is intentionally left in place but not yet documented as a first-
class flow.

## PR-based changelog (`release-notes:*` labels)

CHANGELOG and GitHub Release notes are generated **from merged PRs**, not from
commits. `.github/release.yml` defines the category mapping.

| Label | Category |
|-------|----------|
| `release-notes:feature` | Features |
| `release-notes:fix` | Bug Fixes |
| `release-notes:docs` | Documentation |
| `release-notes:internal` | Excluded (CI / refactor / chore / test) |
| `dependencies` | Excluded (Dependabot) |
| (no label) | Other Changes |

Labelling responsibility sits with the **PR author and reviewer**. CI does not
enforce a `release-notes:*` label — unlabelled PRs fall through to "Other
Changes".

`release-prepare.yml` calls `gh api repos/.../releases/generate-notes`, prepends
the resulting body to `CHANGELOG.md`, and reuses the same body for the draft
GitHub Release. git-cliff has been removed.

## 初回公開時の注意

pyfulgur と fulgur gem はどちらも PyPI / RubyGems に未登録の可能性がある。
既存プロジェクトと新規プロジェクトで UI フローが異なる:

- **新規 (pending publisher)**: プロジェクト名だけ予約し、初回 publish 時に
  OIDC claim で自動的に project が作成される。
- **既存 publisher 追加**: 既に project が存在する場合は publisher を追加登録。

## crates.io Trusted Publisher

`release.yml` の `publish` job は `rust-lang/crates-io-auth-action@v1` で
OIDC token を取得し、crates.io に publish する。長期 PAT
(`CARGO_REGISTRY_TOKEN`) を secrets に持つ必要はない。

各 crate (`fulgur`, `fulgur-cli`) で Trusted Publisher を登録する:

1. <https://crates.io/> にログイン (crate owner アカウント)
2. 各 crate の Settings → "Trusted Publishing" タブを開く
   (例: <https://crates.io/crates/fulgur/settings>)
3. "Add" で以下を登録:
   - Repository owner: `fulgur-rs`
   - Repository name: `fulgur`
   - Workflow filename: `release.yml`
   - Environment: `release`
4. `fulgur-cli` も同様に登録

新規 crate の場合は先に <https://crates.io/settings/tokens> 的に
"Pending Trusted Publisher" で名前を予約してから初回 publish で
OIDC 経由の採用が確定する。

登録完了後、旧 secret `CARGO_REGISTRY_TOKEN` は不要なので Settings →
Secrets and variables → Actions から削除してよい。

## PyPI Trusted Publisher

### Production (pypi.org)

1. <https://pypi.org/manage/account/publishing/> にログイン
2. "Add a new pending publisher" (新規の場合) または既存 project の
   "Publishing" タブから publisher 追加:
   - PyPI Project Name: `pyfulgur`
   - Owner: `mitsuru`
   - Repository name: `fulgur`
   - Workflow name: `release-python.yml`
   - Environment name: `pypi`
3. GitHub リポジトリで Environment `pypi` を作成 (Settings → Environments → New environment)。保護ルール不要。

### TestPyPI (test.pypi.org)

本番公開前に dry-run を試す場合のみ。

1. <https://test.pypi.org/manage/account/publishing/> で同様に登録
2. Environment name: `testpypi`
3. GitHub リポジトリで Environment `testpypi` を作成

Dry-run 発火:

```bash
gh workflow run release-python.yml --field dry_run=true
```

## RubyGems Trusted Publisher

既存 gem (fulgur) の場合:

1. <https://rubygems.org/sign_in> にログイン (gem Owner アカウント)
2. <https://rubygems.org/gems/fulgur/trusted_publishers> を開く
3. "Create" で以下を登録:
   - Repository owner: `mitsuru`
   - Repository name: `fulgur`
   - Workflow filename: `release-ruby.yml`
   - Environment: `rubygems`
4. GitHub リポジトリで Environment `rubygems` を作成

OIDC claim (repo + workflow + environment) で自動照合されるため、`role-to-assume`
等の値は workflow 側に不要 (`rubygems/configure-rubygems-credentials` のデフォルト動作)。

新規 gem を作成する場合は <https://rubygems.org/profile/oidc/pending_trusted_publishers>
から "Pending Trusted Publisher" を登録。

注意: RubyGems には TestPyPI に相当する staging 環境がないため、`release-ruby.yml`
の `workflow_dispatch` dry-run は publish をスキップするのみ (build + smoke test
のみ走る)。OIDC / credential フローの実動作検証は初回の本番リリースで行う。

## GitHub Environments

以下の Environment を作成:

- `release` — crates.io Trusted Publisher の OIDC subject claim スコープ
- `pypi` — PyPI Trusted Publisher の OIDC subject claim スコープ
- `testpypi` (dry-run 用)
- `rubygems` — RubyGems Trusted Publisher の OIDC subject claim スコープ

**These environments are OIDC subject-claim scopes, NOT approval gates.** None of
them carry `Required reviewers`. The single release approval is a status check on
the Release PR — see *Approval model* below.

| Environment | Required reviewers | Purpose |
|-------------|--------------------|---------|
| `release`   | Not set | OIDC subject claim (crates.io Trusted Publisher) |
| `pypi`      | Not set | OIDC subject claim (PyPI Trusted Publisher) |
| `rubygems`  | Not set | OIDC subject claim (RubyGems Trusted Publisher) |
| `testpypi`  | Not set | Dry-run only |

`environment: release` also appears on `release.yml`'s `publish-npm` job, where
it is a deliberate no-op (npm provenance authorizes via OIDC `id-token`, which
does not bind to a GitHub environment). It does not gate anything.

### Approval model

The release pipeline is tag-triggered and has **exactly one human approval
gate**: a maintainer approving the release-plz **Release PR**. Everything
downstream flows from that single approval; no `environment` carries required
reviewers.

Why a status check and not native "required reviews": native branch required-
reviews apply to *every* PR into `main`, which would block a solo maintainer's
own feature PRs (no self-approval). Instead a custom `release-pr-approval` commit
status (reported by `.github/workflows/release-pr-approval-gate.yml`) is used —
`success` immediately for ordinary PRs, and for `release-plz-*` PRs
only once a non-author OWNER/MEMBER/COLLABORATOR has an APPROVED review on the
current head commit. Making that status a **required status check** in the `main`
ruleset hard-blocks an unapproved Release PR from merging.

Release flow (where the one approval happens):

1. `release-plz.yml` opens a `release-plz-*` **Release PR**. Merging it is blocked
   by the `release-pr-approval` required status check until a maintainer approves
   the current head commit. **← the single approval.**
2. On merge, `release-plz.yml`'s `release` job publishes to crates.io and creates
   the `vX.Y.Z` tag (App token, so the tag push fires `release.yml`). No further
   approval — the merge was already gated.
3. The tag triggers `release.yml`: build binaries → publish the GitHub Release
   (`--draft=false`, App token) + npm. Publishing the Release fires
   `release:published`.
4. `release-python.yml` / `release-ruby.yml` publish to PyPI / RubyGems on
   `release:published`, gated transitively by the step-1 approval.

With `skip_bindings=true` (label `release:skip-bindings` on the merged PR):
step 4's publish job is skipped via `check-skip-label`
(`needs.check-skip-label.outputs.skip != 'true'` gate).

The OIDC claim scope (repo + workflow + environment) is independent of reviewer
settings — `if: github.event_name == 'release'` separately blocks publishing from
arbitrary refs.

### Security depends on THREE GitHub-settings controls (not code)

Because the approval lives at the *merge* and not in a workflow `environment`
gate, the model is only as strong as these repo settings. They are **not tracked
in-repo** — verify each is live (Settings → Rules / Rulesets) and re-check after
any ruleset edit; do not assume from this doc:

1. **`release-pr-approval` is a required status check** in the `main` branch
   ruleset. Without it, a Release PR can merge unapproved → full publish.
2. **`main` requires a PR** (no direct pushes) so code can't reach `main` — and
   thus `release-plz release` (crates.io + tag) — without going through the gated
   PR. Note: any actor in the ruleset's *bypass list* (e.g. the Repository Admin
   role) sidesteps both #1 and #2, so keep that list to trusted maintainers only.
3. **A `v*` tag-protection ruleset** restricting tag *creation* to the release
   App (below). This is the load-bearing control against the one path the merge
   gate can't see: a `v*` tag pushed **directly** to the repo, which bypasses
   `release-plz.yml` entirely and lands straight on `release.yml` → GitHub
   Release + PyPI + RubyGems. Without this ruleset that path is unauthenticated
   for anyone with tag-push (write) access.

> Status as of 2026-07-02: all three controls are live. #1 and #2 are in the
> `main` branch ruleset; #3 is the `RestrictReleaseTag` tag ruleset (target
> `refs/tags/v[0-9]*.[0-9]*.[0-9]*`, restrict creations/updates/deletions), whose
> only bypass actor is the `fulgur-release-bot` App — no admin blanket bypass, so
> even a maintainer cannot push a `v*` tag directly. Re-verify with
> `gh api repos/<owner>/<repo>/rulesets` after any settings change.

### Tag-protection ruleset (control #3 — the direct-tag-push block)

Restrict who can create `v*` tags so a rogue tag never reaches `release.yml`:

1. GitHub repo → Settings → Rules → Rulesets → **New tag ruleset**.
2. Target: **Tag** → include by pattern `v[0-9]*.[0-9]*.[0-9]*` (or `v*`).
3. Rules: enable **Restrict creations** (and **Restrict updates** /
   **Restrict deletions**).
4. Bypass list: add **only** the release GitHub App (`fulgur-release-bot`) — the
   actor that creates tags in `release-plz.yml`'s `release` job. Do **not** add
   the "Repository admin" role as a blanket bypass, or a human admin could still
   push a `v*` tag directly.
5. Enforcement status: **Active**.

With this ruleset a `v*` tag can only be created by the release App, which only
does so from the post-merge `release-plz release` job — so it can only exist after
the Release-PR approval. This ruleset is a GitHub Settings change; keep it in sync
with the App name if the release bot is ever renamed.

## GitHub App (release publisher)

`release.yml` の最終 `gh release edit --draft=false` は `release:published` イベントを
発火させ、`release-python.yml` / `release-ruby.yml` を連鎖起動する。しかし GitHub の
無限ループ防止仕様により **`GITHUB_TOKEN` で発火したイベントは別 workflow を起動しない**
([docs](https://docs.github.com/en/actions/using-workflows/triggering-a-workflow#triggering-a-workflow-from-a-workflow))。
そのため GitHub App token で publish する必要がある。

さらに `Tag release` step も同じ App token を使用する。`GITHUB_TOKEN` では
`.github/workflows/*.yml` を含む commit に tag を push しようとすると
`refusing to allow a GitHub App to create or update workflow ... without
workflows permission` で拒否されるためで、App 側に `Workflows: Read and write`
権限があれば通る。

### App 作成手順

1. <https://github.com/settings/apps/new> で新規 App を作成 (個人アカウント所有でも可)
   - GitHub App name: `fulgur-release-bot` 等 (任意・グローバル一意)
   - Homepage URL: 任意 (例: リポジトリ URL)
   - Webhook: "Active" のチェックを外す
   - Repository permissions:
     - **Contents: Read and write** (release 作成・編集に必要)
     - **Workflows: Read and write** (tag push 時に workflow ファイル変更を伴う commit を通すために必要)
     - **Pull requests: Read and write** (`release-plz.yml` の release-pr job が
       この App token で Release PR を作成/更新するために必要。App 作成の PR/commit
       にすることで `ci.yml` (pull_request) が Release PR で自動発火する)
   - Where can this GitHub App be installed?: "Only on this account"
2. 作成後の App 設定画面で:
   - **App ID** を控える (数値)
   - **Private keys** → "Generate a private key" で `.pem` をダウンロード
3. 左メニュー "Install App" から対象リポジトリ (`fulgur-rs/fulgur`) に install
   - "Only select repositories" で fulgur のみに限定

### リポジトリ secrets に登録

Settings → Secrets and variables → Actions → New repository secret:

- `RELEASE_APP_ID`: 上記 App ID (数値)
- `RELEASE_APP_PRIVATE_KEY`: ダウンロードした `.pem` ファイルの内容全体
  (`-----BEGIN RSA PRIVATE KEY-----` から `-----END RSA PRIVATE KEY-----` まで)

### 動作確認

次回 release で GitHub Actions の `release.yml` → `release` job が成功したあと、
Actions タブで `release-python.yml` / `release-ruby.yml` が自動的に `release` イベントで
起動することを確認する。

## Release procedure

### Normal release (minor bump)

1. Trigger `release-prepare.yml` via `workflow_dispatch`.
   - Leave `version` **empty** to let auto-bump pick the next minor (`0.x.0`).
   - For a hotfix, pass an explicit value such as `version=0.6.1`.
2. Inspect the generated `release/vX.Y.Z` PR (CHANGELOG diff, Cargo.toml
   bumps).
3. Merge the PR → `release.yml`'s `publish` job pauses on the `release` env.
4. Approve from the GitHub Actions UI.
5. crates.io publish + tag push + GitHub Release publish + npm publish all
   complete.
6. `release: published` fires `release-python.yml` and `release-ruby.yml` in
   parallel.
7. `check-skip-label` confirms the label is absent → `publish` proceeds.
8. PyPI / RubyGems reflect the new version within minutes.

### Core-only release (skip bindings)

```bash
gh workflow run release-prepare.yml \
  --field version=0.6.1 \
  --field skip_bindings=true
```

- The generated PR is auto-labelled `release:skip-bindings`.
- After merge, `release.yml` skips npm publish (crates.io / GitHub Release /
  CLI binary still ship).
- `release-python.yml` / `release-ruby.yml` still run build + smoke tests but
  `check-skip-label` skips the `publish` job only.

### Previewing release notes

Before triggering `release-prepare.yml`, you can dry-run the notes:

```bash
gh api repos/fulgur-rs/fulgur/releases/generate-notes \
  -f tag_name=v0.6.0 \
  --jq .body
```

Verify categorisation matches expectations (i.e. that the relevant
`release-notes:*` labels are attached). Add missing labels with
`gh pr edit <num> --add-label release-notes:fix` (etc.) and re-run to confirm.
