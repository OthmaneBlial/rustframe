# RustFrame

RustFrame is a stripped-down desktop application framework in Rust built around `tao` and `wry`, without Tauri's CLI, plugin layer, or localhost IPC stack.

## Workspace

- `crates/rustframe`: reusable runtime crate
- `crates/rustframe-cli`: scaffolding and export tooling
- `examples/capability-demo`: sample app showing the embedded frontend, native IPC, filesystem access, and allowlisted shell execution
- `apps/*`: generated HTML/CSS/JS desktop apps
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

Edit:

- `apps/hello-rustframe/frontend/index.html`
- `apps/hello-rustframe/frontend/styles.css`
- `apps/hello-rustframe/frontend/app.js`
- `apps/hello-rustframe/rustframe.toml`

Run the app:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe
```

Point it at a frontend dev server:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe http://127.0.0.1:5173
```

Export a release build into `apps/hello-rustframe/dist/`:

```bash
cargo run -p rustframe-cli -- export hello-rustframe
```

## Linux notes

The current implementation is Linux-first and expects the native GTK/WebKitGTK stack required by `wry`. The release size target refers to the stripped executable only, not system libraries.
