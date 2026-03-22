# Example Apps

RustFrame ships one flagship workflow app, one runtime capability demo, a starter template app, and a wider reference set.

## Flagship App

- `apps/research-desk`
- Local archive review workflow with a bundled sample workspace and a real end-to-end task shape
- Uses embedded SQLite, scoped filesystem roots, allowlisted Python indexing, reader windows, and queue export
- This is the first app to run when evaluating whether RustFrame is useful

## Capability Demo

- `examples/capability-demo`
- Proves embedded assets, native window controls, sandboxed filesystem reads, and allowlisted shell execution
- Uses `allow_fs_root(...)` and `allow_shell_command(...)`

## Starter App

- `apps/hello-rustframe`
- Workflow queue starter used by the CLI template
- Shows runtime search, embedded SQLite, clipboard path copy, and a product-shaped local queue

## Frontend Stack Starters

- `examples/frontend-starters/vite-vanilla`
- `examples/frontend-starters/react-vite`
- `examples/frontend-starters/vue-vite`
- These are copyable dev-server frontends for teams that want Vite or a mainstream component stack without changing the RustFrame runtime path

## Reference Apps

- `apps/daybreak-notes`
  - Local notes library backed by a `notes` table
- `apps/atlas-crm`
  - Pipeline board backed by a `deals` table
- `apps/dispatch-room`
  - Incident tracker backed by an `incidents` table
- `apps/ember-habits`
  - Habit tracker backed by a `habits` table
- `apps/harbor-bookings`
  - Hospitality bookings surface backed by a `reservations` table
- `apps/ledger-grove`
  - Lightweight ledger backed by an `entries` table
- `apps/meridian-inventory`
  - Inventory tracker backed by an `items` table
- `apps/prism-gallery`
  - Asset library backed by an `assets` table
- `apps/quill-studio`
  - Editorial desk backed by a `stories` table
- `apps/orbit-desk`
  - Task planning surface backed by browser storage instead of the SQLite capability

## What The Examples Prove

- RustFrame can already support one credible file-centric workflow, not just UI variety.
- Apps can stay plain HTML, CSS, and JavaScript.
- Data-backed apps can ship with embedded schema, seeds, and packaged local workspace folders.
- Window controls, filesystem access, shell automation, and database methods can stay on one runtime-owned bridge.
- The design language can vary widely without changing the runtime model.

## Good Reference Picks

Use these examples when you need a starting point:

- `apps/research-desk` for the end-to-end local workflow and native capability mix
- `apps/hello-rustframe` for the default workflow starter
- `examples/frontend-starters/*` when the team wants Vite, React, or Vue from the first day
- `apps/daybreak-notes` for a clean CRUD surface
- `apps/atlas-crm` for a dense board layout
- `apps/prism-gallery` for a media-forward card grid
- `apps/quill-studio` for a structured editorial workflow
