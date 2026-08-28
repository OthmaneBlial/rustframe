# RustFrame Docs

RustFrame is a Rust workspace for local-first desktop workflow tools that should stay mostly frontend code. This folder collects the repo's working contract in one place so the public site, the examples, and the developer workflow all point at the same source material.

## Start Here

- [Getting Started](./getting-started.md)
- [Choosing RustFrame](./choosing-rustframe.md)
- [Architecture Overview](./architecture-overview.md)
- [Runtime And Capabilities](./runtime-and-capabilities.md)
- [Build In 20 Minutes](./build-in-20-minutes.md)
- [Cookbook](./cookbook.md)
- [Workflow Guides](./workflow-guides.md)
- [Research Desk Architecture Case Study](./research-desk-architecture.md)
- [Frontend API Reference](./api-reference.md)
- [Manifest Reference](./manifest-reference.md)
- [Troubleshooting](./troubleshooting.md)
- [Threat Model](./threat-model.md)
- [Migrations And Versioning](./migrations-and-versioning.md)
- [Platform Support](./platform-support.md)
- [Signing And Notarization](./signing-and-notarization.md)
- [Update Strategy](./update-strategy.md)
- [Release Checklist](./release-checklist.md)
- [Community Templates](./community-templates.md)
- [Remote Sync Patterns](./remote-sync-patterns.md)
- [Capability Extension Patterns](./capability-extension-patterns.md)
- [Frontend App Rules](./frontend-app-rules.md)
- [Example Apps](./example-apps.md)

## Workspace Map

- `crates/rustframe` is the reusable runtime crate.
- `crates/rustframe-cli` publishes the `rustframe` binary for standalone project creation, validation, building, migration, diagnostics, and packaging.
- `packages/rustframe-api` is the typed npm bridge package used by generated TypeScript projects.
- `schemas/v1/rustframe.schema.json` is the immutable manifest schema v1 contract.
- `examples/community-templates/catalog.json` is the machine-readable ecosystem catalog for workflow-shaped starting points.
- `examples/capability-demo` proves embedded assets, native IPC, sandboxed filesystem access, and allowlisted shell execution.
- `apps/research-desk` is the flagship user-selected document workflow and the main proof app for the repo.
- `apps/*` contains frontend-only desktop apps with root-level `index.html`, `styles.css`, `app.js`, optional `data/`, raw binaries in `dist/`, and platform bundles in `dist/<platform>/`.

## Repo Sources

- Project overview: [README.md](https://github.com/OthmaneBlial/rustframe/blob/main/README.md)
- App contract: [FRONTEND_APP_RULES.md](https://github.com/OthmaneBlial/rustframe/blob/main/FRONTEND_APP_RULES.md)
- Repository root: [github.com/OthmaneBlial/rustframe](https://github.com/OthmaneBlial/rustframe)

This `docs/` folder reorganizes that material into shorter product and implementation guides.
