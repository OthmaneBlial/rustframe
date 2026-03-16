# RustFrame

RustFrame is a stripped-down desktop application framework in Rust for people who want the desktop shell, not the framework ceremony.

It is built around `tao` and `wry`, but deliberately avoids the usual pileup:

- no visible Rust project inside every app
- no plugin layer
- no localhost IPC server
- no "your frontend is now a framework-specific desktop app" feeling

The core idea is simple:

> a desktop app can just be a frontend folder

That means an app in `apps/<name>/` can stay plain HTML, CSS, and JavaScript, while RustFrame handles the native window, embedded assets, IPC bridge, and optional local database behind the scenes.

Repository: [github.com/OthmaneBlial/rustframe](https://github.com/OthmaneBlial/rustframe)

RustFrame is especially aimed at local-first tools, internal software, and vanilla-web desktop apps that should be shippable in hours, not after a week of framework ceremony.

## Why Build This?

RustFrame exists because there is a real gap between:

- building a normal frontend
- and buying into a large desktop framework stack

Sometimes you do not want plugins, abstraction layers, generated config files, and a desktop-specific mental model wrapped around everything.

Sometimes you just want this:

- an `index.html`
- a `styles.css`
- an `app.js`
- a tiny native bridge
- one command to run it
- one command to export it

RustFrame is that bet.

It keeps the app authoring model aggressively small, while still giving you the native pieces that matter:

- embedded assets
- native window controls
- direct IPC
- optional SQLite with schema and seed files
- optional allowlisted filesystem and shell capabilities

## What Makes It Different

### 1. The app folder stays frontend-first

There is no visible `Cargo.toml`, `src/`, or Rust runner living inside each app.

The CLI generates the hidden runner under:

```text
target/rustframe/apps/<name>/runner/
```

Your app folder stays readable and boring on purpose.

When you outgrow that path, `rustframe-cli eject <name>` materializes an app-owned runner under `apps/<name>/native/` without forking the `rustframe` library.

### 2. Window metadata lives in HTML

RustFrame reads desktop window settings directly from `index.html`:

```html
<title>Hello Rustframe</title>
<meta name="rustframe:width" content="1280">
<meta name="rustframe:height" content="820">
```

That keeps the source of truth close to the UI.

### 3. Zero-config local data is a real differentiator

If `data/schema.json` exists, RustFrame initializes a SQLite database in the user app-data directory on first launch.

If `data/seeds/*.json` exists, those rows are embedded into the binary and applied once.

If `data/migrations/*.sql` exists, versioned SQL migrations are embedded and applied during schema upgrades before the runtime falls back to additive reconciliation.

That means a frontend-only app can gain a local database by dropping files into the app folder instead of wiring a custom backend, a custom IPC layer, and a migration story by hand.

### 4. Native capabilities stay explicit

The runtime can expose:

- window controls
- database operations
- filesystem reads inside allowed roots
- allowlisted shell commands

Nothing about that needs a localhost bridge or a plugin marketplace.

### 5. The example apps prove the thesis

RustFrame does not just show one tiny starter app and call it a day.

The repo ships eleven vanilla JS/CSS example apps plus a capability demo to prove that the same runtime model can handle:

- notes
- CRM
- inventory
- habits
- editorial workflows
- task planning
- media libraries

That matters because it shows the framework is not just elegant on paper. It already scales to real UI density and varied product shapes.

## What Is In This Repo

- `crates/rustframe`
  Reusable runtime crate.
- `crates/rustframe-cli`
  Scaffolding, dev, and export tooling.
- `examples/capability-demo`
  Sample app showing embedded assets, native IPC, filesystem access, and allowlisted shell execution.
- `apps/*`
  Frontend-only desktop apps with root-level HTML, CSS, JavaScript, optional data files, and exported binaries in `dist/`.
- `docs/`
  Repo docs covering getting started, runtime capabilities, app rules, and the example app set.
- `site/`
  Portable static project site generated from the repository itself.

## Who RustFrame Is For

RustFrame is a strong fit when you want to build:

- local-first desktop apps
- internal business tools
- solo-dev and indie-hacker products
- vanilla web apps that need a native shell and local data

It is especially attractive if you want the desktop runtime without adopting a heavyweight desktop framework mental model.

If you are trying to build something in the class of Discord, Figma, or VS Code, RustFrame is not really trying to compete there yet.

## Quick Start

Run the capability demo:

```bash
cargo run -p capability-demo
```

Use a frontend dev server during development:

```bash
RUSTFRAME_DEV_URL=http://127.0.0.1:5173 cargo run -p capability-demo
```

## Create An App

Generate a new app:

```bash
cargo run -p rustframe-cli -- new hello-rustframe
```

RustFrame creates a frontend-first app folder in `apps/hello-rustframe`.

Edit these files directly:

- `apps/hello-rustframe/index.html`
- `apps/hello-rustframe/styles.css`
- `apps/hello-rustframe/app.js`
- `apps/hello-rustframe/bridge.js`
- `apps/hello-rustframe/data/schema.json`
- `apps/hello-rustframe/data/seeds/*.json`

Run it:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe
```

Or run it from inside the app directory:

```bash
cd apps/hello-rustframe
cargo run -p rustframe-cli -- dev
```

Point it at a frontend dev server:

```bash
cargo run -p rustframe-cli -- dev hello-rustframe http://127.0.0.1:5173
```

Export a release build:

```bash
cargo run -p rustframe-cli -- export hello-rustframe
```

Or from inside the app directory:

```bash
cd apps/hello-rustframe
cargo run -p rustframe-cli -- export
```

The exported binary lands in:

```text
apps/hello-rustframe/dist/
```

## The App Contract

At a practical level, RustFrame asks app authors to follow a very small contract:

- `apps/<name>/index.html` is required
- everything in the app root is treated as frontend assets except `dist/` and hidden files
- `bridge.js` should load before `app.js`
- if `data/schema.json` exists, the app gets embedded SQLite support
- seed files in `data/seeds/*.json` are embedded and applied once
- use a dev server when you want frontend tooling, but keep production export static and embedded

Full rules and repo context:

- [Frontend app rules](https://github.com/OthmaneBlial/rustframe/blob/main/FRONTEND_APP_RULES.md)
- [Docs folder](https://github.com/OthmaneBlial/rustframe/tree/main/docs)
- [Project site files](https://github.com/OthmaneBlial/rustframe/tree/main/site)

## Example Apps

The repo ships multiple example apps to prove that this model is not just a toy:

- `hello-rustframe`
- `daybreak-notes`
- `atlas-crm`
- `dispatch-room`
- `ember-habits`
- `harbor-bookings`
- `ledger-grove`
- `meridian-inventory`
- `orbit-desk`
- `prism-gallery`
- `quill-studio`

Some are SQLite-backed. One is local-storage-first. All of them keep the same core idea: the app starts as a frontend, and RustFrame gives it a native shell without taking over the whole project.

## Current Limitations

RustFrame is promising, but it is still honest software:

- the current implementation is Linux-first
- the current app model is still mainly single-window
- packaging and distribution are still early

That is a conscious tradeoff for simplicity today, not the end state.

## Linux Notes

The current implementation is Linux-first and expects the native GTK/WebKitGTK stack required by `wry`.

The release size target refers to the stripped executable only, not system libraries.
