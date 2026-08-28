# Troubleshooting

Start with the machine-readable diagnostic output. It keeps the failing stage and remediation evidence together.

```bash
rustframe doctor --json
rustframe validate
```

## Install and host checks

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

Include the RustFrame version, host and architecture, the failing command, redacted `doctor --json` output, and the smallest manifest that reproduces the problem. Never attach private file paths, document contents, signing credentials, or npm tokens.
