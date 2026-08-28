# Build and Package a Local Tool

This tutorial starts from the published CLI and ends with a native package. The project is independent of the RustFrame source repository.

## 1. Install and Check the Host

Install the prebuilt CLI on macOS or Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/OthmaneBlial/rustframe/releases/download/v0.1.0-rc.2/rustframe-cli-installer.sh | sh
rustframe doctor
```

Windows users can run the release's `rustframe-cli-installer.ps1`. `cargo install --git https://github.com/OthmaneBlial/rustframe --tag v0.1.0-rc.2 rustframe-cli --locked` remains the build-from-source alternative. The initial `rustframe-api` npm publication must be complete before the registry-only project install below can succeed.

Install any native WebView dependencies reported by `doctor` before continuing.

## 2. Create a Project

```bash
rustframe new triage-desk --template vanilla-ts --package-manager npm
cd triage-desk
npm install
```

The starter is a Vite application with a versioned `rustframe.json`, a restrictive Content Security Policy, a SQLite schema, and generated TypeScript APIs.

## 3. Model the Workflow

Edit `data/schema.json`. Rename the starter table to match the work being tracked and add the searchable fields the tool needs. Then regenerate types:

```bash
rustframe codegen
```

Import `getRustFrame` and the generated table types from `src/rustframe.generated.ts`. Keep seeds for genuine first-run defaults; do not present example activity as user data.

## 4. Develop

```bash
rustframe dev
```

RustFrame starts the configured Vite command, waits for `frontend.devUrl`, regenerates database types when the schema changes, and closes the frontend process with the desktop runner.

## 5. Validate the Public Contract

```bash
rustframe validate
rustframe inspect
```

Validation rejects stale generated types, unknown manifest fields, invalid paths and commands, missing assets, and unsupported capability declarations. Use `rustframe inspect --json` in automation.

## 6. Build and Package

```bash
rustframe build
rustframe package --verify
```

`build` runs the frontend build before compiling the hidden native runner. `package` uses cargo-packager to create host-native artifacts under `dist/packages/`, plus checksums, release metadata, and release notes. Local packages are explicitly marked unsigned.

Document-oriented apps can add versioned native file types in `.rustframe/file-associations.json`. The main manifest remains on immutable schema v1. See [Single-instance file-open routing](./file-open-routing.md) for the configuration and frontend event contract.

Run packaging on each native host for its formats:

- macOS: `.app` and `.dmg`
- Windows: NSIS and MSI
- Linux: AppImage and Debian

Use `rustframe eject` only when the application genuinely needs app-owned Rust code. The normal workflow keeps the runner hidden under `target/rustframe/`.
