# Changelog

All notable RustFrame changes are documented here. RustFrame follows [Semantic Versioning](https://semver.org/), with coordinated versions for `rustframe-runtime`, `rustframe-cli`, `rustframe-api`, and manifest schema compatibility.

## [Unreleased]

## [0.1.0-rc.2] - 2026-08-28

### Changed

- Moved the public manifest schema to the durable GitHub Pages URL and taught `rustframe migrate` to replace the retired `rustframe.dev` URL.
- Made prebuilt release binaries the primary documented CLI installation path.
- Rebuilt the product site, documentation browser, and showcase around a responsive local-workbench visual system with self-hosted fonts and optimized WebP proof media.
- Reworked the native runtime around explicit local ownership, observable development startup, background database and filesystem work, and one-instance document routing.
- Made Research Desk indexing incremental and benchmarkable while adding cancellation, recoverable errors, FTS5 search, review organization, synchronized readers, and portable exports.

### Added

- CI checks that compile TypeScript documentation examples, verify local links, and keep published docs and schemas synchronized.
- A cross-platform public-artifact smoke workflow that exercises the registry-only quickstart after a release is published.
- An interactive least-privilege policy explorer, accessible installation tabs, keyboard-safe mobile navigation, and task-filterable mirrored documentation.
- Canonical metadata, structured data, social previews, sitemap, robots policy, `llms.txt`, and an automated site contract covering local links, image budgets, headings, and metadata.
- A schema-to-TypeScript workbench whose golden fixture is checked against the Rust CLI, including a locally generated runnable starter ZIP.
- Generated frontend API and manifest references, full-text documentation search, RC version visibility, previous/next navigation, troubleshooting, and workflow-shaped guides.
- Playwright desktop/mobile journeys and a Lighthouse CI gate requiring 95+ category scores, LCP at or below 2.5 seconds, and CLS at or below 0.1.
- Capability inspection and deny-expansion checks, portable JSON/JSONL/CSV export, redacted diagnostics, and transported release verification receipts.
- Native document associations for macOS, Windows, and Linux plus install, launch, offline, and uninstall smoke coverage for `.app`, `.dmg`, NSIS, MSI, AppImage, and Debian packages.
- A schema-validated registry of seven first-party workflow templates with fixed verification profiles, real previews, executable source, and contributor submission guidance.
- GitHub Discussions, a contributor-ready issue queue, `CODEOWNERS`, Dependabot, CodeQL for Rust/TypeScript/workflows, OpenSSF Scorecard, immutable Action pins, and least-privilege workflow policy enforcement.

### Known limitations

- `rustframe-api` still requires its initial npm publication with 2FA or Trusted Publishing before the registry-only generated-project path is supported.
- Research Desk end-user downloads are not declared trusted until Developer ID and Authenticode credentials are available and the signed release workflow passes on downloaded artifacts.
- Linux desktop support remains X11-only; native Wayland support is not claimed by this candidate.

## [0.1.0-rc.1] - 2026-08-13

### Added

- Standalone Vite project creation for TypeScript, JavaScript, React, Vue, and Svelte.
- Manifest schema v1, deterministic database type generation, and the typed `rustframe-api` frontend package.
- Window-scoped permissions, opaque filesystem grants, watchers, atomic database batches, change events, online backup, and safety-backed restore.
- `cargo-packager` integration for macOS, Windows, and Linux native artifacts.
- Research Desk as the public API end-to-end example.

### Changed

- The public runtime crate is named `rustframe-runtime` while its Rust import remains `rustframe`.
- The CLI binary is named `rustframe` and discovers the nearest `rustframe.json`.

### Removed

- Repository-relative generated runner dependencies and the `apps/` parent-directory requirement.

[Unreleased]: https://github.com/OthmaneBlial/rustframe/compare/v0.1.0-rc.2...HEAD
[0.1.0-rc.2]: https://github.com/OthmaneBlial/rustframe/compare/v0.1.0-rc.1...v0.1.0-rc.2
[0.1.0-rc.1]: https://github.com/OthmaneBlial/rustframe/releases/tag/v0.1.0-rc.1
