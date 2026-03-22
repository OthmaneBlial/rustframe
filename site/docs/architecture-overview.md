# Architecture Overview

## Purpose

RustFrame keeps the app itself small while the runtime owns the native shell.

The default idea is simple:

> the app lives as a frontend folder, and the runtime handles the desktop responsibilities around it

## The Four Layers

### 1. Frontend App Folder

Each app starts as plain files under `apps/<name>/`:

- `index.html`
- `styles.css`
- `app.js`
- `rustframe.json`
- optional `assets/`
- optional `data/`

This is the main authoring model.

## 2. Manifest Contract

`rustframe.json` is the typed contract for:

- window settings
- development URL
- security model
- filesystem roots
- shell commands
- packaging metadata

That keeps the app configuration explicit without forcing a visible native project first.

## 3. Runner Layer

RustFrame generates a hidden runner by default under:

```text
target/rustframe/apps/<name>/runner/
```

That runner:

- embeds app assets
- injects the `window.RustFrame` bridge
- wires capabilities from the manifest
- loads SQLite when schema files exist
- becomes the target for `dev`, `export`, `platform-check`, and `package`

When the app outgrows that path, `rustframe-cli eject <name>` creates an app-owned native runner in `apps/<name>/native/`.

## 4. Runtime Layer

The runtime crate owns:

- the native window shell
- the `app://localhost/` asset protocol
- IPC dispatch
- multi-window coordination
- SQLite lifecycle and migrations
- filesystem scope enforcement
- hardened shell execution
- security boundary enforcement

This is where the desktop responsibility lives.

## Request Flow

From the frontend, the path looks like this:

```text
window.RustFrame.* call
  -> injected bridge
  -> native IPC request
  -> runtime capability check
  -> filesystem / shell / database / window action
  -> structured response back to the frontend
```

The frontend talks to one runtime-owned bridge surface instead of owning its own native bridge layer.

## Security Boundary

RustFrame has two main trust modes:

- `local-first`
- `networked`

`local-first` assumes the frontend is trusted and can access declared capabilities.

`networked` assumes the frontend is less trusted and blocks database, filesystem, and shell access unless the manifest explicitly re-enables them.

The runtime enforces this in both the injected bridge config and the native IPC layer.

## Data Model

If `data/schema.json` exists, RustFrame can provision embedded SQLite for the app.

The runtime can also embed:

- seed files from `data/seeds/`
- migration files from `data/migrations/`

The schema definition ships with the app, while the actual SQLite database is created in the user's app-data directory.

## Why The Hidden Runner Exists

The hidden runner exists so small apps do not need to begin life as visible native projects.

That gives RustFrame its core authoring model:

- small app folder first
- runtime-owned native shell first
- app-owned native runner later only when needed

## Where RustFrame Draws The Line

RustFrame is deliberately strongest in the middle:

- more than a browser tab
- less than a full native-first desktop framework commitment

That line is what gives the project its identity.
