# scripts/release/

Release automation helpers invoked from `.github/workflows/release-plz.yml` and
`.github/workflows/release.yml`.

## `rebuild_changelog.py`

Rebuilds `CHANGELOG.md` for a release, preserving hand-added notes that live
**outside** the `<!-- release-notes:auto:begin -->` /
`<!-- release-notes:auto:end -->` marker pair. Called by `release-plz.yml`
aux-sync on every push to `main` while a Release PR is open.

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
    - Put the note **outside** the marker pair — either above
      `<!-- release-notes:auto:begin -->` or below
      `<!-- release-notes:auto:end -->`.
    - Do **not** edit content between the markers; aux-sync overwrites it.
3. Commit and push. aux-sync (release-plz.yml) preserves the hand-add on
   subsequent runs.

### Missing-markers fallback

If a section lacks the marker pair, aux-sync logs `::warning::` and treats all
content as postamble (safe: no data loss). Add the markers manually to control
regeneration.
