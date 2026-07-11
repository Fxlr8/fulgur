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
