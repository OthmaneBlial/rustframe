# Support Policy

RustFrame is currently a release-candidate project. The support policy is intentionally precise so that early adopters know which guarantees exist today.

## Supported versions

| Release line | Status | Security fixes | Compatibility fixes |
| --- | --- | --- | --- |
| Latest stable | Supported once published | Yes | Yes |
| Current `0.1.0-rc.*` | Supported until `0.1.0` ships | Yes | Best effort without silent contract breaks |
| Older release candidates | Upgrade required | Critical fixes only when practical | No |
| Unreleased `main` | Development only | No support promise | No support promise |

Pre-release versions may still change, but RustFrame documents migrations and runs API compatibility checks before doing so. A stable release will not be declared until the public registry, clean-install, package-signing, upgrade, and external usability gates in [ROADMAP.md](ROADMAP.md) are satisfied.

## Supported hosts

- macOS, Windows, and Linux receive framework CI, build, and native-package smoke coverage.
- The current Linux desktop runtime is X11-only. Native Wayland support is not yet claimed.
- Research Desk stable downloads will require Developer ID signing and notarization on macOS and Authenticode signing on Windows. Unsigned development artifacts are not presented as trusted end-user releases.

Host and package-manager versions used for a release are recorded in its verification receipts. When reporting a host issue, include `rustframe doctor --json` output after reviewing it for local paths you do not want to share.

## Getting help

- Use [GitHub Discussions](https://github.com/OthmaneBlial/rustframe/discussions) for setup questions, design discussion, and workflow ideas.
- Use the issue templates for reproducible bugs and bounded feature requests.
- Use GitHub private vulnerability reporting for suspected security issues; do not disclose them in a public issue.

The target is an initial maintainer response within five business days for complete bug reports and within three business days for complete security reports. These are response targets, not resolution guarantees.

## Privacy boundary

RustFrame does not add product telemetry to generated applications. Support information is shared only when a developer chooses to export or post it. Diagnostic bundles must redact sensitive paths before they are attached publicly.
