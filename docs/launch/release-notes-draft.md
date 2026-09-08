# Research Desk and RustFrame — release preparation

Status: local preview from the working tree based on `bae1321`. Not published, signed, notarized or declared stable. Package metadata remains `0.1.0-rc.2`; these local builds include unreleased fixes and must not replace assets on the existing RC2 tag.

## What changes for users

Research Desk keeps its first screen compact, limits the document queue to the selected workspace and selects the matching document when searching. Typing continues uninterrupted while native search results arrive. Reader windows now open with the intended permissions, focus an existing reader for the same document and synchronize saved review notes with the main window.

Restore validation opens source databases read-only: a missing backup no longer creates a misleading empty database file. The public quickstart gate fails when the exact frontend API package is absent, including for candidates.

## Local artifacts

The delivery directory will contain the macOS ARM app/DMG, frontend API tarball, runtime crate, checksums, dependency inventory, validation summary and real product media. Consult each verification receipt for its exact scope. A checksum or SBOM does not imply a trusted signature.

## Validation

- Workspace Rust tests, Clippy and packaged runtime verification; detailed current results in `docs/validation/roadmap-execution.md`.
- All five starters: independent frontend projects, local-tarball installation, native build/runtime initialization on macOS, exact registry dependency after ejection.
- Research Desk: actual native file picker, index/search/read, saved note across relaunch, reader synchronization, JSONL export, file watcher, rename/delete, unreadable file, empty folder and revocation.
- Packaged native process: local indexing/read/write while external sockets are denied and loopback IPC remains allowed.
- Temporary app and DMG installation/runtime initialization/uninstallation on macOS ARM.

## Upgrade and data

These changes do not change the manifest schema, database schema or public API. Existing notes remain in SQLite; source files remain in their selected directories. Records from other workspaces are preserved and no longer appear in the active queue. Back up the database before adopting a candidate. Database upgrade, identity validation and rollback rejection are covered by the existing workflow tests; a fresh cross-platform installed-version upgrade matrix remains a release gate.

## Before publication

1. Choose a new coordinated candidate version for runtime, CLI and API; update generated package requirements and rebuild from a committed source revision.
2. Obtain explicit publication authorization and complete npm/crates.io publication. npm is currently unavailable and crates.io still exposes RC1.
3. Run exact public-registry quickstart and native artifact checks on all supported hosts.
4. Supply Apple/Windows signing identities and verify transported signatures, SBOM and provenance for a trusted Research Desk release.
5. Upload the reviewed media, update README/site to the real final URLs and inspect their rendered players.
6. Publish outcome-focused notes and recruit the opt-in builder pilot only when the public entry gate works.

No release, upload, outreach or remote CI dispatch has been performed by preparing these notes.
