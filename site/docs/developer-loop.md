# Developer Loop

RustFrame keeps the frontend server and generated native runner as separate processes. Vite owns frontend hot reload; Cargo only rebuilds when the generated runner contract, embedded production assets, runtime, or native configuration changes.

## Diagnose the host once

```bash
rustframe doctor
rustframe doctor --json
```

The JSON contract is `rustframe.doctor` schema version 1. It records the CLI version and detected install source, host and architecture, readiness, and one stable `RF-DOCTOR-*` code per check. Every check includes a remediation URL and a host-appropriate command or next action. The command works outside a project, so installers and CI can run it immediately.

## Start development with visible phases

```bash
rustframe dev
rustframe dev --open-devtools
```

The CLI prints the current phase while it checks the native host, synchronizes generated database types, starts the frontend server, prepares the runner, and hands control to Cargo. The optional inspector opens only for the primary debug WebView; release packages do not enable it.

Generated runner files are written only when their bytes change. The reusable Cargo target lives at `target/rustframe/native`, so repeated development, build, and package commands share compiled dependencies instead of invalidating the runner on every invocation. Frontend edits continue through Vite without waiting for native compilation.

## Export a support bundle

```bash
rustframe diagnostics export
rustframe diagnostics export ./support-bundle.json
```

The default file is `target/rustframe/diagnostics/rustframe-diagnostics.json`. The schema version 1 bundle contains:

- `doctor --json` host evidence;
- manifest validation and stable error codes;
- project inspection, local-first conformance, and normalized capability policy when the manifest can load;
- at most the latest 200 native audit records;
- an explicit state for evidence that was not captured.

The exporter removes the project directory, home directory, and opaque grant/root identifiers recursively. It never includes document contents or environment values. Frontend console capture is deliberately excluded; use `--open-devtools` for the live page console and reproduce only the smallest safe error in an issue.

The exporter still writes a bundle when validation or project loading fails. A failed stage is represented as `{ "state": "error", "error": "..." }`, which preserves the diagnostic sequence without turning the support tool into another blocked command.

## Where state lives

```text
target/rustframe/runner/       generated native source, updated only on change
target/rustframe/native/       shared Cargo build cache
target/rustframe/logs/         redacted runtime audit JSONL
target/rustframe/diagnostics/  redacted support bundles
```

Use the [public benchmark](https://othmaneblial.github.io/rustframe/benchmarks.html) for measured native initialization, package size, memory, indexing, and warm production builds. Local timings vary with the Rust cache, linker, host WebView, and frontend dependency cache; RustFrame does not turn one machine's result into a universal speed claim.
