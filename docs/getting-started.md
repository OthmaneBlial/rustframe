# Getting Started

## What RustFrame Ships

RustFrame is a Rust workspace with two main moving parts:

- A runtime crate that owns the desktop window, the custom `app://localhost/` protocol, the native IPC handler, and the optional SQLite, filesystem, and shell capabilities.
- A CLI that scaffolds frontend-first apps and generates the hidden Rust runner under `target/` when you run `dev` or `export`.

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
- `apps/hello-rustframe/bridge.js`
- `apps/hello-rustframe/rustframe.json`
- `apps/hello-rustframe/data/schema.json`
- `apps/hello-rustframe/data/seeds/*.json`

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

You can also declare the dev server in `index.html` with:

```html
<meta name="rustframe:dev-url" content="http://127.0.0.1:5173">
```

Or in `rustframe.json`:

```json
{
  "devUrl": "http://127.0.0.1:5173"
}
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

## What To Read Next

- [Runtime And Capabilities](./runtime-and-capabilities.md)
- [Frontend App Rules](./frontend-app-rules.md)
- [Example Apps](./example-apps.md)
