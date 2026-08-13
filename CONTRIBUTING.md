# Contributing to RustFrame

RustFrame welcomes focused bug fixes, documentation improvements, security hardening, and features that serve local-first workflow tools.

## Setup

Install Rust 1.88 or newer, Node.js 20 or newer, and the native WebView dependencies for your host. Then run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
npm --prefix packages/rustframe-api ci
npm --prefix packages/rustframe-api run build
npm --prefix apps/research-desk ci
npm --prefix apps/research-desk run build
```

Linux also needs GTK 3 and WebKitGTK development packages.

## Pull requests

- Keep changes scoped and add tests for observable behavior.
- Add a changelog entry for public API or behavior changes.
- Preserve stable error codes and deterministic generated output.
- Do not add broad filesystem or shell access to solve an app-specific problem.
- Explain platform testing gaps honestly; native packaging changes should be exercised on every affected host.

Public Rust, TypeScript, manifest, permission, and error-code changes require an explicit compatibility decision. Breaking changes are allowed before 1.0 but must include migration guidance.

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
