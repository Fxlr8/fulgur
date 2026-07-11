import subprocess
from pathlib import Path
import pytest
from rebuild_changelog import rebuild_changelog, AUTO_BEGIN, AUTO_END


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
    assert result.index("## [0.31.0]") < result.index("## [0.30.0]")


def test_rerun_no_hand_adds_idempotent():
    """すでに marker 付きセクションがある PR CHANGELOG に対して同じ auto notes を渡すと出力が完全一致する。"""
    auto_notes = "### Bug Fixes\n* fix A\n"
    origin = HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"

    pr_v1 = origin
    result_1 = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes=auto_notes, pr_changelog=pr_v1, origin_changelog=origin,
    )

    result_2 = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes=auto_notes, pr_changelog=result_1, origin_changelog=origin,
    )

    assert result_1 == result_2


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
    end_idx = result.index(AUTO_END)
    security_idx = result.index("GHSA-xxxx")
    next_ver_idx = result.index("## [0.30.0]")
    assert end_idx < security_idx < next_ver_idx


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
    assert "* fresh fix" in result
    assert AUTO_BEGIN in result
    assert AUTO_END in result

    captured = capsys.readouterr()
    assert "::warning::" in captured.err
    assert "missing" in captured.err.lower() or "marker" in captured.err.lower()


def test_auto_notes_updated_postamble_stable():
    """新しい PR がマージされて auto notes が更新されても POSTAMBLE は安定。"""
    origin = HEADER + "\n## [0.30.0] - 2026-07-01\n\n### Bug Fixes\n* old fix\n"

    pr_v1 = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes="### Bug Fixes\n* fix A\n",
        pr_changelog=origin, origin_changelog=origin,
    )

    pr_with_hand = pr_v1.replace(
        f"{AUTO_END}\n",
        f"{AUTO_END}\n\n### Security\n* GHSA-yyyy\n",
    )

    result = rebuild_changelog(
        version="0.31.0", date="2026-07-11",
        auto_notes="### Bug Fixes\n* fix A\n* fix B (new PR)\n",
        pr_changelog=pr_with_hand, origin_changelog=origin,
    )

    assert "* fix A" in result
    assert "* fix B (new PR)" in result
    assert "GHSA-yyyy" in result


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

    assert result.index("## [0.99.0] - 2026-07-11") < result.index("## [0.34.0]")
    assert AUTO_BEGIN in result
    assert "## [0.34.0]" in result
    assert "## [0.27.0]" in result
    assert "GHSA-395p-pj7r-jm42" in result
