# RustFrame Frontend App Rules

This file defines the rules a frontend app must follow to export cleanly with RustFrame.

## Goal

RustFrame apps are frontend-first desktop apps. The app folder should feel like a plain HTML/CSS/JS project, while RustFrame generates the hidden Rust runner under `target/` during `dev` and `export`.

## Required App Shape

- Every app lives in `apps/<app-name>/`.
- `apps/<app-name>/index.html` is required.
- Keep runtime assets in the app root or in subfolders under that root.
- `dist/` is reserved for exported binaries.
- Hidden files and folders are ignored by the embed step.

## Recommended Minimum Files

- `index.html`
- `styles.css`
- `app.js`
- `bridge.js`
- `rustframe.json` when the app needs native capabilities or typed runtime config
- `data/schema.json` when the app needs persistent data
- `data/seeds/*.json` for optional first-run rows
- `data/migrations/*.sql` for versioned database upgrades and backfills
- `dist/`

## Window Metadata

RustFrame reads desktop window metadata directly from `index.html`.

Required pattern:

```html
<title>My App</title>
<meta name="rustframe:width" content="1280">
<meta name="rustframe:height" content="820">
```

Rules:

- `<title>` becomes the native window title at launch.
- `rustframe:width` must be a positive number.
- `rustframe:height` must be a positive number.
- If width or height is missing, RustFrame falls back to defaults.
- You may also set `<meta name="rustframe:dev-url" content="http://127.0.0.1:5173">` for development.

## Manifest Rules

Use `apps/<app-name>/rustframe.json` for typed runtime config that should not live in HTML:

```json
{
  "appId": "my-app",
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
- `filesystem.roots` entries must be non-empty strings.
- `shell.commands[].name` values must be unique.
- `${SOURCE_APP_DIR}`, `${SOURCE_ASSET_DIR}`, and `${EXE_DIR}` are supported inside declared values.
- Relative filesystem roots resolve against the source app folder in debug builds and against the executable directory in release builds.

## Asset Rules

- Use relative asset paths such as `./styles.css`, `styles.css`, `assets/icon.png`, or `scripts/app.js`.
- Do not rely on absolute filesystem paths.
- Do not depend on `http://localhost/...` in production mode.
- Everything in the app root, except `dist/` and hidden files, is treated as exportable app content.
- Do not keep `node_modules`, screenshots, docs, archives, or random tooling files in the app root if you plan to export directly from it.
- If you need a bundler, use a dev server during development and export only the built static assets into the app root before running `export`.
- If you define `data/schema.json`, it is embedded into the app and used to initialize the SQLite database on first launch.
- Seed files under `data/seeds/` are also embedded and applied once to the user database.
- SQL migration files under `data/migrations/` are embedded and applied in version order during upgrades.

## HTML Rules

- `index.html` must be a valid standalone entrypoint.
- Load `bridge.js` before `app.js`.
- Keep script and stylesheet references relative.
- If you use client-side routing, route paths without file extensions are safest because RustFrame falls back to `index.html` for extensionless routes.

Recommended footer pattern:

```html
<script src="bridge.js"></script>
<script src="app.js"></script>
```

## JavaScript Rules

- Use `window.RustFrame` as the native bridge surface.
- Do not call `window.ipc.postMessage` directly unless you are extending the bridge intentionally.
- Handle Promise rejections from native calls.
- Assume desktop startup should feel instant; avoid heavy blocking work on first render.
- Keep app startup resilient if the WebView is running in embedded mode or dev-server mode.

## Currently Safe Native APIs

Available by default in frontend-only apps:

- `window.RustFrame.window.close()`
- `window.RustFrame.window.minimize()`
- `window.RustFrame.window.maximize()`
- `window.RustFrame.window.setTitle(title)`

Available when `data/schema.json` exists:

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
- If `apps/<app-name>/native/Cargo.toml` exists because the app was ejected, `dev` and `export` use that runner instead.
- Database schema and seeds are embedded into the binary, but user data is written to the OS app-data directory.

Examples:

```bash
cargo run -p rustframe-cli -- export orbit-desk
```

```bash
cd apps/orbit-desk
cargo run -p rustframe-cli -- export
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
- Do not load `app.js` before `bridge.js`.
- Do not assume filesystem or shell access exists by default.
- Do not ship a UI that starts on a blank page.

## Practical Checklist

- The app folder contains frontend files only.
- `index.html` has a `<title>`.
- `index.html` defines `rustframe:width` and `rustframe:height`.
- `bridge.js` loads before `app.js`.
- All asset references are relative.
- The app works without a localhost server.
- If persistent data is needed, `data/schema.json` exists and is valid JSON.
- Export places a binary in `dist/`.
