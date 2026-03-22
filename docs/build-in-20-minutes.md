# Build This In 20 Minutes

This walkthrough turns the RustFrame starter into a simple local triage desk without touching runtime internals.

## 1. Check The Host

Run:

```bash
cargo run -p rustframe-cli -- doctor
```

Fix host issues first if the CLI reports missing desktop dependencies.

## 2. Create The App

Run:

```bash
cargo run -p rustframe-cli -- new triage-desk
```

The generated app already contains:

- a workflow queue UI
- a `work_items` SQLite table
- seeded records
- packaging metadata

## 3. Rename The Workflow Shape

Edit `apps/triage-desk/data/schema.json`.

For example, change `work_items` into `incidents` and replace the generic columns with the fields your team actually needs.

Keep the starter rules:

- text fields for the things you want to search
- seeds only for first-run defaults
- migrations later for non-additive changes

## 4. Match The UI To The Job

Edit:

- `apps/triage-desk/index.html`
- `apps/triage-desk/styles.css`
- `apps/triage-desk/app.js`

Do the minimum product-specific rewrite:

- rename queue labels
- swap lanes for your real statuses
- change the selected-item panel into the detail view you actually need

## 5. Run It

Run:

```bash
cargo run -p rustframe-cli -- dev triage-desk
```

If you prefer Vite, start from one of the frontend starters under `examples/frontend-starters/` and point RustFrame at the dev server:

```bash
cargo run -p rustframe-cli -- dev triage-desk http://127.0.0.1:5173
```

## 6. Inspect The Resolved Contract

Run:

```bash
cargo run -p rustframe-cli -- inspect triage-desk
```

This confirms the real paths, bridge surface, schema assets, packaging roots, and security model.

## 7. Add Native Capabilities Only When Needed

Edit `apps/triage-desk/rustframe.json` when the product genuinely needs them.

Typical next additions:

- filesystem roots for real local files
- allowlisted shell commands for import or indexing
- `networked` mode if the frontend stops being fully trusted

## 8. Package And Verify

Run:

```bash
cargo run -p rustframe-cli -- package triage-desk --verify
```

That builds the host-native bundle and checks the produced layout, metadata, scripts, and archive.

## What You Should Avoid

- do not edit old seed files after users already ran the app
- do not expose raw shell access when named commands will do
- do not give the filesystem bridge the whole machine when one project root is enough
- do not keep the starter labels if the product has a clearer job shape
