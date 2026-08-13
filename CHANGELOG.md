# Changelog

All notable RustFrame changes are documented here. RustFrame follows [Semantic Versioning](https://semver.org/), with coordinated versions for `rustframe-runtime`, `rustframe-cli`, `rustframe-api`, and manifest schema compatibility.

## [Unreleased]

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

[Unreleased]: https://github.com/OthmaneBlial/rustframe/compare/v0.1.0...HEAD
