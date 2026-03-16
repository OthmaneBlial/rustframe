# Runtime And Capabilities

## Runtime Shape

RustFrame is not trying to be a full Tauri replacement. The current workspace intentionally stays small:

- The desktop shell is built around `tao` and `wry`.
- Frontend assets are served from a custom `app://localhost/` protocol when embedded.
- Development can switch to an HTTP dev server through `RUSTFRAME_DEV_URL` or a `rustframe:dev-url` meta tag.
- The runtime injects a small promise bridge onto `window.RustFrame`, not through a localhost IPC server.

## App Metadata Comes From HTML Plus An Optional Manifest

The CLI still reads desktop metadata from `index.html` by default:

```html
<title>Hello Rustframe</title>
<meta name="rustframe:width" content="1280">
<meta name="rustframe:height" content="820">
```

- `<title>` becomes the native window title.
- `rustframe:width` and `rustframe:height` set the initial window size.
- `rustframe:dev-url` can override the embedded asset mode during development.

Frontend-only apps can also add `apps/<name>/rustframe.json` for typed configuration:

```json
{
  "appId": "hello-rustframe",
  "devUrl": "http://127.0.0.1:5173",
  "packaging": {
    "version": "0.1.0",
    "description": "Hello RustFrame desktop package",
    "linux": {
      "icon": "assets/icon.svg",
      "categories": ["Utility"],
      "keywords": ["desktop", "rustframe"]
    }
  },
  "filesystem": {
    "roots": ["fixtures"]
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

When both HTML metadata and `rustframe.json` set the same window fields, the manifest wins.
The same manifest also provides Linux packaging metadata for `rustframe-cli package`.

## Embedded Assets

When you run `dev`, `export`, or `package`, the CLI walks the app asset directory and embeds everything except:

- `dist/` at the app root
- hidden files and hidden folders

`index.html` is required.

## Native IPC Surface

The shipped bridge exposes these methods:

### Window

- `window.RustFrame.window.close()`
- `window.RustFrame.window.minimize()`
- `window.RustFrame.window.maximize()`
- `window.RustFrame.window.setTitle(title)`

### Database

If the app contains `data/schema.json`, RustFrame enables a SQLite capability with:

- `window.RustFrame.db.info()`
- `window.RustFrame.db.get(table, id)`
- `window.RustFrame.db.list(table, options)`
- `window.RustFrame.db.count(table, options)`
- `window.RustFrame.db.insert(table, record)`
- `window.RustFrame.db.update(table, id, patch)`
- `window.RustFrame.db.delete(table, id)`

The runtime manages these record fields automatically:

- `id`
- `createdAt`
- `updatedAt`

Schema files, seed files, and versioned SQL migration files under `data/migrations/` are embedded into the app binary. The actual SQLite file is created in the user's app-data directory, not inside `dist/`.

Migration files:

- are discovered from `data/migrations/*.sql`
- are versioned by the numeric prefix in the filename, such as `002-rename-title.sql`
- run during upgrades before the runtime falls back to additive table reconciliation
- are the supported path for non-additive changes such as column renames, drops, type changes, and backfills

## Filesystem Capability

The runtime can expose read access to explicit directories through `allow_fs_root(...)`.
Frontend-only apps now declare those roots through `rustframe.json`.

- `window.RustFrame.fs.readText(path)` only succeeds inside the configured roots.
- Parent escapes and absolute paths outside those roots are rejected.
- Relative roots resolve against the source app folder in debug builds and against the executable directory in release builds.
- `${SOURCE_APP_DIR}`, `${SOURCE_ASSET_DIR}`, and `${EXE_DIR}` can be expanded inside declared values.

The capability demo previously wired this in Rust by hand; frontend-only apps can now do the same through the manifest.

## Shell Capability

The runtime can expose allowlisted commands through `allow_shell_command(...)`.
Frontend-only apps now declare allowlisted commands through `rustframe.json`.

- `window.RustFrame.shell.exec(name, args)` resolves to structured `stdout`, `stderr`, and `exitCode`.
- Unknown commands are rejected.
- Commands run directly through `std::process::Command`, not through a shell pipeline.
- `${SOURCE_APP_DIR}`, `${SOURCE_ASSET_DIR}`, and `${EXE_DIR}` can be used inside the declared program or argument strings.

## Hidden Runner Generation

Frontend-only apps stay clean because the Rust runner is generated under:

```text
target/rustframe/apps/<name>/runner/
```

That runner:

- embeds the app assets
- injects the canonical `window.RustFrame` bridge at document start
- carries forward window metadata from `index.html` and optional overrides from `rustframe.json`
- wires in the database capability when `data/schema.json` exists
- wires in filesystem roots and shell commands declared in `rustframe.json`
- feeds Linux package metadata from `rustframe.json` into `rustframe-cli package`

## Ejected Runner Path

When an app needs deeper native control, `rustframe-cli eject <name>` creates `apps/<name>/native/`.

That ejected runner:

- depends on the `rustframe` library instead of copying runtime code into the app
- embeds the app assets directly from the app folder through `rust-embed`
- becomes the runner used by `dev`, `export`, and `package` for that app
- is the place to add extra native crates, deeper `tao` or `wry` setup, menus, tray work, or shortcuts

## Practical Summary

RustFrame's contract is simple on purpose:

- plain HTML, CSS, and JavaScript in the app folder
- a tiny native bridge
- optional embedded SQLite
- optional scoped filesystem access
- optional allowlisted process execution
