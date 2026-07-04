# Security Policy

Fulgur converts untrusted HTML/CSS (and bundled SVG/image/font assets) into PDF,
often on a server that processes input from many tenants. We take security
reports for that threat model seriously and appreciate the effort of researchers
who help keep Fulgur's users safe.

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues,
pull requests, or discussions.**

Report privately through GitHub's
[private vulnerability reporting](https://github.com/fulgur-rs/fulgur/security/advisories/new)
("Report a vulnerability" under the repository's **Security** tab). This opens a
private advisory visible only to you and the maintainers.

If you are unable to use GitHub's private reporting, open a public issue that
asks a maintainer to establish a private channel — **without any vulnerability
details** — and we will follow up.

A good report includes:

- The affected version(s) and which surface is impacted
  (`fulgur` crate, `@fulgur-rs/cli`, `pyfulgur`, `fulgur-ruby`, WASM).
- A **minimal HTML/CSS input** (and any bundled assets) that reproduces the issue.
- The observed impact — crash/panic, unbounded memory or CPU, reading of
  unintended files, incorrect output that crosses a trust boundary, etc.
- Any relevant environment details (OS, Rust version, bundled fonts).

## Supported Versions

Fulgur is pre-1.0 and releases as a single lockstep version across all crates.
Only the **most recent published release** receives security fixes; there are no
backports to earlier `0.x` releases. Upgrading to the latest release is the
supported remediation path.

## Threat Model and Scope

Fulgur's primary use case is server-side conversion of **untrusted, attacker-
controlled HTML/CSS** in multi-tenant contexts. Reports are most valuable when
they demonstrate that such an input can harm the host or other tenants.

**In scope** (please report):

- Memory-safety issues (`unsafe` misuse, out-of-bounds, use-after-free) reachable
  from Fulgur's own code or its handling of parsed input.
- Denial of service that a caller cannot reasonably bound — pathological memory
  or CPU consumption, or unbounded recursion, triggered by crafted HTML/CSS/SVG
  or a malformed bundled asset.
- Panics reachable from library entry points (e.g. `Engine::render_html`) that a
  server cannot cleanly contain, when driven by untrusted input.
- Reading or exfiltration of files outside the intended asset scope via crafted
  asset references or path resolution.

**Out of scope / by design:**

- Rendering inaccuracy or CSS-support gaps that do not cross a trust boundary —
  file these as normal issues.
- Non-determinism from the documented system-font fallback caveat (see the
  README's *Determinism and fonts* section). This is a known limitation, not a
  vulnerability.
- html5ever parse-error noise printed to stdout by an upstream dependency
  (a cosmetic issue documented in the project's engineering notes).
- Vulnerabilities in a caller's own code, or issues that require the caller to
  feed already-trusted local paths/configuration that they control.

Fulgur is **offline by design**: it performs no network access, and all assets
must be explicitly bundled. Reports that rely on Fulgur fetching a remote
resource are therefore likely out of scope — but if you find a path that does
reach the network, that itself is a vulnerability worth reporting.

## Disclosure Process

We follow coordinated disclosure:

1. We acknowledge your report and work with you privately to confirm and assess
   it. Fixes for confirmed vulnerabilities are developed in a private fork so the
   details are not public before a patch exists.
2. When a fix is ready, we publish a release, a
   [GitHub Security Advisory (GHSA)](https://github.com/fulgur-rs/fulgur/security/advisories),
   and request a CVE where warranted. The advisory is propagated to the relevant
   ecosystem databases (e.g. RustSec / crates.io) so downstream `cargo audit`
   users are notified.
3. We credit reporters in the advisory unless you ask to remain anonymous.

Because this is a volunteer-maintained project, we respond and remediate on a
**best-effort basis** rather than a fixed SLA. We will keep you informed of
progress throughout.

Some low-severity, defense-in-depth hardening that we find ourselves may be
fixed directly in public pull requests without an embargo.

## Safe Harbor

We welcome good-faith security research conducted in accordance with this
policy. If you make a good-faith effort to comply with it — avoiding privacy
violations and service disruption, and giving us a reasonable opportunity to
remediate before public disclosure — we will treat your research as authorized
and work with you to resolve the issue quickly.
