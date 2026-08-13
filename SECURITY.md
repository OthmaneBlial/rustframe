# Security Policy

## Supported versions

Security fixes are provided for the latest stable RustFrame release. Release candidates receive fixes until the corresponding stable release ships.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Use GitHub's private vulnerability reporting for this repository. Include the affected version, platform, reproduction steps, impact, and any proposed mitigation.

We aim to acknowledge a complete report within three business days. We will coordinate validation, remediation, an advisory, and disclosure timing with the reporter. Please avoid accessing data that is not yours or disrupting services while researching.

## Security baseline

RustFrame denies undeclared native operations in native IPC. Applications should bundle trusted frontend assets, declare a restrictive Content Security Policy, grant the smallest per-window permission set, expose only named bounded commands, and use opaque filesystem URIs. Remote production navigation is not a supported local-first configuration.

See [the threat model](docs/threat-model.md) for trust boundaries and known limits.
