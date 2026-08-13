# RustFrame

RustFrame is a narrow, local-first desktop framework for TypeScript and JavaScript tools. It supplies a native window, SQLite, capability-scoped filesystem access, bounded automation, multi-window events, and host-native packaging without making the application itself a Rust project.

## Quickstart

```bash
cargo install rustframe-cli
rustframe new my-tool
cd my-tool
npm install
rustframe dev
```

The default template is Vite with vanilla TypeScript. React, Vue, Svelte, and plain JavaScript are also supported:

```bash
rustframe new my-react-tool --template react-ts --package-manager npm --install
rustframe new no-typescript --template vanilla-js
```

Each project is independent and can live anywhere:

```text
my-tool/
├── rustframe.json
├── package.json
├── index.html
├── src/
│   ├── main.ts
│   └── rustframe.generated.ts
├── data/
│   ├── schema.json
│   ├── seeds/
│   └── migrations/
├── public/
└── assets/
```

RustFrame locates the nearest `rustframe.json`. Generated native runners live under `target/rustframe/` and depend on the exact compatible `rustframe-runtime` registry release. Use `rustframe eject` only when the project needs to own its native Rust code.

## Stable commands

```text
rustframe new
rustframe doctor
rustframe dev
rustframe validate
rustframe inspect
rustframe codegen
rustframe build
rustframe package
rustframe db reset
rustframe db backup
rustframe db restore
rustframe migrate
rustframe eject
```

Use `--project <path>` for monorepos:

```bash
rustframe --project apps/research-desk validate
```

## Project contract

`rustframe.json` schema version 1 is the public native contract. Unknown fields, unsafe paths, missing assets, undeclared permissions, stale generated database types, and incompatible trust settings fail `rustframe validate`.

```json
{
  "$schema": "https://rustframe.dev/schemas/v1/rustframe.schema.json",
  "schemaVersion": 1,
  "app": {
    "id": "my-tool",
    "title": "My Tool",
    "windows": [{ "id": "main", "route": "/" }]
  },
  "frontend": {
    "devCommand": "npm run dev -- --host 127.0.0.1",
    "buildCommand": "npm run build",
    "devUrl": "http://127.0.0.1:5173",
    "distDir": "dist",
    "generatedTypes": "src/rustframe.generated.ts"
  },
  "security": {
    "model": "local-first",
    "csp": "default-src 'self'; object-src 'none'; base-uri 'none'",
    "permissions": [{
      "window": "main",
      "allow": ["db:read", "db:write", "fs:grants:read", "dialog:open"]
    }]
  },
  "database": {
    "schema": "data/schema.json",
    "seeds": "data/seeds",
    "migrations": "data/migrations"
  },
  "filesystem": { "roots": [], "persistGrants": true },
  "shell": { "commands": [] },
  "packaging": {
    "version": "0.1.0",
    "identifier": "dev.example.my-tool",
    "icon": "assets/icon.svg"
  }
}
```

The published `rustframe-api` npm package supplies bridge availability checks, stable errors, complete frontend types, and generated table-aware database clients. Plain JavaScript can continue using the injected `window.RustFrame` global.

## Local workflow APIs

- Opaque `grant://` and `root://` filesystem URIs; absolute paths are not the primary frontend API.
- Persistent or temporary user-selected file and folder grants, recursive walking, revocation, and watchers.
- Atomic SQLite batches, full-text search, migrations, cross-window mutation events, online backups, and safety-backed restore.
- Exact or prefix-pattern window permissions enforced by native IPC.
- Named shell commands with fixed/allowlisted arguments, timeouts, output limits, and redacted audit records.
- Multiple native windows with shared database and restore events.

## Build and package

```bash
rustframe validate
rustframe build
rustframe package --verify
```

Packaging uses `cargo-packager` and creates the host formats:

- macOS: `.app` and DMG
- Windows: NSIS and MSI
- Linux: AppImage and Debian package

Use `--format` for one format, such as `rustframe package --format app --verify`. Local packages are explicitly marked unsigned and include `SHA256SUMS`, a machine-readable package manifest, and release notes. Signing and notarization are release-pipeline concerns; automatic updating is deliberately outside v1.

## Research Desk

[`apps/research-desk`](apps/research-desk) is the end-to-end proof app. It uses only public RustFrame APIs to let the user select a document folder, persist consented access, index Markdown/text without Python, watch incremental changes, search, tag, review, pin, open synchronized reader windows, switch recent workspaces, export queues, and back up or restore SQLite.

Run it from this repository with the integration-only local runtime override:

```bash
export RUSTFRAME_RUNTIME_PATH="$PWD/crates/rustframe"
cargo run -p rustframe-cli -- --project apps/research-desk dev
```

## Scope

RustFrame is intentionally smaller than Tauri or Electron. Choose it for local-first workflow tools whose frontend is trusted and bundled. Choose a broader framework when you need a plugin ecosystem, mobile targets, tray APIs, notifications, global shortcuts, or built-in updating. Those features are not part of public v1.

## Documentation

- [Getting started](docs/getting-started.md)
- [Runtime and capabilities](docs/runtime-and-capabilities.md)
- [Threat model](docs/threat-model.md)
- [Platform support](docs/platform-support.md)
- [Signing and notarization](docs/signing-and-notarization.md)
- [Release checklist](docs/release-checklist.md)
- [Security policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)

RustFrame collects no telemetry by default. Public runtime, CLI, manifest, and frontend API compatibility follow semantic versioning.
