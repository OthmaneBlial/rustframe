# Changelog

All notable RustFrame changes are documented here. RustFrame follows [Semantic Versioning](https://semver.org/), with coordinated versions for `rustframe-runtime`, `rustframe-cli`, `rustframe-api`, and manifest schema compatibility.

## [Unreleased]

### Changed

- Moved the public manifest schema to the durable GitHub Pages URL and taught `rustframe migrate` to replace the retired `rustframe.dev` URL.
- Made prebuilt release binaries the primary documented CLI installation path.
- Rebuilt the product site, documentation browser, and showcase around a responsive local-workbench visual system with self-hosted fonts and optimized WebP proof media.

### Added

- CI checks that compile TypeScript documentation examples, verify local links, and keep published docs and schemas synchronized.
- A cross-platform public-artifact smoke workflow that exercises the registry-only quickstart after a release is published.
- An interactive least-privilege policy explorer, accessible installation tabs, keyboard-safe mobile navigation, and task-filterable mirrored documentation.
- Canonical metadata, structured data, social previews, sitemap, robots policy, `llms.txt`, and an automated site contract covering local links, image budgets, headings, and metadata.

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

[Unreleased]: https://github.com/OthmaneBlial/rustframe/compare/v0.1.0-rc.1...HEAD
[0.1.0-rc.1]: https://github.com/OthmaneBlial/rustframe/releases/tag/v0.1.0-rc.1
