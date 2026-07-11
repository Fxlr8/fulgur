# CHANGELOG marker + SSoT mirror Implementation Plan

**Goal:** release-plz aux-sync が Release PR ブランチの手書き changelog エントリを marker 対で保護し、GH Release body は committed CHANGELOG.md ミラーになる SSoT ワークフローを実装する (v94v + kf6a 統合)。

**Architecture:** CHANGELOG.md の各 version セクションに `<!-- release-notes:auto:begin -->` / `<!-- release-notes:auto:end -->` marker を導入。marker 内側は毎 aux-sync で fresh generate-notes に再生成、外側 (PREAMBLE / POSTAMBLE) は PR ブランチ現状を保持。再構築ロジックは Python スクリプト `scripts/release/rebuild_changelog.py` に切り出し pytest でカバー。GH Release body は committed CHANGELOG.md 由来に切り替え。

**Tech Stack:** Python 3.12 (GHA runner 標準)、pytest、bash (workflow glue)、GitHub Actions。

**Related beads issue:** fulgur-v94v (SSoT protector) + fulgur-kf6a (SSoT mirror)。

**Design reference:** `bd show fulgur-v94v` (DESIGN セクション)。

---

## Task 1: scripts/release/ scaffold + initial run test

**Files:**

- Create: `scripts/release/rebuild_changelog.py`
- Create: `scripts/release/test_rebuild_changelog.py`
- Create: `scripts/release/__init__.py` (空、pytest import 用)

**Step 1: 空の test file + 1 ケースを書く**

```python
# scripts/release/test_rebuild_changelog.py
from pathlib import Path
import pytest
from rebuild_changelog import rebuild_changelog


HEADER = "# Changelog\n\nAll notable changes to this project will be documented in this file.\n"


def test_initial_run_no_version_section():
    """PR CHANGELOG に対象 version セクションが無い場合、marker 付き新規セクションを先頭に挿入する。"""
    pr_changelog = HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"
    origin_changelog = pr_changelog
    auto_notes = "### Bug Fixes\n* new fix\n"

    result = rebuild_changelog(
        version="0.31.0",
        date="2026-07-11",
        auto_notes=auto_notes,
        pr_changelog=pr_changelog,
        origin_changelog=origin_changelog,
    )

    assert "## [0.31.0] - 2026-07-11" in result
    assert "<!-- release-notes:auto:begin -->" in result
    assert "<!-- release-notes:auto:end -->" in result
    assert "* new fix" in result
    assert "## [0.30.0] - 2026-07-01" in result
    assert "* old fix" in result
    # 履歴セクションは origin/main 由来
    assert result.index("## [0.31.0]") < result.index("## [0.30.0]")
```

**Step 2: test 実行 (fail 確認)**

```bash
cd .worktrees/v94v-changelog-marker
python3 -m pytest scripts/release/test_rebuild_changelog.py::test_initial_run_no_version_section -v
```

Expected: `ModuleNotFoundError: No module named 'rebuild_changelog'` で FAIL。

**Step 3: 最小実装**

```python
# scripts/release/rebuild_changelog.py
"""Rebuild CHANGELOG.md preserving hand-added marker-outside content.

See docs/plans/2026-07-11-changelog-marker-ssot.md and bd show fulgur-v94v.
"""
from __future__ import annotations

HEADER = (
    "# Changelog\n\n"
    "All notable changes to this project will be documented in this file.\n"
)

AUTO_BEGIN = "<!-- release-notes:auto:begin -->"
AUTO_END = "<!-- release-notes:auto:end -->"


def _extract_history(changelog: str) -> str:
    """Return everything from the first `## [` heading to EOF, unchanged."""
    lines = changelog.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if line.startswith("## ["):
            return "".join(lines[i:])
    return ""


def rebuild_changelog(
    *,
    version: str,
    date: str,
    auto_notes: str,
    pr_changelog: str,
    origin_changelog: str,
) -> str:
    """Build a new CHANGELOG.md body with the version section wrapped in markers."""
    history = _extract_history(origin_changelog)

    section = (
        f"## [{version}] - {date}\n\n"
        f"{AUTO_BEGIN}\n"
        f"{auto_notes.rstrip()}\n"
        f"{AUTO_END}\n\n"
    )

    return HEADER + "\n" + section + history
```

**Step 4: test 実行 (pass 確認)**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py::test_initial_run_no_version_section -v
```

Expected: 1 passed。

**Step 5: commit**

```bash
git add scripts/release/__init__.py scripts/release/rebuild_changelog.py scripts/release/test_rebuild_changelog.py
git commit -m "feat(release): scaffold rebuild_changelog.py + initial-run test (fulgur-v94v)"
```

---

## Task 2: idempotency (rerun_no_hand_adds)

**Files:**

- Modify: `scripts/release/test_rebuild_changelog.py`
- Modify: `scripts/release/rebuild_changelog.py`

**Step 1: idempotency テストを追加**

```python
def test_rerun_no_hand_adds_idempotent():
    """すでに marker 付きセクションがある PR CHANGELOG に対して同じ auto notes を渡すと出力が完全一致する。"""
    auto_notes = "### Bug Fixes\n* fix A\n"
    origin = HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"

    # 1 回目
    pr_v1 = origin
    result_1 = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes=auto_notes, pr_changelog=pr_v1, origin_changelog=origin,
    )

    # 2 回目: PR ブランチには前回の出力が乗っている想定
    result_2 = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes=auto_notes, pr_changelog=result_1, origin_changelog=origin,
    )

    assert result_1 == result_2
```

**Step 2: test 実行 (fail 確認)**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py::test_rerun_no_hand_adds_idempotent -v
```

Expected: FAIL — 現在の実装は PR CHANGELOG を無視して常に origin history を base にするので、2 回目の入力に `## [0.31.0]` が入ると origin 側にも展開されて重複する可能性がある。実装挙動により pass する可能性もある — 結果を確認して次ステップを決める。

**Step 3: 実装調整 (必要なら)**

- `_extract_history(origin_changelog)` は origin/main を見るので、PR ブランチ由来の `## [0.31.0]` が history に混ざらないことを確認。
- origin_changelog に `## [0.31.0]` が入っている場合 (再走中に既に main にマージ済み、稀) → history から `## [0.31.0]` セクションを削って重複回避する。

必要になったら _extract_history に version filter を足す:

```python
def _extract_history(changelog: str, *, drop_version: str | None = None) -> str:
    ...
    if drop_version is not None:
        # Skip sections matching `## [drop_version] - `
        ...
```

**Step 4: test pass 確認**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py -v
```

Expected: 2 passed。

**Step 5: commit**

```bash
git add scripts/release/rebuild_changelog.py scripts/release/test_rebuild_changelog.py
git commit -m "test(release): idempotent rerun with no hand-adds (fulgur-v94v)"
```

---

## Task 3: preserve POSTAMBLE (hand-adds after auto:end)

**Files:**

- Modify: `scripts/release/test_rebuild_changelog.py`
- Modify: `scripts/release/rebuild_changelog.py`

**Step 1: POSTAMBLE テスト**

```python
def test_rerun_with_postamble_preserved():
    """auto:end の後に手書きされた ### Security セクションが保持される。"""
    auto_notes = "### Bug Fixes\n* fix A\n"
    origin = HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"
    pr_with_postamble = (
        HEADER + "\n"
        "## [0.31.0] - 2026-07-11\n\n"
        f"{AUTO_BEGIN}\n"
        "### Bug Fixes\n* fix A\n"
        f"{AUTO_END}\n\n"
        "### Security\n* GHSA-xxxx: hand-added\n\n"
        "## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"
    )

    result = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes=auto_notes, pr_changelog=pr_with_postamble, origin_changelog=origin,
    )

    assert "GHSA-xxxx: hand-added" in result
    # POSTAMBLE は auto:end と 次の ## [0.30.0] の間に位置する
    end_idx = result.index(AUTO_END)
    security_idx = result.index("GHSA-xxxx")
    next_ver_idx = result.index("## [0.30.0]")
    assert end_idx < security_idx < next_ver_idx
```

`AUTO_BEGIN` / `AUTO_END` を test file の top で import (`from rebuild_changelog import ..., AUTO_BEGIN, AUTO_END`)。

**Step 2: test 実行 (fail 確認)**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py::test_rerun_with_postamble_preserved -v
```

Expected: FAIL — 現在 POSTAMBLE 抽出無し。

**Step 3: 実装 — PREAMBLE/POSTAMBLE 抽出**

```python
import re

_SECTION_HEAD = re.compile(r"^## \[[^\]]+\]", re.M)


def _extract_version_section(pr_changelog: str, version: str) -> str | None:
    """Return the `## [version]` section body (from heading to next `## [` or EOF)."""
    pattern = re.compile(rf"^## \[{re.escape(version)}\][^\n]*\n", re.M)
    m = pattern.search(pr_changelog)
    if not m:
        return None
    start = m.end()
    next_head = _SECTION_HEAD.search(pr_changelog, pos=start)
    end = next_head.start() if next_head else len(pr_changelog)
    return pr_changelog[start:end]


def _split_around_markers(section: str) -> tuple[str, str, bool]:
    """Return (preamble, postamble, markers_present).

    On missing markers: return ('', section, False) — safe fallback.
    """
    begin_idx = section.find(AUTO_BEGIN)
    end_idx = section.find(AUTO_END)
    if begin_idx == -1 or end_idx == -1 or end_idx < begin_idx:
        return ("", section, False)
    preamble = section[:begin_idx].strip("\n")
    postamble = section[end_idx + len(AUTO_END):].strip("\n")
    return (preamble, postamble, True)
```

`rebuild_changelog` を書き換え、抽出結果を組み込む:

```python
def rebuild_changelog(*, version, date, auto_notes, pr_changelog, origin_changelog):
    history = _extract_history(origin_changelog)
    section = _extract_version_section(pr_changelog, version)
    if section is not None:
        preamble, postamble, _ = _split_around_markers(section)
    else:
        preamble, postamble = "", ""

    parts = [f"## [{version}] - {date}\n"]
    if preamble:
        parts.append(f"\n{preamble}\n")
    parts.append(f"\n{AUTO_BEGIN}\n{auto_notes.rstrip()}\n{AUTO_END}\n")
    if postamble:
        parts.append(f"\n{postamble}\n")
    section_out = "".join(parts)

    return HEADER + "\n" + section_out + "\n" + history
```

**Step 4: test pass 確認**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py -v
```

Expected: 3 passed (initial / idempotent / postamble)。initial test の期待 format が変わっていたら微調整。

**Step 5: commit**

```bash
git add scripts/release/rebuild_changelog.py scripts/release/test_rebuild_changelog.py
git commit -m "feat(release): preserve POSTAMBLE hand-adds via marker split (fulgur-v94v)"
```

---

## Task 4: preserve PREAMBLE (hand-adds before auto:begin)

**Files:**

- Modify: `scripts/release/test_rebuild_changelog.py`

**Step 1: PREAMBLE テスト (実装は Task 3 で既に対応済み想定)**

```python
def test_rerun_with_preamble_preserved():
    """auto:begin の前に手書きされた migration note が保持される。"""
    auto_notes = "### Bug Fixes\n* fix A\n"
    origin = HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"
    pr = (
        HEADER + "\n"
        "## [0.31.0] - 2026-07-11\n\n"
        "> Migration note: this release bumps MSRV to 1.85.\n\n"
        f"{AUTO_BEGIN}\n"
        "### Bug Fixes\n* fix A\n"
        f"{AUTO_END}\n\n"
        "## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"
    )

    result = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes=auto_notes, pr_changelog=pr, origin_changelog=origin,
    )

    assert "Migration note" in result
    version_idx = result.index("## [0.31.0]")
    migration_idx = result.index("Migration note")
    begin_idx = result.index(AUTO_BEGIN)
    assert version_idx < migration_idx < begin_idx


def test_rerun_with_both_preserved():
    """PREAMBLE + POSTAMBLE 両方保持される。"""
    auto_notes = "### Bug Fixes\n* fix A\n"
    origin = HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"
    pr = (
        HEADER + "\n"
        "## [0.31.0] - 2026-07-11\n\n"
        "> Migration note\n\n"
        f"{AUTO_BEGIN}\n### Bug Fixes\n* fix A\n{AUTO_END}\n\n"
        "### Security\n* GHSA-xxxx\n\n"
        "## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"
    )

    result = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes=auto_notes, pr_changelog=pr, origin_changelog=origin,
    )

    assert "Migration note" in result
    assert "GHSA-xxxx" in result
```

**Step 2: test 実行**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py -v
```

Expected: 5 passed (Task 3 の実装で PREAMBLE も動くはず、動かなければ調整)。

**Step 3-4: 実装調整 (必要なら)**

Task 3 の `_split_around_markers` が両側を返しているので pass すべき。fail なら preamble 側の strip / spacing を再検討。

**Step 5: commit**

```bash
git add scripts/release/test_rebuild_changelog.py
git commit -m "test(release): preserve PREAMBLE + both hand-adds (fulgur-v94v)"
```

---

## Task 5: missing markers fallback + stderr warning

**Files:**

- Modify: `scripts/release/test_rebuild_changelog.py`
- Modify: `scripts/release/rebuild_changelog.py`

**Step 1: missing_markers テスト**

```python
import io
import sys


def test_missing_markers_fallback(capsys):
    """marker が無い旧フォーマットのセクションは全内容 POSTAMBLE 化 + stderr warning。"""
    auto_notes = "### Bug Fixes\n* fresh fix\n"
    origin = HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"
    pr = (
        HEADER + "\n"
        "## [0.31.0] - 2026-07-11\n\n"
        "### Bug Fixes\n* legacy auto item\n\n"
        "### Security\n* hand-added, no markers\n\n"
        "## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"
    )

    result = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes=auto_notes, pr_changelog=pr, origin_changelog=origin,
    )

    assert "hand-added, no markers" in result
    assert "* fresh fix" in result  # 新 auto は挿入される
    assert AUTO_BEGIN in result
    assert AUTO_END in result

    captured = capsys.readouterr()
    assert "::warning::" in captured.err
    assert "missing" in captured.err.lower() or "marker" in captured.err.lower()
```

**Step 2: test 実行 (fail 確認)**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py::test_missing_markers_fallback -v
```

Expected: FAIL — 現状 stderr warning 未実装。

**Step 3: 実装**

`rebuild_changelog` に markers_present 判定 + stderr 出力:

```python
import sys


def rebuild_changelog(*, version, date, auto_notes, pr_changelog, origin_changelog):
    history = _extract_history(origin_changelog)
    section = _extract_version_section(pr_changelog, version)
    if section is not None:
        preamble, postamble, markers_present = _split_around_markers(section)
        if not markers_present:
            print(
                f"::warning::CHANGELOG section [{version}] lacks auto markers; "
                "treating full content as postamble (safe fallback). "
                "Add <!-- release-notes:auto:begin/end --> markers to control regeneration.",
                file=sys.stderr,
            )
    else:
        preamble, postamble = "", ""

    ...  # 以下変わらず
```

**Step 4: test pass 確認**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py -v
```

Expected: 6 passed。

**Step 5: commit**

```bash
git add scripts/release/rebuild_changelog.py scripts/release/test_rebuild_changelog.py
git commit -m "feat(release): missing-marker fallback + stderr warning (fulgur-v94v)"
```

---

## Task 6: auto_notes_updated (only auto region changes on rerun)

**Files:**

- Modify: `scripts/release/test_rebuild_changelog.py`

**Step 1: テスト**

```python
def test_auto_notes_updated_postamble_stable():
    """新しい PR がマージされて auto notes が更新されても POSTAMBLE は安定。"""
    origin = HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"

    pr_v1 = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes="### Bug Fixes\n* fix A\n",
        pr_changelog=origin, origin_changelog=origin,
    )

    # 人間が POSTAMBLE を追記
    pr_with_hand = pr_v1.replace(
        f"{AUTO_END}\n",
        f"{AUTO_END}\n\n### Security\n* GHSA-yyyy\n",
    )

    # 新 PR がマージされて auto notes が拡張された
    result = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes="### Bug Fixes\n* fix A\n* fix B (new PR)\n",
        pr_changelog=pr_with_hand, origin_changelog=origin,
    )

    assert "* fix A" in result
    assert "* fix B (new PR)" in result
    assert "GHSA-yyyy" in result
```

**Step 2-4: test 実行 → 実装調整不要のはず → pass 確認**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py -v
```

Expected: 7 passed。

**Step 5: commit**

```bash
git add scripts/release/test_rebuild_changelog.py
git commit -m "test(release): auto notes update keeps postamble stable (fulgur-v94v)"
```

---

## Task 7: CLI wrapper + subprocess integration test

**Files:**

- Modify: `scripts/release/rebuild_changelog.py`
- Modify: `scripts/release/test_rebuild_changelog.py`

**Step 1: CLI wrapper 実装**

```python
# rebuild_changelog.py の末尾に追加
import argparse


def _read_file(path: str) -> str:
    from pathlib import Path
    if path == "-":
        return sys.stdin.read()
    return Path(path).read_text(encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Rebuild CHANGELOG.md preserving marker-outside hand-adds.")
    parser.add_argument("--version", required=True)
    parser.add_argument("--date", required=True)
    parser.add_argument("--auto-notes", required=True, help="Path to fresh generate-notes body")
    parser.add_argument("--pr-changelog", required=True, help="Path to PR branch CHANGELOG.md")
    parser.add_argument("--origin-changelog", required=True, help="Path to origin/main CHANGELOG.md")
    args = parser.parse_args(argv)

    result = rebuild_changelog(
        version=args.version,
        date=args.date,
        auto_notes=_read_file(args.auto_notes),
        pr_changelog=_read_file(args.pr_changelog),
        origin_changelog=_read_file(args.origin_changelog),
    )
    sys.stdout.write(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

**Step 2: subprocess integration test**

```python
import subprocess
from pathlib import Path


def test_cli_end_to_end(tmp_path):
    """CLI で subprocess 経由に呼び出しても正しい出力が stdout に出る。"""
    auto_path = tmp_path / "RELEASE_NOTES.md"
    auto_path.write_text("### Bug Fixes\n* fresh fix\n")
    pr_path = tmp_path / "CHANGELOG.md"
    pr_path.write_text(HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old\n")
    origin_path = tmp_path / "origin_CHANGELOG.md"
    origin_path.write_text(pr_path.read_text())

    script = Path(__file__).parent / "rebuild_changelog.py"
    result = subprocess.run(
        [
            "python3", str(script),
            "--version", "0.31.0",
            "--date", "2026-07-11",
            "--auto-notes", str(auto_path),
            "--pr-changelog", str(pr_path),
            "--origin-changelog", str(origin_path),
        ],
        capture_output=True, text=True, check=True,
    )
    assert "## [0.31.0] - 2026-07-11" in result.stdout
    assert AUTO_BEGIN in result.stdout
    assert "* fresh fix" in result.stdout
    assert "## [0.30.0] - 2026-07-01" in result.stdout
```

**Step 3-4: test 実行 → pass 確認**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py -v
```

Expected: 8 passed。

**Step 5: commit**

```bash
git add scripts/release/rebuild_changelog.py scripts/release/test_rebuild_changelog.py
git commit -m "feat(release): CLI wrapper + e2e subprocess test (fulgur-v94v)"
```

---

## Task 8: maintainer convention README

**Files:**

- Create: `scripts/release/README.md`

**Step 1: README 作成**

```markdown
# scripts/release/

Release automation helpers invoked from `.github/workflows/release-plz.yml` and `release.yml`.

## `rebuild_changelog.py`

Rebuilds `CHANGELOG.md` for a release, preserving hand-added notes that live
**outside** the `<!-- release-notes:auto:begin -->` / `<!-- release-notes:auto:end -->`
marker pair. Called by `release-plz.yml` aux-sync on every push to `main` while a
Release PR is open.

### Section format

Each version section in `CHANGELOG.md`:

    ## [0.35.0] - 2026-07-11

    > Optional preamble (hand-added; preserved across aux-sync)

    <!-- release-notes:auto:begin -->
    ### Bug Fixes
    * ... (regenerated from PR labels every aux-sync)
    <!-- release-notes:auto:end -->

    ### Optional postamble (hand-added; preserved)
    * GHSA-xxxx-... security note

### Adding a no-PR entry (GHSA, direct hotfix)

1. Check out the Release PR branch: `gh pr checkout <N>`.
2. Edit the current version section in `CHANGELOG.md`:
   - Put the note **outside** the marker pair — either above `<!-- release-notes:auto:begin -->` or below `<!-- release-notes:auto:end -->`.
   - Do **not** edit content between the markers; aux-sync overwrites it.
3. Commit and push. aux-sync (release-plz.yml) preserves the hand-add on subsequent runs.

### Missing-markers fallback

If a section lacks the marker pair, aux-sync logs `::warning::` and treats all content as postamble (safe: no data loss). Add the markers manually to control regeneration.
```

**Step 2-4: markdownlint check**

```bash
npx markdownlint-cli2 'scripts/release/README.md'
```

Expected: no violations (or fix any that appear).

**Step 5: commit**

```bash
git add scripts/release/README.md
git commit -m "docs(release): scripts/release/README with marker convention (fulgur-v94v)"
```

---

## Task 9: CI job for scripts/release/**

**Files:**

- Modify: `.github/workflows/ci.yml`

**Step 1: 現行 ci.yml 構造の確認**

```bash
head -40 .github/workflows/ci.yml
grep -n "^  [a-z_-]*:" .github/workflows/ci.yml | head -20
```

`jobs:` セクションの構造を把握し、新規 job を追加する位置を決める。

**Step 2: pytest job 追加**

`.github/workflows/ci.yml` の適切な位置に:

```yaml
  scripts-release-tests:
    name: Release scripts (pytest)
    runs-on: ubuntu-latest
    # Path-filter is at job level via if. GitHub Actions doesn't support job-level
    # paths on pull_request without a full workflow filter; using dorny/paths-filter
    # or a `changes` job would work but a simple always-run is cheap (<1min) and
    # protects us from filter drift.
    steps:
      - uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0  # v7.0.0
      - uses: actions/setup-python@e348410e00f449f3d70bb17cc94a48c2b26a2a35  # v6.1.0
        with:
          python-version: '3.12'
      - run: pip install pytest
      - run: python3 -m pytest scripts/release/ -v
```

**Step 3: yamllint / actionlint 確認 (あれば)**

```bash
# actionlint がなければ skip
which actionlint && actionlint .github/workflows/ci.yml
```

**Step 4: commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add pytest job for scripts/release (fulgur-v94v)"
```

---

## Task 10: wire rebuild_changelog.py into release-plz.yml

**Files:**

- Modify: `.github/workflows/release-plz.yml:210-227`

**Step 1: 該当ブロックの現状把握**

```bash
sed -n '186,230p' .github/workflows/release-plz.yml
```

該当箇所は「gh api generate-notes → RELEASE_NOTES.md → OLD_CHANGELOG=... → { echo header; cat RELEASE_NOTES.md; awk ... } > CHANGELOG.tmp」の流れ。

**Step 2: 置換案**

`OLD_CHANGELOG=...` から `mv CHANGELOG.tmp CHANGELOG.md` までの `{ ... } > CHANGELOG.tmp; mv CHANGELOG.tmp CHANGELOG.md` ブロックを、script 呼び出しに置換:

```bash
DATE=$(date -u +%Y-%m-%d)
sed -i "/^## What's Changed$/d" RELEASE_NOTES.md

# Read origin/main CHANGELOG into a file (script requires paths, not stdin+fd tricks).
git show origin/main:CHANGELOG.md > /tmp/origin_CHANGELOG.md

python3 scripts/release/rebuild_changelog.py \
  --version "$VERSION" \
  --date "$DATE" \
  --auto-notes RELEASE_NOTES.md \
  --pr-changelog CHANGELOG.md \
  --origin-changelog /tmp/origin_CHANGELOG.md \
  > CHANGELOG.tmp

mv CHANGELOG.tmp CHANGELOG.md
rm -f RELEASE_NOTES.md /tmp/origin_CHANGELOG.md
```

git config / add / commit / push 部分 (230-241) はそのまま。

**Step 3: yamllint 確認**

```bash
which yamllint && yamllint .github/workflows/release-plz.yml
```

**Step 4: dry-run 相当の smoke: script 単体で「今の CHANGELOG.md + 今の origin/main + 空の RELEASE_NOTES.md」を渡して stdout に出る事を確認**

```bash
echo "" > /tmp/fake_release_notes.md
git show origin/main:CHANGELOG.md > /tmp/origin_CL.md
python3 scripts/release/rebuild_changelog.py \
  --version "0.99.0" --date "2026-07-11" \
  --auto-notes /tmp/fake_release_notes.md \
  --pr-changelog CHANGELOG.md \
  --origin-changelog /tmp/origin_CL.md \
  | head -30
```

Expected: `## [0.99.0] - 2026-07-11` + marker + 空 auto + 既存 history が見える。

**Step 5: commit**

```bash
git add .github/workflows/release-plz.yml
git commit -m "ci(release-plz): switch aux-sync to rebuild_changelog.py (fulgur-v94v)"
```

---

## Task 11: SSoT mirror in release.yml (kf6a)

**Files:**

- Modify: `.github/workflows/release.yml:203-209`

**Step 1: 現状把握**

```bash
sed -n '186,215p' .github/workflows/release.yml
```

該当: `NOTES=$(gh api "repos/$REPO/releases/generate-notes" -f tag_name="v$VERSION" --jq .body)` を CHANGELOG 由来に置換。

**Step 2: awk one-liner で section 抽出**

```bash
# Extract "## [VERSION]" body (excluding heading), stopping at next "## [".
NOTES=$(awk -v ver="$VERSION" '
  BEGIN { in_sec = 0 }
  /^## \[/ {
    if (in_sec) exit
    if (match($0, /^## \[[^\]]+\]/)) {
      hdr = substr($0, RSTART, RLENGTH)
      if (hdr == "## [" ver "]") { in_sec = 1; next }
    }
  }
  in_sec { print }
' CHANGELOG.md)

# Fallback: if empty, use generate-notes (safety net for legacy / edge cases).
if [ -z "$(echo "$NOTES" | tr -d '[:space:]')" ]; then
  echo "::warning::CHANGELOG.md has no section for $VERSION; falling back to generate-notes"
  NOTES=$(gh api "repos/$REPO/releases/generate-notes" -f tag_name="v$VERSION" --jq .body)
fi

gh release edit "v$VERSION" --notes "$NOTES" --draft=false --repo "$REPO"
```

**Step 3: smoke test — 現行 CHANGELOG.md で awk を試す**

```bash
awk -v ver="0.34.0" '
  BEGIN { in_sec = 0 }
  /^## \[/ {
    if (in_sec) exit
    if (match($0, /^## \[[^\]]+\]/)) {
      hdr = substr($0, RSTART, RLENGTH)
      if (hdr == "## [" ver "]") { in_sec = 1; next }
    }
  }
  in_sec { print }
' CHANGELOG.md | head -20
```

Expected: 0.34.0 section 本体 (見出し除く) が出る。

**Step 4: commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(release): source GH Release body from CHANGELOG.md (fulgur-kf6a)"
```

---

## Task 12: Final verification (pytest + workflow lint + markdownlint)

**Step 1: 全 pytest 再走**

```bash
python3 -m pytest scripts/release/ -v
```

Expected: 全 8 tests passed。

**Step 2: markdownlint**

```bash
npx markdownlint-cli2 'scripts/release/**/*.md' 'docs/plans/2026-07-11-*.md'
```

**Step 3: yamllint / actionlint (あれば)**

```bash
which actionlint && actionlint .github/workflows/release-plz.yml .github/workflows/release.yml .github/workflows/ci.yml
```

**Step 4: git status / log 確認**

```bash
git log --oneline main..HEAD
git diff --stat main..HEAD
```

Expected: task ごとに小さめの commit が 10+ 個並んでいる、touched files が想定範囲 (scripts/release/**, .github/workflows/{ci,release-plz,release}.yml, docs/plans/2026-07-11-*.md, CHANGELOG.md は変更なし)。

**Step 5: 最終 commit なし (品質チェックのみ)**

問題があれば修正 commit。無ければ次の finishing-a-development-branch へ。

---

## Task 13: Test end-to-end intent via unit-level golden

**Files:**

- Modify: `scripts/release/test_rebuild_changelog.py`

**Step 1: 実 CHANGELOG を fixture として使う golden test**

```python
def test_golden_current_changelog(tmp_path):
    """本物の CHANGELOG.md を PR ブランチ + origin 両方として使い、
    marker 追加前の初回 aux-sync を再現。missing-markers 経路で全内容が保持されることを確認。"""
    repo_root = Path(__file__).resolve().parents[2]
    changelog = (repo_root / "CHANGELOG.md").read_text(encoding="utf-8")

    auto = "### Bug Fixes\n* fresh test fix\n"

    result = rebuild_changelog(
        version="0.99.0", date="2026-07-11",
        auto_notes=auto, pr_changelog=changelog, origin_changelog=changelog,
    )

    # 新 section が先頭
    assert result.index("## [0.99.0] - 2026-07-11") < result.index("## [0.34.0]")
    # marker 追加
    assert AUTO_BEGIN in result
    # 既存 history 保持
    assert "## [0.34.0]" in result
    assert "## [0.27.0]" in result
    # 0.27.0 の Security note 保持 (GHSA-395p-pj7r-jm42)
    assert "GHSA-395p-pj7r-jm42" in result
```

**Step 2-4: pytest 実行**

```bash
python3 -m pytest scripts/release/test_rebuild_changelog.py::test_golden_current_changelog -v
```

Expected: PASS。

**Step 5: commit**

```bash
git add scripts/release/test_rebuild_changelog.py
git commit -m "test(release): golden against current CHANGELOG.md (fulgur-v94v)"
```

---

## Rollout Notes

- **Merge order**: この PR は 1 発マージで v94v + kf6a 両方が有効化される。
- **初回 aux-sync**: 新 workflow が動く最初の Release PR では既存 CHANGELOG に marker が無いため missing-markers fallback 経路で全内容 POSTAMBLE 化される。生成される新 section だけに marker が付き、既存 history section は改変されない。
- **既存 history section への marker 遡及**: 不要 (aux-sync は最新 section だけを再生成)。
- **手書き運用開始**: 次回リリース以降、GHSA fix 等を Release PR ブランチ の marker 外領域に手書きしても保持される。
- **rollback**: PR revert で inline bash 復帰、挙動は現状に戻る。

## Follow-ups (別 issue)

- `missing_markers` の warning を将来 fail に格上げできるかは、marker 導入後 3-5 リリースを通して問題が出なければ検討 (別 beads issue 起票候補)。
- 0.27.0 CHANGELOG と GH Release body の Security note 表現統一 (`### Security` vs `## Security`、CVSS 表記) — 既に両方反映済みで小さな整形差なので必要になったら手直し。
