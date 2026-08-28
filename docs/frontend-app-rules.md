# RustFrame Frontend Project Contract

This document defines the semver-governed standalone project contract for manifest schema v1.

## Required layout

```text
project/
├── rustframe.json
├── package.json
├── index.html
├── src/
├── data/
│   ├── schema.json
│   ├── seeds/
│   └── migrations/
├── public/
└── assets/
```

Projects may live anywhere. Commands resolve the nearest `rustframe.json`, or the path supplied with `rustframe --project <path>`. `target/rustframe/` and `dist/` are generated output. `native/` exists only after `rustframe eject`.

## Manifest

- Set `$schema` to `https://othmaneblial.github.io/rustframe/schemas/v1/rustframe.schema.json` and `schemaVersion` to `1`. Projects created with the retired release-candidate URL can update it with `rustframe migrate`.
- Unknown fields, undeclared windows, unknown or duplicate permissions, unsafe relative paths, missing assets, invalid commands, and incompatible trust settings are errors.
- Keep a restrictive CSP in `security.csp` and the matching `index.html` meta tag.
- Use exact window IDs or one trailing wildcard, such as `reader-*`, for permission scopes.
- A `networked` renderer may not receive database, filesystem, or shell permissions.
- Shell capabilities name an executable and bounded fixed/allowlisted arguments; they are not shell source strings.

Run `rustframe validate` before development and in CI. `rustframe inspect --json` exposes the resolved contract for automation.

## Frontend API

TypeScript projects import `getRustFrame` and public types from `rustframe-api`. `rustframe codegen` creates the table map and `AppRustFrameClient` from `data/schema.json`. Generated output is deterministic and should be committed; validation fails when it is stale.

Plain JavaScript can use the same package without types or the injected `window.RustFrame` global. Native IPC remains the authorization boundary regardless of which JavaScript entry point is used.

## Local files

Use `grant://` URIs returned after an explicit user selection or `root://` URIs for narrow declared roots. Do not persist a grant without clear consent. Do not store arbitrary absolute paths as application identity. Watchers belong to their creating window and stop when that window closes or the grant is revoked.

## Database

RustFrame owns `id`, `createdAt`, and `updatedAt`. Seeds are immutable first-run data. Versioned SQL migrations perform non-additive changes. Use `db.batch` when mutations must commit or roll back together; database change events are emitted only after commit.

Backup and restore go through native dialogs. Restore validates app identity and schema compatibility and creates a safety snapshot before replacing active state.

## Build and packaging

`rustframe build` runs `frontend.buildCommand` before compiling the hidden native runner. `rustframe package --verify` uses manifest packaging metadata, preserves declared resources, and emits native artifacts, checksums, a package manifest, and release notes under `dist/packages/`.

Generated runners depend on the exact compatible registry release of `rustframe-runtime`. `RUSTFRAME_RUNTIME_PATH` is reserved for RustFrame's own integration tests and must never appear in a generated public project.
