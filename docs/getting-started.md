# Getting Started

## What RustFrame Ships

RustFrame is a Rust workspace with two main moving parts:

- A runtime crate that owns the desktop window, the custom `app://localhost/` protocol, the native IPC handler, and the optional SQLite, filesystem, and shell capabilities.
- A CLI that scaffolds frontend-first apps, generates the hidden Rust runner under `target/` for the simple path, and can eject an app-owned runner when you need native control.

## Prerequisites

- Rust and Cargo
- A Linux desktop setup that already satisfies the GTK and WebKitGTK requirements used by `wry`

RustFrame is currently Linux-first.

## Run The Capability Demo

```bash
cargo run -p capability-demo
```

To point the runtime at a frontend dev server instead of embedded assets:

```bash
RUSTFRAME_DEV_URL=http://127.0.0.1:5173 cargo run -p capability-demo
```

## Create A Frontend-First App

Generate a new app into `apps/<name>`:

```bash
cargo run -p rustframe-cli -- new hello-rustframe
```

RustFrame writes a plain frontend folder. The generated app does not contain a visible `Cargo.toml`, `src/`, or runner files.

Edit these files directly:

- `apps/hello-rustframe/index.html`
- `apps/hello-rustframe/styles.css`
- `apps/hello-rustframe/app.js`
- `apps/hello-rustframe/assets/icon.svg`
- `apps/hello-rustframe/rustframe.json`
- `apps/hello-rustframe/data/schema.json`
- `apps/hello-rustframe/data/seeds/*.json`
- `apps/hello-rustframe/data/migrations/*.sql`

Use `rustframe.json` as the primary typed config for window settings, dev URLs, capabilities, and packaging. `<title>` plus `rustframe:*` meta tags still work as fallback.

The native bridge is injected by the runtime, so frontend-only apps do not need to ship a `bridge.js` file.

## Run An App In Development

From the workspace root:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe
```

From inside the app directory:

```bash
cd apps/hello-rustframe
cargo run -p rustframe-cli -- dev
```

To use a frontend dev server:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe http://127.0.0.1:5173
```

Prefer `rustframe.json`:

```json
{
  "window": {
    "title": "Hello Rustframe",
    "width": 1280,
    "height": 820
  },
  "devUrl": "http://127.0.0.1:5173"
}
```

HTML fallback still works when you want a minimal setup:

```html
<title>Hello Rustframe</title>
<meta name="rustframe:width" content="1280">
<meta name="rustframe:height" content="820">
<meta name="rustframe:dev-url" content="http://127.0.0.1:5173">
```

## Declare Native Capabilities

Frontend-only apps can declare filesystem roots and allowlisted shell commands in `rustframe.json`:

```json
{
  "appId": "hello-rustframe",
  "filesystem": {
    "roots": ["fixtures", "${EXE_DIR}/imports"]
  },
  "shell": {
    "commands": [
      {
        "name": "listFixtures",
        "program": "ls",
        "args": ["-la", "${SOURCE_APP_DIR}/fixtures"]
      }
    ]
  }
}
```

Supported path tokens:

- `${SOURCE_APP_DIR}` resolves to the source app folder.
- `${SOURCE_ASSET_DIR}` resolves to the embedded asset folder.
- `${EXE_DIR}` resolves to the runtime executable directory.

Linux packaging also reads from `rustframe.json`:

```json
{
  "packaging": {
    "version": "0.1.0",
    "description": "Hello RustFrame desktop app",
    "linux": {
      "icon": "assets/icon.svg",
      "categories": ["Utility"],
      "keywords": ["desktop", "rustframe"]
    }
  }
}
```

## Evolve The Database Safely

Use the database files with these roles:

- `data/schema.json` is the latest desired schema.
- `data/seeds/*.json` is first-run data and should stay immutable once applied.
- `data/migrations/*.sql` is for versioned upgrades, data backfills, column renames, drops, and type changes.

Example:

```text
data/migrations/002-rename-title.sql
```

Migration files are applied in schema-version order before RustFrame runs its additive schema reconciliation.

## Export A Release Build

From the workspace root:

```bash
cargo run -p rustframe-cli -- export hello-rustframe
```

From inside the app directory:

```bash
cd apps/hello-rustframe
cargo run -p rustframe-cli -- export
```

RustFrame generates a hidden runner in:

```text
target/rustframe/apps/<name>/runner/
```

The release binary is copied into:

```text
apps/<name>/dist/
```

Use `export` when you want the raw executable only.

## Package A Linux Bundle

From the workspace root:

```bash
cargo run -p rustframe-cli -- package hello-rustframe
```

From inside the app directory:

```bash
cd apps/hello-rustframe
cargo run -p rustframe-cli -- package
```

RustFrame writes:

```text
apps/<name>/dist/linux/<app-id>-<version>-linux-<arch>/
apps/<name>/dist/linux/<app-id>-<version>-linux-<arch>.tar.gz
```

The bundle contains:

- a portable `*.AppDir`
- a desktop entry and icon
- `install.sh` and `uninstall.sh` for per-user installs
- `rustframe-package.json` with release metadata

## Eject To A Native Runner

When you need tray work, deeper `tao` or `wry` configuration, extra native crates, or other runtime customization, eject the app:

```bash
cargo run -p rustframe-cli -- eject hello-rustframe
```

That creates an app-owned Rust project in:

```text
apps/<name>/native/
```

After that:

- `cargo run -p rustframe-cli -- dev <name>` uses the ejected runner automatically.
- `cargo run -p rustframe-cli -- export <name>` builds from the ejected runner automatically.
- `cargo run -p rustframe-cli -- package <name>` builds the Linux bundle from the ejected runner automatically.
- The ejected runner stays backed by the `rustframe` library instead of copying the runtime into your app.

Stay on the hidden-runner path when the default runtime is enough. Eject when the app genuinely needs native customization.

## What To Read Next

- [Runtime And Capabilities](./runtime-and-capabilities.md)
- [Frontend App Rules](./frontend-app-rules.md)
- [Example Apps](./example-apps.md)
