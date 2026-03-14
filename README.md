# RustFrame

RustFrame is a stripped-down desktop application framework in Rust built around `tao` and `wry`, without Tauri's CLI, plugin layer, or localhost IPC stack.

## Workspace

- `crates/rustframe`: reusable runtime crate
- `crates/rustframe-cli`: scaffolding and export tooling
- `examples/capability-demo`: sample app showing the embedded frontend, native IPC, filesystem access, and allowlisted shell execution
- `apps/*`: frontend-only desktop apps with root-level `html/css/js` and `dist/`
- `base/`: hidden legacy C++ baseline, intentionally ignored by Git

## Run the demo

```bash
cargo run -p capability-demo
```

In development you can point the runtime at a frontend dev server:

```bash
RUSTFRAME_DEV_URL=http://127.0.0.1:5173 cargo run -p capability-demo
```

## Create an app

Generate a new app into `apps/<name>`:

```bash
cargo run -p rustframe-cli -- new hello-rustframe
```

Each app is just a frontend folder. There is no visible `Cargo.toml`, `rustframe.toml`, or `src/` inside the app.

Edit:

- `apps/hello-rustframe/index.html`
- `apps/hello-rustframe/styles.css`
- `apps/hello-rustframe/app.js`
- `apps/hello-rustframe/bridge.js`

Window metadata is defined directly in `index.html`:

```html
<title>Hello Rustframe</title>
<meta name="rustframe:width" content="1280">
<meta name="rustframe:height" content="820">
```

Run the app:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe
```

Or run it from inside the app directory without passing the app name:

```bash
cd apps/hello-rustframe
cargo run -p rustframe-cli -- dev
```

Point it at a frontend dev server:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe http://127.0.0.1:5173
```

Export a release build into `apps/hello-rustframe/dist/`:

```bash
cargo run -p rustframe-cli -- export hello-rustframe
```

From inside the app directory:

```bash
cd apps/hello-rustframe
cargo run -p rustframe-cli -- export
```

## App Rules

- `apps/<name>/index.html` is required.
- Everything in the app root is treated as frontend assets, except `dist/` and hidden files.
- The CLI generates a hidden Rust runner under `target/rustframe/apps/<name>/runner/`.
- Exported binaries are copied into `apps/<name>/dist/`.
- Use a dev server when you want tooling like Vite; the production export remains a single embedded binary.

See [FRONTEND_APP_RULES.md](/home/othmane/Downloads/RustFrame/FRONTEND_APP_RULES.md) for the full contract app authors should follow.

## Linux notes

The current implementation is Linux-first and expects the native GTK/WebKitGTK stack required by `wry`. The release size target refers to the stripped executable only, not system libraries.
