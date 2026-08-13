# RustFrame

> A Rust runtime and CLI for building frontend-first, local-first desktop workflow tools.

RustFrame is for apps that are mostly HTML, CSS, and JavaScript but still need a real desktop window, local SQLite, a small and explicit native surface, and an installable package.

The app starts as a plain folder. RustFrame owns the desktop shell, the runtime bridge, database lifecycle, capability boundaries, and packaging. You can eject to an app-owned Rust runner when the app needs deeper native control.

RustFrame is not a Rust UI toolkit. It is the Rust layer around a frontend-first desktop app.

<p align="center">
  <a href="docs/getting-started.md">Get Started</a>
  ·
  <a href="docs/choosing-rustframe.md">Choose RustFrame</a>
  ·
  <a href="docs/architecture-overview.md">Architecture</a>
  ·
  <a href="docs/runtime-and-capabilities.md">Runtime API</a>
  ·
  <a href="docs/example-apps.md">Example Apps</a>
  ·
  <a href="docs/README.md">All Docs</a>
</p>

## The useful middle

A browser tab is often too limited for a serious local workflow. A full desktop project can be too much ceremony for a focused internal tool, workbench, or operator app.

RustFrame targets that middle:

| If you need… | RustFrame gives you… |
| --- | --- |
| A desktop window | `tao` + `wry`, managed by the runtime |
| Local structured data | Bundled SQLite with schema files, immutable seeds, and SQL migrations |
| Machine access | Scoped filesystem roots, native dialogs, clipboard, and in-app windows |
| A little automation | Named shell commands with allowlists, timeouts, output limits, and audit records |
| A small starting point | A plain frontend folder and a generated hidden runner |
| A path to more Rust | `rustframe-cli eject` and an app-owned native runner |

The point is not to replace Tauri, Electron, or a native stack at everything. The point is to keep a narrow class of local-first tools small until their native requirements justify more structure.

## Why Rust developers may want it

RustFrame lets Rust own the parts that should be explicit and host-aware without making every app start as a Rust application:

- the native window and event loop
- the `window.RustFrame` IPC bridge
- SQLite provisioning, search, and migrations
- filesystem scope and shell capability enforcement
- multi-window coordination
- host-native export and packaging

Frontend authors work in `index.html`, `styles.css`, `app.js`, and `rustframe.json`. Rust developers get a reusable runtime crate, a deterministic manifest contract, and an escape hatch for custom native code.

## See the proof app first

`apps/research-desk` is the clearest example of the intended wedge: a local archive review workbench that is awkward in a browser tab but does not need a full native-first rewrite on day one.

```bash
cargo run -p rustframe-cli -- doctor
cargo run -p rustframe-cli -- dev research-desk
```

The app demonstrates:

- indexing a bundled local archive into SQLite
- reading real documents through declared filesystem roots
- running an allowlisted Python indexer from the UI
- opening focused reader windows over the shared local database
- exporting the visible review queue

## Start a new app

Check the host, scaffold a frontend-first app, and run it:

```bash
cargo run -p rustframe-cli -- doctor
cargo run -p rustframe-cli -- new hello-rustframe
cargo run -p rustframe-cli -- dev hello-rustframe
```

The generated app is intentionally not a miniature native project. Its source shape is:

```text
apps/hello-rustframe/
├── index.html
├── styles.css
├── app.js
├── rustframe.json
├── assets/
└── data/
    ├── schema.json
    ├── seeds/
    └── migrations/
```

Use `rustframe.json` for window settings, development URLs, security mode, capabilities, and packaging metadata. The runtime generates the Rust runner under `target/`; it does not clutter the app folder with generated native code.

If you prefer a frontend dev server, use one of the Vite, React, Vue, or Svelte starters under [`examples/frontend-starters/`](examples/frontend-starters/).

## The app contract

The manifest makes the native surface visible and reviewable:

```json
{
  "appId": "research-desk",
  "security": { "model": "local-first" },
  "filesystem": {
    "roots": ["workspace", "tools"]
  },
  "shell": {
    "commands": [
      {
        "name": "indexWorkspace",
        "program": "python3",
        "args": ["index_workspace.py", "../workspace"],
        "cwd": "tools",
        "timeoutMs": 15000,
        "maxOutputBytes": 262144
      }
    ]
  }
}
```

The injected bridge exposes only the namespaces allowed by the resolved trust model and manifest. In `networked` mode, database, filesystem, and shell access are disabled by default. Native IPC enforces the same boundary; the frontend cannot bypass it by calling the transport directly.

## From prototype to package

The CLI covers the workflow around the runtime:

```bash
# inspect the resolved app contract
cargo run -p rustframe-cli -- inspect hello-rustframe

# rebuild local SQLite from schema, migrations, and immutable seeds
cargo run -p rustframe-cli -- reset-data hello-rustframe

# emit the raw executable
cargo run -p rustframe-cli -- export hello-rustframe

# create and verify a host-native bundle
cargo run -p rustframe-cli -- package hello-rustframe --verify

# create an app-owned Rust runner when the hidden runner is no longer enough
cargo run -p rustframe-cli -- eject hello-rustframe
```

`export` and `package` carry declared relative filesystem roots beside the executable or inside the platform bundle. Platform checks report which rows were validated on the current native host instead of pretending cross-host validation happened.

## Fit guide

Choose RustFrame when:

- the product is mostly frontend code but local-first data is central
- you need a native shell plus a few explicit machine capabilities
- you want the runtime to own SQLite, packaging, and capability wiring
- you want to delay app-owned Rust until the product earns that complexity

Choose a browser or PWA when the app does not need packaging, local SQLite, or machine access.

Choose Tauri, Electron, or a native stack when you need broad native integrations, a mature plugin ecosystem, Chromium-level rendering consistency, or a framework your team already operates well.

See the fuller [Choosing RustFrame](docs/choosing-rustframe.md) comparison for the trade-offs.

## What ships today

- `rustframe`: the reusable runtime crate, built around `tao`, `wry`, and bundled `rusqlite`
- `rustframe-cli`: scaffolding, host checks, inspection, development, export, packaging, reset, and ejection
- runtime-owned `window.RustFrame` bridge for window, database, filesystem, dialog, clipboard, and shell operations
- local-first and networked trust models
- schema reconciliation, immutable seeds, versioned migrations, and runtime full-text search
- host-native Linux, Windows, and macOS packaging flows
- workflow starters, frontend-stack starters, capability examples, and a community template catalog
- automated tests plus CI package verification on supported host runners

The project is early (`0.1.x`). The ecosystem is small, deep native integration is intentionally not the default path, and signing and updates remain release-pipeline concerns. Linux also carries the GTK, WebKitGTK, and display-stack requirements of `wry`.

## Repository map

- [`crates/rustframe`](crates/rustframe) — reusable runtime crate
- [`crates/rustframe-cli`](crates/rustframe-cli) — app lifecycle and packaging CLI
- [`apps/research-desk`](apps/research-desk) — flagship local archive workflow
- [`apps/hello-rustframe`](apps/hello-rustframe) — default workflow starter
- [`examples/frontend-starters`](examples/frontend-starters) — Vite, React, Vue, and Svelte entry points
- [`examples/capability-demo`](examples/capability-demo) — direct runtime capability example
- [`docs/`](docs) — product, architecture, security, and release guides
- [`site/`](site) — static project site generated from the repo

## Read next

- [Getting Started](docs/getting-started.md)
- [Architecture Overview](docs/architecture-overview.md)
- [Runtime And Capabilities](docs/runtime-and-capabilities.md)
- [Frontend App Rules](FRONTEND_APP_RULES.md)
- [Example Apps](docs/example-apps.md)
- [Platform Support](docs/platform-support.md)
- [Release Checklist](docs/release-checklist.md)
