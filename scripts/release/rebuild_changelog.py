"""Rebuild CHANGELOG.md preserving hand-added marker-outside content.

See docs/plans/2026-07-11-changelog-marker-ssot.md and bd show fulgur-v94v.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

HEADER = (
    "# Changelog\n\n"
    "All notable changes to this project will be documented in this file.\n"
)

AUTO_BEGIN = "<!-- release-notes:auto:begin -->"
AUTO_END = "<!-- release-notes:auto:end -->"

_SECTION_HEAD = re.compile(r"^## \[[^\]]+\]", re.M)


def _extract_history(changelog: str, drop_version: str | None = None) -> str:
    """Return everything from the first `## [` heading to EOF, unchanged.

    If `drop_version` is given and a matching `## [drop_version]` section exists,
    that section is skipped (heading through the next `## [` or EOF). This
    guards against duplication when `origin_changelog` already contains the
    current release's section (workflow re-run after merge, or unusual manual
    dispatch).
    """
    lines = changelog.splitlines(keepends=True)
    start = next((i for i, ln in enumerate(lines) if ln.startswith("## [")), None)
    if start is None:
        return ""
    if drop_version is None:
        return "".join(lines[start:])

    prefix = f"## [{drop_version}]"
    out: list[str] = []
    skipping = False
    for line in lines[start:]:
        if line.startswith("## ["):
            skipping = line.startswith(prefix)
        if not skipping:
            out.append(line)
    return "".join(out)


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
    history = _extract_history(origin_changelog, drop_version=version)
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


def _read_file(path: str) -> str:
    if path == "-":
        return sys.stdin.read()
    return Path(path).read_text(encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Rebuild CHANGELOG.md preserving marker-outside hand-adds."
    )
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
