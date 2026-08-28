# Troubleshooting

Start with the machine-readable diagnostic output. It keeps the failing stage and remediation evidence together.

```bash
rustframe doctor --json
rustframe validate
```

## Install and host checks

### Rust and Cargo

Install Rust 1.88 or newer with rustup and reopen the terminal. `rustframe doctor --json` reports `RF-DOCTOR-001` through `RF-DOCTOR-003` for Cargo, the compiler, and the resolved host triple.

### Linux WebKitGTK

Install `pkg-config`, GTK 3 development files, and a WebKitGTK 4.1 development package. Debian and Ubuntu use `libgtk-3-dev` and `libwebkit2gtk-4.1-dev`; use the exact package names from your distribution rather than copying this command to an unrelated system.

### macOS Xcode tools

Run `xcode-select --install`, then verify `xcode-select -p`. Signing credentials are a separate protected release concern and are not required for normal local development.

### Windows MSVC

Use an MSVC Rust toolchain and install Visual Studio Build Tools with the Desktop development with C++ workload. Open a Developer PowerShell when `cl.exe` is installed but missing from the current shell.

### Supported hosts

RustFrame's native development and verification hosts are Linux, Windows, and macOS. Cross-compilation does not replace a native-host install, launch, signature, and uninstall receipt.

| Symptom | Why it happens | Next action |
| --- | --- | --- |
| `rustframe` is not found | The installer directory is not on `PATH` | Reopen the terminal, then run the installer’s printed PATH command. |
| Rust is missing | The stock native runner is compiled on the host | Install Rust 1.88 or newer, then rerun `rustframe doctor`. |
| Linux WebView check fails | GTK or WebKit development packages are absent | Run the exact distribution command printed by `doctor`; package names differ by distribution. |
| The frontend package cannot install | `rustframe-api@0.1.0-rc.1` has not completed its initial npm publication | Check the release status. Do not substitute a repository path in a public-project test. |

## Create and validate

### The manifest schema does not resolve

Current schema v1 manifests must use:

```json
{
  "$schema": "https://othmaneblial.github.io/rustframe/schemas/v1/rustframe.schema.json",
  "schemaVersion": 1
}
```

Run `rustframe migrate` to replace the retired pre-release URL.

### Generated database types are stale

Run:

```bash
rustframe codegen
rustframe codegen --check
```

Commit the generated file with the schema change. CI treats drift as a contract failure.

### Validation rejects a permission

Permissions are attached to exact window IDs or an explicit supported suffix pattern. Check the `security.permissions` entry and use:

```bash
rustframe validate --json
```

Do not widen every window to make one call pass. Read [Runtime and capabilities](./runtime-and-capabilities.md) and [Threat model](./threat-model.md).

## Development

| Symptom | Check | Remediation |
| --- | --- | --- |
| Frontend port is busy | Compare the process with `frontend.devUrl` | Stop the stale process or choose one matching loopback URL and rerun validation. |
| Native window opens but the page does not | Inspect the Vite command and CSP connect sources | Make `devCommand`, `devUrl`, and the loopback CSP sources agree. |
| A grant URI stops working | The user revoked access or moved the file | Ask for access again and present the lost-access state; never reconstruct an absolute path. |
| Shell command times out | The manifest bound was reached | Reduce the work, add cancellation, or deliberately review `timeoutMs`; do not remove the bound. |

## Build and package

Run the stages separately so the error stays attributable:

```bash
rustframe validate
rustframe build
rustframe package
```

`build` proves the frontend and native runner. `package` additionally needs host-native packaging tools and metadata. Signing and notarization are separate trust gates; read [Platform support](./platform-support.md), [Signing and notarization](./signing-and-notarization.md), and the [release checklist](./release-checklist.md).

## Report a reproducible problem

Run `rustframe diagnostics export`, review the redacted JSON, and include it with the smallest manifest that reproduces the problem. Never attach private file paths, document contents, signing credentials, or npm tokens. See [Developer loop and diagnostics](./developer-loop.md) for the exact bundle contract.
