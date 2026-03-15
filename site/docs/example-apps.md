# Example Apps

RustFrame ships a mix of starter surfaces, data-backed desktop apps, and one runtime capability demo.

## Capability Demo

- `examples/capability-demo`
- Proves embedded assets, native window controls, sandboxed filesystem reads, and allowlisted shell execution
- Uses `allow_fs_root(...)` and `allow_shell_command(...)`

## Starter App

- `apps/hello-rustframe`
- Minimal note app used by the CLI template
- Shows window title updates and embedded SQLite through `notes` and `settings`

## Data-Backed App Set

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

## Local-Storage App

- `apps/orbit-desk`
- Task planning surface backed by browser storage instead of the SQLite capability

## What The Examples Prove

- Apps can stay plain HTML, CSS, and JavaScript.
- Data-backed apps can ship with embedded schema and seeds.
- Window controls are available from the same bridge surface as the database methods.
- The overall design language can vary widely without changing the runtime model.

## Good Reference Picks

Use these examples when you need a starting point:

- `apps/hello-rustframe` for the smallest end-to-end app
- `apps/daybreak-notes` for a clean CRUD surface
- `apps/atlas-crm` for a dense board layout
- `apps/prism-gallery` for a media-forward card grid
- `apps/quill-studio` for a structured editorial workflow
- `apps/orbit-desk` for a frontend-only app that does not depend on the database capability
