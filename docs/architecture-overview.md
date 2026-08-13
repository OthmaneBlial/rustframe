# Architecture Overview

RustFrame has three coordinated public packages:

- `rustframe-runtime`: the Rust library imported as `rustframe`; owns windows, embedded assets, IPC authorization, capabilities, and SQLite.
- `rustframe-cli`: installs the `rustframe` binary; owns project discovery, manifests, generation, build orchestration, migration, diagnostics, and packaging.
- `rustframe-api`: the npm package containing frontend types, runtime availability checks, stable errors, and the generic table-aware client.

```text
rustframe.json + frontend + data/schema.json
                    │
                    ▼
              rustframe CLI
          validate / codegen / build
                    │
                    ▼
       target/rustframe/runner (hidden)
                    │ exact runtime release
                    ▼
     native window ── typed IPC ── capabilities
                          │
             SQLite / grants / watchers / commands
```

The manifest and generated frontend types are build-time contracts. The injected bridge is convenience and availability plumbing. Native IPC is the runtime security boundary. SQLite and filesystem work run away from the UI thread; dialogs and window operations remain on it. Committed database changes and filesystem watcher events are delivered back through the native event loop.

`cargo-packager` consumes an internal configuration generated from `rustframe.json`. RustFrame's own CLI release binaries are configured separately through `dist`/`cargo-dist`.
