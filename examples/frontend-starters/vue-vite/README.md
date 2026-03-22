# Vue Vite Starter

Use this when you want Vue single-file components while RustFrame keeps the desktop runner hidden.

Suggested flow:

1. Create a RustFrame app with `cargo run -p rustframe-cli -- new my-workbench`
2. From `apps/my-workbench/`, scaffold Vite with `npm create vite@latest . -- --template vue`
3. Replace `index.html` and `src/*` with the files in this folder
4. Set `"devUrl": "http://127.0.0.1:5173"` in `rustframe.json`
5. Run `npm install`, `npm run dev`, and `cargo run -p rustframe-cli -- dev my-workbench http://127.0.0.1:5173`
