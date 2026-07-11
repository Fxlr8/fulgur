"""Rebuild CHANGELOG.md preserving hand-added marker-outside content.

See docs/plans/2026-07-11-changelog-marker-ssot.md and bd show fulgur-v94v.
"""
from __future__ import annotations

import re
import sys

HEADER = (
    "# Changelog\n\n"
    "All notable changes to this project will be documented in this file.\n"
)

AUTO_BEGIN = "<!-- release-notes:auto:begin -->"
AUTO_END = "<!-- release-notes:auto:end -->"

_SECTION_HEAD = re.compile(r"^## \[[^\]]+\]", re.M)


def _extract_history(changelog: str) -> str:
    """Return everything from the first `## [` heading to EOF, unchanged."""
    lines = changelog.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if line.startswith("## ["):
            return "".join(lines[i:])
    return ""


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
        return ("", section.strip("\n"), False)
    preamble = section[:begin_idx].strip("\n")
    postamble = section[end_idx + len(AUTO_END):].strip("\n")
    return (preamble, postamble, True)


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

    parts = [f"## [{version}] - {date}\n"]
    if preamble:
        parts.append(f"\n{preamble}\n")
    parts.append(f"\n{AUTO_BEGIN}\n{auto_notes.rstrip()}\n{AUTO_END}\n")
    if postamble:
        parts.append(f"\n{postamble}\n")
    section_out = "".join(parts)

    return HEADER + "\n" + section_out + "\n" + history
