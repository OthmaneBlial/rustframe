# Frontend App Rules

This is the working app contract for frontend-first RustFrame apps.

## Required App Shape

- Every app lives in `apps/<app-name>/`.
- `apps/<app-name>/index.html` is required.
- Keep runtime assets in the app root or subfolders under that root.
- `dist/` is reserved for exported binaries.
- Hidden files and hidden folders are ignored by the embed step.

## Recommended Files

- `index.html`
- `styles.css`
- `app.js`
- `bridge.js`
- `data/schema.json` when the app needs persistent data
- `data/seeds/*.json` for first-run rows
- `dist/`

## HTML Contract

Load `bridge.js` before `app.js`:

```html
<script src="bridge.js"></script>
<script src="app.js"></script>
```

Window metadata also lives in `index.html`:

```html
<title>My App</title>
<meta name="rustframe:width" content="1280">
<meta name="rustframe:height" content="820">
```

Optional:

```html
<meta name="rustframe:dev-url" content="http://127.0.0.1:5173">
```

## Asset Rules

- Use relative asset paths.
- Do not rely on absolute filesystem paths.
- Do not depend on `http://localhost/...` in production mode.
- Everything in the app root except `dist/` and hidden files is treated as exportable app content.
- Do not leave screenshots, archives, or random tooling files in the app root if you plan to export directly from it.

## JavaScript Rules

- Use `window.RustFrame` as the native bridge surface.
- Do not call `window.ipc.postMessage` directly unless you are intentionally extending the bridge.
- Handle Promise rejections from native calls.
- Keep startup fast and resilient.

## Database Rules

If `data/schema.json` exists:

- RustFrame creates the SQLite database on first launch.
- Seed files under `data/seeds/` are embedded and applied once.
- The database file lives in the user app-data directory.

## Filesystem And Shell Limits

Frontend-only apps do not get filesystem or shell access by default.

- `window.RustFrame.fs.readText(...)` exists in the bridge, but requires the runtime to allow one or more filesystem roots.
- `window.RustFrame.shell.exec(...)` exists in the bridge, but requires the runtime to allow a named command.

Without those capabilities, expect permission errors.

## Export Rules

From the workspace root:

```bash
cargo run -p rustframe-cli -- export orbit-desk
```

From inside the app directory:

```bash
cd apps/orbit-desk
cargo run -p rustframe-cli -- export
```

Export copies the built binary into `apps/<name>/dist/`.

## Dev Rules

For a static app:

```bash
cargo run -p rustframe-cli -- dev orbit-desk
```

For a dev server:

```bash
cargo run -p rustframe-cli -- dev orbit-desk http://127.0.0.1:5173
```

## Do Not Break These Rules

- Do not add a visible `Cargo.toml` or `src/` to app folders.
- Do not treat `dist/` as source input.
- Do not load `app.js` before `bridge.js`.
- Do not assume filesystem or shell access exists.
- Do not ship a UI that boots to a blank screen.
