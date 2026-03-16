# Frontend App Rules

This is the working app contract for frontend-first RustFrame apps.

## Required App Shape

- Every app lives in `apps/<app-name>/`.
- `apps/<app-name>/index.html` is required.
- Keep runtime assets in the app root or subfolders under that root.
- `dist/` is reserved for release artifacts such as exported binaries and Linux bundles.
- Hidden files and hidden folders are ignored by the embed step.

## Recommended Files

- `index.html`
- `styles.css`
- `app.js`
- `rustframe.json` when the app needs native capabilities or typed runtime config
- `assets/icon.svg` when the app will be packaged for Linux
- `data/schema.json` when the app needs persistent data
- `data/seeds/*.json` for first-run rows
- `data/migrations/*.sql` for versioned database upgrades and backfills
- `dist/`

## HTML Contract

The runtime injects `window.RustFrame` before your app scripts run, so frontend-only apps can load `app.js` directly:

```html
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

## Manifest Contract

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
- `filesystem.roots` entries must be non-empty strings.
- `shell.commands[].name` values must be unique.
- `packaging.linux.keywords[]` entries must not contain semicolons.
- `${SOURCE_APP_DIR}`, `${SOURCE_ASSET_DIR}`, and `${EXE_DIR}` are supported inside declared values.
- Relative filesystem roots resolve against the source app folder in debug builds and against the executable directory in release builds.

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
- SQL migration files under `data/migrations/` are applied in version order during upgrades.
- The database file lives in the user app-data directory.

Use these roles:

- `data/seeds/*.json` for immutable first-run data.
- `data/migrations/*.sql` for schema evolution and data changes after release.

## Filesystem And Shell Limits

Frontend-only apps do not get filesystem or shell access by default.

- `window.RustFrame.fs.readText(...)` exists in the bridge, but requires the runtime to allow one or more filesystem roots.
- `window.RustFrame.shell.exec(...)` exists in the bridge, but requires the runtime to allow a named command.
- `rustframe.json` is the frontend-only way to declare those capabilities.

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

## Package Rules

From the workspace root:

```bash
cargo run -p rustframe-cli -- package orbit-desk
```

From inside the app directory:

```bash
cd apps/orbit-desk
cargo run -p rustframe-cli -- package
```

Package writes a Linux bundle into `apps/<name>/dist/linux/` with:

- a portable `*.AppDir`
- a `.desktop` launcher and app icon
- `install.sh` and `uninstall.sh`
- a `.tar.gz` archive for distribution

If `apps/<name>/native/Cargo.toml` exists because the app was ejected, `dev`, `export`, and `package` use that runner instead of the hidden generated runner.

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

- Do not add a visible `Cargo.toml` or `src/` to app folders unless you intentionally used `rustframe-cli eject`.
- The supported ejected location is `apps/<app-name>/native/`.
- Do not treat `dist/` as source input.
- Do not assume filesystem or shell access exists.
- Do not point `packaging.linux.icon` at a missing or unsupported file type.
- Do not ship a UI that boots to a blank screen.
