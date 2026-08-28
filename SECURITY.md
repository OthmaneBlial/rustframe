# Security Policy

## Supported versions

Security fixes are provided for the latest stable RustFrame release. Release candidates receive fixes until the corresponding stable release ships.

See [SUPPORT.md](SUPPORT.md) for the complete version, platform, and response policy.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Use GitHub's private vulnerability reporting for this repository. Include the affected version, platform, reproduction steps, impact, and any proposed mitigation.

We aim to acknowledge a complete report within three business days. We will coordinate validation, remediation, an advisory, and disclosure timing with the reporter. Please avoid accessing data that is not yours or disrupting services while researching.

## Security baseline

RustFrame denies undeclared native operations in native IPC. Applications should bundle trusted frontend assets, declare a restrictive Content Security Policy, grant the smallest per-window permission set, expose only named bounded commands, and use opaque filesystem URIs. Remote production navigation is not a supported local-first configuration.

Repository automation uses read-only workflow permissions by default, full-length commit pins for every external Action, and checkout steps that do not persist credentials. CI enforces that policy with `scripts/check_workflow_security.mjs` and a checksum-verified Actionlint binary. Dependabot proposes Cargo, npm, and GitHub Actions updates for review; dependency policy, CodeQL, and OpenSSF Scorecard run independently.

Release jobs elevate only the permissions needed by the publishing job. Framework artifacts receive GitHub build provenance, and the trusted Research Desk pipeline adds downloaded-artifact verification, SHA-256 checksums, an SPDX SBOM, host signature evidence, and a protected `release` environment. A generated SBOM or a passing workflow does not substitute for signed end-user packages.

See [the threat model](docs/threat-model.md) for trust boundaries and known limits.
