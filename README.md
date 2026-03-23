# RustFrame

<p align="center">
  <strong>Build local-first desktop workflow tools with plain frontend files.</strong><br>
  Native window. Embedded SQLite. Scoped machine access. No full desktop project on day one.
</p>

<p align="center">
  <a href="docs/getting-started.md">Get Started</a>
  ·
  <a href="docs/choosing-rustframe.md">Fit Guide</a>
  ·
  <a href="docs/architecture-overview.md">Architecture</a>
  ·
  <a href="docs/runtime-and-capabilities.md">Runtime</a>
  ·
  <a href="docs/example-apps.md">Example Apps</a>
  ·
  <a href="docs/README.md">All Docs</a>
</p>

RustFrame is not trying to be the next everything-framework for desktop apps.

It is a narrower bet:

> workflow tools that are too native for a browser tab, but too small to deserve a full desktop framework project from day one

That means apps that are mostly frontend code, but still need:

- a real desktop window
- local SQLite
- scoped filesystem access
- one or two allowlisted automations
- packaging into a real installable desktop app

If that is not your shape, RustFrame is probably the wrong tool. That honesty is part of the pitch.

## The Honest Pitch

- If a browser tab is enough, use a browser tab.
- If you already know you need deep native APIs, broad plugins, or a mature ecosystem, use Tauri, Electron, or a native stack.
- If your app is mostly HTML, CSS, and JavaScript but still needs a desktop shell, local data, and a tight native surface, RustFrame is where it starts to make sense.

## Why This Project Exists

The painful part of many small desktop tools is not the UI.

It is all the scaffolding around the UI:

- the native project
- the bridge layer
- the SQLite glue
- the packaging story
- the capability boundaries
- the awkward jump from "simple tool" to "needs a bit more native control"

RustFrame tries to make that path smaller without pretending the desktop disappears.

## Run This First

The flagship app is `apps/research-desk`.

If you want to decide whether RustFrame is useful, do not start with the template. Start here:

```bash
cargo run -p rustframe-cli -- dev research-desk
```

`research-desk` is the clearest proof of the wedge today. It:

- indexes a bundled local archive into SQLite
- reads real files through scoped filesystem roots
- runs an allowlisted Python indexer from the UI
- opens reader windows for focused review
- exports the visible review queue

That is the current answer to "why not just a browser tab?"

## What Makes RustFrame Different

RustFrame changes the default authoring model.

Your app starts as a plain folder:

```text
apps/<name>/
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

The runtime owns the rest:

- the native window
- the injected `window.RustFrame` bridge
- embedded assets
- SQLite provisioning and migrations
- scoped filesystem access
- allowlisted shell execution
- packaging and host validation
- the eject path when the app outgrows the hidden runner

That keeps the early path small while still leaving a way out later.

## Best Fit

Use RustFrame when:

- your app should stay mostly frontend code
- you want local SQLite without building the full desktop stack yourself
- you need a native window, local files, and a few explicit machine capabilities
- you want packaging, export, and capability boundaries to be runtime-owned
- you want to eject only after the product earns that complexity

Do not use RustFrame when:

- the product works fine as a normal web app or PWA
- you need deep platform integrations immediately
- you want a large desktop ecosystem from day one
- you need Chromium-level rendering consistency everywhere
- you are already productive in Tauri, Electron, or native and not feeling friction

## Browser vs RustFrame vs Tauri/Electron

| Question | Browser tab | RustFrame | Tauri / Electron |
| --- | --- | --- | --- |
| Native desktop window | No | Yes | Yes |
| Embedded local SQLite by default | No | Yes | Possible |
| Scoped filesystem and allowlisted commands | Limited | Yes | Yes |
| Plain frontend folder as the default app shape | Yes | Yes | Not usually |
| Mature plugin ecosystem | N/A | No | Yes |
| Best for narrow local-first workflow tools | Sometimes | Yes | Sometimes |
| Best for broad desktop app ambitions | No | No | Yes |

The point is not "RustFrame beats Tauri." The point is that there is a smaller product slice where RustFrame can be the cleaner starting shape.

## What Ships Today

RustFrame already includes:

- a runtime crate built on `tao` and `wry`
- a CLI that can `new`, `doctor`, `dev`, `inspect`, `reset-data`, `export`, `platform-check`, `package --verify`, and `eject`
- runtime-owned `window.RustFrame` injection instead of per-app bridge duplication
- embedded SQLite with schema files, immutable seeds, versioned SQL migrations, and runtime full-text search
- scoped filesystem helpers for reads, writes, dialogs, open, and reveal
- hardened shell capabilities with explicit timeout and output limits
- `local-first` and `networked` trust models
- clipboard writes and multi-window state persistence
- host-native packaging flows for Linux, Windows, and macOS
- workflow-first starters plus Vite, React Vite, Vue Vite, and Svelte Vite starters
- a community template catalog and ecosystem docs for sync and capability extension patterns
- automated tests and workflow smoke coverage

## Start In Minutes

Prerequisites:

- Rust and Cargo
- a native host toolchain for the platform you are targeting
- Linux uses the GTK and WebKitGTK stack required by `wry`
- Windows uses the MSVC Rust toolchain
- macOS uses Xcode command line tools

Check the host first:

```bash
cargo run -p rustframe-cli -- doctor
```

Run the flagship workflow:

```bash
cargo run -p rustframe-cli -- dev research-desk
```

Generate a new app:

```bash
cargo run -p rustframe-cli -- new hello-rustframe
```

Run it:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe
```

Inspect the resolved contract:

```bash
cargo run -p rustframe-cli -- inspect hello-rustframe
```

Export the raw binary:

```bash
cargo run -p rustframe-cli -- export hello-rustframe
```

Package and verify a host-native bundle:

```bash
cargo run -p rustframe-cli -- package hello-rustframe --verify
```

If you want to develop with a frontend dev server:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe http://127.0.0.1:5173
```

Starter source for Vite, React Vite, Vue Vite, and Svelte Vite lives under `examples/frontend-starters/`.

## Ecosystem, But On Purpose

Phase 5 added a small ecosystem surface, but it stays tied to the wedge:

- [Community Templates](docs/community-templates.md)
- [Remote Sync Patterns](docs/remote-sync-patterns.md)
- [Capability Extension Patterns](docs/capability-extension-patterns.md)
- [Example Apps](docs/example-apps.md)

The machine-readable template catalog lives here:

```text
examples/community-templates/catalog.json
```

The rule is simple: ecosystem only after fit. One credible flagship workflow matters more than a hundred random demos.

## Reality Check

RustFrame is promising, but still early:

- the ecosystem is small
- deep native integration is not the default path
- signing and updates are documented, but still handled at the release-pipeline layer
- Linux still carries heavier GTK, WebKitGTK, and display-stack constraints
- cross-host validation still depends on the matching native host toolchain

Those are not footnotes. They define the real shape of the project today.

## Production Surface

RustFrame now has a clearer shipping contract for small production tools:

- [Platform Support](docs/platform-support.md)
- [Signing And Notarization](docs/signing-and-notarization.md)
- [Update Strategy](docs/update-strategy.md)
- [Release Checklist](docs/release-checklist.md)

Repo CI also verifies packaged bundles on supported hosts.

## Repo Map

- `crates/rustframe`
  Reusable runtime crate.
- `crates/rustframe-cli`
  Scaffolding, validation, export, packaging, and ejection tooling.
- `apps/research-desk`
  Flagship local archive review workflow.
- `apps/hello-rustframe`
  Default workflow-first starter app.
- `examples/frontend-starters/`
  Optional frontend-stack starters.
- `examples/capability-demo`
  Sample app proving the bridge, filesystem scope, and shell model.
- `docs/`
  Product and implementation docs.
- `site/`
  Portable static site derived from the repo.

## Read Next

- [Getting Started](docs/getting-started.md)
- [Choosing RustFrame](docs/choosing-rustframe.md)
- [Architecture Overview](docs/architecture-overview.md)
- [Runtime And Capabilities](docs/runtime-and-capabilities.md)
- [Frontend App Rules](FRONTEND_APP_RULES.md)
- [Example Apps](docs/example-apps.md)

The shortest useful next step is still:

```bash
cargo run -p rustframe-cli -- dev research-desk
```
