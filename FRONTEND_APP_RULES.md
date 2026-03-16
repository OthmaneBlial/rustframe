# RustFrame Frontend App Rules

This file defines the rules a frontend app must follow to export cleanly with RustFrame.

## Goal

RustFrame apps are frontend-first desktop apps. The app folder should feel like a plain HTML/CSS/JS project, while RustFrame generates the hidden Rust runner under `target/` during `dev`, `export`, and `package`.

## Required App Shape

- Every app lives in `apps/<app-name>/`.
- `apps/<app-name>/index.html` is required.
- Keep runtime assets in the app root or in subfolders under that root.
- `dist/` is reserved for release artifacts such as exported binaries and Linux bundles.
- Hidden files and folders are ignored by the embed step.

## Recommended Minimum Files

- `index.html`
- `styles.css`
- `app.js`
- `rustframe.json` when the app needs native capabilities or typed runtime config
- `assets/icon.svg` when the app will be packaged for Linux
- `data/schema.json` when the app needs persistent data
- `data/seeds/*.json` for optional first-run rows
- `data/migrations/*.sql` for versioned database upgrades and backfills
- `dist/`

## Window Metadata

RustFrame prefers `rustframe.json` as the typed source for desktop window metadata.

Preferred manifest pattern:

```json
{
  "window": {
    "title": "My App",
    "width": 1280,
    "height": 820
  }
}
```

Fallback HTML pattern:

```html
<title>My App</title>
<meta name="rustframe:width" content="1280">
<meta name="rustframe:height" content="820">
```

Rules:

- `window.title`, `window.width`, and `window.height` are the primary typed source for native window config.
- `<title>` becomes the native window title at launch when the manifest omits `window.title`.
- `rustframe:width` must be a positive number.
- `rustframe:height` must be a positive number.
- If manifest and HTML both omit width or height, RustFrame falls back to defaults.
- You may also set `<meta name="rustframe:dev-url" content="http://127.0.0.1:5173">` for development.
- The runtime injects `window.RustFrame` before your app scripts run, so frontend-only apps do not need a `bridge.js` asset.

## Manifest Rules

Use `apps/<app-name>/rustframe.json` for typed runtime config that should not live in HTML:

```json
{
  "appId": "my-app",
  "packaging": {
    "version": "0.1.0",
    "description": "My App desktop package",
    "linux": {
      "icon": "assets/icon.svg",
      "categories": ["Utility"],
      "keywords": ["desktop", "rustframe"]
    }
  },
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

Rules:

- `appId` is optional and defaults to the app folder name.
- `packaging.version` defaults to `0.1.0` when omitted.
- `packaging.description` defaults to the app title when omitted.
- `packaging.linux.icon` may point to an `.svg` or `.png` file relative to the app root.
- `packaging.linux.categories` defaults to `["Utility"]`.
- `security.model` defaults to `"local-first"` and may also be `"networked"`.
- `security.bridge.database`, `security.bridge.filesystem`, and `security.bridge.shell` only matter when you need to selectively re-enable bridge namespaces for a `networked` frontend.
- `filesystem.roots` entries must be non-empty strings.
- `shell.commands[].name` values must be unique.
- `shell.commands[].args` and `shell.commands[].allowedArgs` entries must be non-empty when present.
- `shell.commands[].cwd` must not be blank when present.
- `shell.commands[].timeoutMs` and `shell.commands[].maxOutputBytes` must be greater than zero when present.
- `shell.commands[].env` keys must be non-empty and must not contain `=` or NUL bytes.
- `packaging.linux.keywords[]` entries must not contain semicolons.
- `${SOURCE_APP_DIR}`, `${SOURCE_ASSET_DIR}`, and `${EXE_DIR}` are supported inside declared values.
- Relative filesystem roots resolve against the source app folder in debug builds and against the executable directory in release builds.

## Asset Rules

- Use relative asset paths such as `./styles.css`, `styles.css`, `assets/icon.png`, or `scripts/app.js`.
- Do not rely on absolute filesystem paths.
- Do not depend on `http://localhost/...` in production mode.
- Everything in the app root, except `dist/` and hidden files, is treated as exportable app content.
- Do not keep `node_modules`, screenshots, docs, archives, or random tooling files in the app root if you plan to export directly from it.
- If you need a bundler, use a dev server during development and export only the built static assets into the app root before running `export` or `package`.
- If you define `data/schema.json`, it is embedded into the app and used to initialize the SQLite database on first launch.
- Seed files under `data/seeds/` are also embedded and applied once to the user database.
- SQL migration files under `data/migrations/` are embedded and applied in version order during upgrades.

## HTML Rules

- `index.html` must be a valid standalone entrypoint.
- `window.RustFrame` is injected by the runtime before your app scripts run.
- Keep script and stylesheet references relative.
- If you use client-side routing, route paths without file extensions are safest because RustFrame falls back to `index.html` for extensionless routes.

Recommended footer pattern:

```html
<script src="app.js"></script>
```

## JavaScript Rules

- Use `window.RustFrame` as the native bridge surface.
- Do not call `window.ipc.postMessage` directly unless you are extending the bridge intentionally.
- Handle Promise rejections from native calls.
- Assume desktop startup should feel instant; avoid heavy blocking work on first render.
- Keep app startup resilient if the WebView is running in embedded mode or dev-server mode.

## Trust Rules

- RustFrame assumes a trusted frontend by default.
- If the app loads remote content, uses third-party scripts, renders user HTML, or has a meaningful XSS surface, set `security.model` to `"networked"`.
- In `networked` mode, only the window bridge stays exposed by default. Database, filesystem, and shell access must be re-enabled explicitly through `security.bridge.*`.
- The runtime enforces those bridge boundaries in both JS and native IPC. Hidden calls to `window.ipc.postMessage(...)` do not bypass them.
- `window.RustFrame.security` reports the active trust model and exposed bridge namespaces.
- Keep the template CSP strict unless you have a concrete reason to loosen it. If you loosen CSP or load remote scripts, treat the app as `networked`.

## Currently Safe Native APIs

Available by default in frontend-only apps:

- `window.RustFrame.window.close()`
- `window.RustFrame.window.minimize()`
- `window.RustFrame.window.maximize()`
- `window.RustFrame.window.setTitle(title)`

Available when `data/schema.json` exists and the frontend trust model allows database access:

- `window.RustFrame.db.info()`
- `window.RustFrame.db.get(table, id)`
- `window.RustFrame.db.list(table, options)`
- `window.RustFrame.db.count(table, options)`
- `window.RustFrame.db.insert(table, record)`
- `window.RustFrame.db.update(table, id, patch)`
- `window.RustFrame.db.delete(table, id)`

Important limitation:

- `window.RustFrame.fs.readText(...)` exists in the bridge, but frontend-only apps do not grant filesystem roots by default.
- `window.RustFrame.shell.exec(...)` exists in the bridge, but frontend-only apps do not allow shell commands by default.
- `window.RustFrame.db.*` only stays exposed by default for `local-first` apps.
- `shell.exec` frontend args are denied unless that named command explicitly allowlists them.
- Declared shell commands run with bounded time and bounded captured output.
- `rustframe.json` is the frontend-only way to declare those capabilities.
- If you call those APIs without declaring capabilities, expect permission errors.
- The SQLite file is not stored inside `dist/` or the executable. RustFrame creates it in the user app-data directory.

## CSS and UI Rules

- Design for a desktop window, not a mobile webpage.
- Avoid relying on browser-default form controls if visual consistency matters across WebView engines.
- Set explicit layout constraints so text, cards, and controls do not overflow narrow panels.
- Always provide a meaningful initial paint or loading state. Do not show a blank screen while the app initializes.

## Export Rules

- Run export from the workspace root with an app name, or from inside the app folder with no app name.
- The exported binary is copied into `apps/<app-name>/dist/`.
- The hidden generated runner lives under `target/rustframe/apps/<app-name>/runner/`.
- Database schema and seeds are embedded into the binary, but user data is written to the OS app-data directory.

## Package Rules

- Run package from the workspace root with an app name, or from inside the app folder with no app name.
- The Linux bundle is written into `apps/<app-name>/dist/linux/`.
- The bundle contains a portable `*.AppDir`, a `.desktop` launcher, an app icon, install scripts, and a `.tar.gz` archive.
- The hidden generated runner still lives under `target/rustframe/apps/<app-name>/runner/`.
- If `apps/<app-name>/native/Cargo.toml` exists because the app was ejected, `dev`, `export`, and `package` use that runner instead.
- Database schema and seeds are embedded into the binary, but user data is written to the OS app-data directory.

Examples:

```bash
cargo run -p rustframe-cli -- export orbit-desk
```

```bash
cd apps/orbit-desk
cargo run -p rustframe-cli -- export
```

```bash
cargo run -p rustframe-cli -- package orbit-desk
```

```bash
cd apps/orbit-desk
cargo run -p rustframe-cli -- package
```

## Dev Rules

- For a static app, use `cargo run -p rustframe-cli -- dev <app-name>`.
- For a dev server, pass the URL explicitly or place it in a `rustframe:dev-url` meta tag.
- The production export should still be tested without the dev server.

Examples:

```bash
cargo run -p rustframe-cli -- dev orbit-desk
```

```bash
cargo run -p rustframe-cli -- dev orbit-desk http://127.0.0.1:5173
```

## Do Not Break These Rules

- Do not add ad hoc Rust runner files inside app folders unless you intentionally used `rustframe-cli eject`.
- The supported ejected location is `apps/<app-name>/native/`.
- Do not treat `dist/` as source input.
- Do not put unrelated non-app files in the app root.
- Do not assume filesystem or shell access exists by default.
- Do not point `packaging.linux.icon` at a missing or unsupported file type.
- Do not ship a UI that starts on a blank page.

## Practical Checklist

- The app folder contains frontend files only.
- `index.html` has a `<title>`.
- `index.html` defines `rustframe:width` and `rustframe:height`.
- All asset references are relative.
- The app works without a localhost server.
- If persistent data is needed, `data/schema.json` exists and is valid JSON.
- Export places a binary in `dist/`.
- Package places a Linux bundle in `dist/linux/`.
