# Frontend Starters

These examples show how to plug RustFrame into a mainstream frontend stack without rewriting the runtime.

Each starter assumes you already created a RustFrame app:

```bash
cargo run -p rustframe-cli -- new my-workbench
```

Then replace the generated frontend files in `apps/my-workbench/` with one of these Vite-based starters, set `"devUrl"` in `rustframe.json`, and run the Vite dev server beside:

```bash
cargo run -p rustframe-cli -- dev my-workbench http://127.0.0.1:5173
```

Included starters:

- `vite-vanilla`
- `react-vite`
- `vue-vite`

They all use the same assumptions:

- RustFrame injects `window.RustFrame`
- the paired app still owns `rustframe.json`, `data/schema.json`, and `data/seeds/`
- the frontend can call runtime APIs directly without a hand-written bridge file
