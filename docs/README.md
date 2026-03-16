# RustFrame Docs

RustFrame is a stripped-down desktop application framework in Rust built around `tao` and `wry`. This folder collects the repo's working contract in one place so the public site, the examples, and the developer workflow all point at the same source material.

## Start Here

- [Getting Started](./getting-started.md)
- [Runtime And Capabilities](./runtime-and-capabilities.md)
- [Frontend App Rules](./frontend-app-rules.md)
- [Example Apps](./example-apps.md)

## Workspace Map

- `crates/rustframe` is the reusable runtime crate.
- `crates/rustframe-cli` creates, exports, and Linux-packages frontend-first desktop apps.
- `examples/capability-demo` proves embedded assets, native IPC, sandboxed filesystem access, and allowlisted shell execution.
- `apps/*` contains frontend-only desktop apps with root-level `index.html`, `styles.css`, `app.js`, `bridge.js`, optional `data/`, raw binaries in `dist/`, and Linux bundles in `dist/linux/`.

## Repo Sources

- Project overview: [README.md](https://github.com/OthmaneBlial/rustframe/blob/main/README.md)
- App contract: [FRONTEND_APP_RULES.md](https://github.com/OthmaneBlial/rustframe/blob/main/FRONTEND_APP_RULES.md)
- Repository root: [github.com/OthmaneBlial/rustframe](https://github.com/OthmaneBlial/rustframe)

This `docs/` folder reorganizes that material into shorter product and implementation guides.
