# Getting Started

This tutorial starts with the public CLI and ends with a native package. The project can live anywhere; it is not a RustFrame workspace member.

## 1. Install

Install Rust 1.88 or newer, Node.js 20 or newer, and your host's native WebView toolchain. The fastest CLI path uses the release binary.

macOS or Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/OthmaneBlial/rustframe/releases/download/v0.1.0-rc.1/rustframe-cli-installer.sh | sh
```

Windows PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/OthmaneBlial/rustframe/releases/download/v0.1.0-rc.1/rustframe-cli-installer.ps1 | iex"
```

Build-from-source alternative:

```bash
cargo install rustframe-cli --version 0.1.0-rc.1 --locked
```

Then run `rustframe doctor` for host-specific checks. Use `rustframe doctor --json` for stable host-check codes and remediation links. The initial `rustframe-api` npm publication is still a release-candidate gate; until it is public, generated projects cannot complete a registry-only `npm install`.

Linux needs GTK 3 and WebKitGTK development packages, Windows needs the MSVC toolchain, and macOS needs Xcode command-line tools.

During development, `rustframe dev --open-devtools` opens the primary WebView inspector. It is a debug-only switch and does not change packaged releases.

## 2. Create a project

```bash
rustframe new research-tool
cd research-tool
npm install
```

The default is vanilla TypeScript. Select another deterministic template with `--template vanilla-js`, `react-ts`, `vue-ts`, or `svelte-ts`. Package managers are selected with `--package-manager npm|pnpm|yarn|bun`.

The public project contract is:

```text
research-tool/
├── rustframe.json
├── package.json
├── index.html
├── src/
│   ├── main.ts
│   └── rustframe.generated.ts
├── data/
│   ├── schema.json
│   ├── seeds/
│   └── migrations/
├── public/
└── assets/
```

There is no app-owned Rust project. RustFrame creates its runner under `target/rustframe/` and uses the exact compatible `rustframe-runtime` release. `rustframe eject` is the explicit escape hatch.

## 3. Validate and develop

```bash
rustframe validate
rustframe dev
```

`validate` checks manifest schema v1, permissions, paths, assets, database schema, and committed generated types. `dev` regenerates database types, starts the configured Vite command, waits for `frontend.devUrl`, launches the native window, and owns both process lifetimes.

Use the typed package in TypeScript:

```ts
import { getRustFrame } from "rustframe-api";
import type { AppRustFrameClient } from "./rustframe.generated";

const rustframe = getRustFrame() as AppRustFrameClient;
const rows = await rustframe.db.list("items");
```

Plain JavaScript can use `getRustFrame()` or the injected `window.RustFrame` global.

## 4. Add a local folder workflow

Declare only the permissions the main window needs in `rustframe.json`, then request a user-selected folder:

```ts
const grant = await rustframe.fs.requestGrant({
  kind: "directory",
  access: "read",
  persist: true
});

if (!grant) throw new Error("No folder selected");

const documents = await rustframe.fs.walk(grant.uri, {
  recursive: true,
  extensions: ["md", "txt"],
  limit: 10_000
});

const watcher = await rustframe.fs.watch(grant.uri, { recursive: true });
```

Frontend code receives opaque `grant://` and `root://` URIs. Native IPC resolves and authorizes every operation. Watchers stop when their window closes or their grant is revoked.

## 5. Build and package

```bash
rustframe build
rustframe package --verify
```

`build` runs the frontend build before compiling the hidden release runner. `package` creates the native host formats and writes artifacts under `dist/packages/`, together with `SHA256SUMS`, `rustframe-package-manifest.json`, and `RELEASE_NOTES.md`.

- macOS: `.app` and DMG
- Windows: NSIS and MSI
- Linux: AppImage and Debian

Use `--format app|dmg|nsis|msi|appimage|deb` to request a single compatible format. Local output is explicitly unsigned; use the platform release pipeline for signing and notarization.

## 6. Inspect or use a monorepo

```bash
rustframe inspect
rustframe inspect --json
rustframe --project apps/research-desk validate
```

Without `--project`, the CLI walks upward to the nearest `rustframe.json`. This keeps standalone projects simple while preserving explicit monorepo support.
