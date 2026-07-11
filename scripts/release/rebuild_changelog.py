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
