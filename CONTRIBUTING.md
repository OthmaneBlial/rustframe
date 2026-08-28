# Contributing to RustFrame

RustFrame welcomes focused fixes, documentation, security hardening, workflow templates, and runtime improvements that preserve its local-first contract. You do not need private guidance to start: every `ready-for-contributor` issue must include the affected area, acceptance criteria, an exact test command, and explicit non-goals.

## Find the right contribution

- `good first issue` means one bounded change with a maintainer-approved design.
- `help wanted` means the scope is accepted but may require deeper repository knowledge.
- `ready-for-contributor` means implementation can begin now.
- `needs-design` or `needs-reproduction` means discuss or reproduce before writing code.
- `maintainer-only` marks release, credential, compatibility, or security-boundary work that is not safe to claim casually.
- `area-runtime`, `area-cli`, `area-docs`, and `area-templates` identify the owning component.

Use [GitHub Discussions](https://github.com/OthmaneBlial/rustframe/discussions) for questions, use cases, and early ideas. An accepted design becomes an issue before implementation. Report vulnerabilities through a [private security advisory](https://github.com/OthmaneBlial/rustframe/security/advisories/new), never a public issue.

## Local setup

Install Rust 1.88 or newer, Node.js 22 or newer, and Git. Linux also needs GTK 3 and WebKitGTK 4.1 development packages. Clone your fork, then create a branch from current `main`.

Run the smallest check that proves your change:

```bash
# Documentation or website contracts
node scripts/check_public_contracts.mjs
node scripts/check_site.mjs

# Template registry and every fixed template verification profile
./scripts/verify_templates.sh

# Frontend API
npm --prefix packages/rustframe-api ci
npm --prefix packages/rustframe-api run check
npm --prefix packages/rustframe-api test

# Rust workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Native packaging changes must also pass the six-host-format `Native package smoke` matrix. A local package or one operating system is not cross-platform proof.

## Repository map

| Area | Paths | Minimum proof |
| --- | --- | --- |
| Runtime and native bridge | `crates/rustframe/`, `crates/rustframe/tests/` | focused Rust test, workspace Clippy |
| CLI, manifests, codegen | `crates/rustframe-cli/`, `schemas/` | CLI tests, generated fixtures, schema mirror |
| Frontend API | `packages/rustframe-api/` | typecheck, Node tests, package dry-run |
| First-party workflows | `apps/` | frontend build, `rustframe validate`, relevant browser test |
| Template registry | `apps/*/.rustframe/template.json`, `examples/community-templates/` | `./scripts/verify_templates.sh` |
| Public site and docs | `site/`, `docs/` | contract check, Playwright, Lighthouse when layout changes |
| Release and supply chain | `.github/workflows/`, `scripts/*release*` | dry-run receipts; maintainer review required |

## Pull-request contract

- Keep one user-visible outcome per pull request and link the accepted issue.
- Add tests for observable behavior and state exactly which commands passed.
- Add a changelog entry for public API or behavior changes.
- Preserve stable error codes, deterministic generated output, and immutable public schemas.
- Do not add broad filesystem, shell, or network access to solve an app-specific problem.
- State untested platforms and signing gaps honestly.
- Public Rust, TypeScript, manifest, permission, and error-code changes require an explicit compatibility decision and migration guidance.

Maintainers may ask to split a change when review, rollback, or security impact cannot be evaluated independently.

## Template contributions

Template metadata is declarative. The catalog accepts only a path to a versioned `.rustframe/template.json`; it cannot provide commands, scripts, or CI steps. Verification uses maintainer-owned profiles and rejects unknown fields, traversal, stale runtime versions, missing licenses, missing screenshots, or incomplete platform claims.

Read the [template contract](docs/community-templates.md), open the [template submission form](https://github.com/OthmaneBlial/rustframe/issues/new?template=template_submission.yml), and run:

```bash
./scripts/verify_templates.sh
```

The current showcase is explicitly first-party. A community entry is credited to its author, and the site will not imply a broader ecosystem until at least three external applications or templates pass the same public contract.

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
