# RustFrame Docs

RustFrame is a stripped-down desktop application framework in Rust built around `tao` and `wry`. This folder collects the repo's working contract in one place so the public site, the examples, and the developer workflow all point at the same source material.

## Start Here

- [Getting Started](./getting-started.md)
- [Runtime And Capabilities](./runtime-and-capabilities.md)
- [Frontend App Rules](./frontend-app-rules.md)
- [Example Apps](./example-apps.md)

## Workspace Map

- `crates/rustframe` is the reusable runtime crate.
- `crates/rustframe-cli` creates and exports frontend-first desktop apps.
- `examples/capability-demo` proves embedded assets, native IPC, sandboxed filesystem access, and allowlisted shell execution.
- `apps/*` contains frontend-only desktop apps with root-level `index.html`, `styles.css`, `app.js`, `bridge.js`, optional `data/`, and exported binaries in `dist/`.

## Repo Sources

- The project overview still lives in the root `README.md`.
- The raw app contract still lives in the root `FRONTEND_APP_RULES.md`.
- This `docs/` folder reorganizes that information into shorter product and implementation guides.
