# Workflow Cookbook

This guide maps common desktop workflow jobs to the smallest RustFrame shape that usually makes sense.

## Local File Indexer

Use this when the app owns a bounded local corpus and needs search, review state, and a few helper scripts.

- Keep the source files under a declared filesystem root such as `workspace/`.
- Put the imported metadata in SQLite so the UI can sort, filter, and search without rescanning files on every paint.
- Add helper scripts under a second declared root such as `tools/` and expose them through allowlisted shell commands.
- Start from `apps/research-desk` if you need a working reference.

Good fit:

- document review desks
- research archives
- compliance evidence workbenches
- operations incident notebooks

## Markdown Library

Use this when the product is mostly a local writing or reading surface over a directory of notes.

- Declare a notes root such as `library/`.
- Store tags, reading status, backlinks, or export history in SQLite.
- Use the filesystem bridge for the source markdown and the database for app-specific state.
- Add `dialog.openFile(...)` or `dialog.openDirectory(...)` so users can attach or import new content.

Good fit:

- personal knowledge bases
- meeting-note review apps
- editorial libraries
- SOP or runbook readers

## Command Runner

Use this when the app is mostly a controlled front end over a small set of machine actions.

- Model jobs, runs, and outputs in SQLite.
- Keep the shell surface narrow: expose named commands with bounded args, explicit `cwd`, and timeout/output limits.
- Turn on the dev audit log while shaping the command contract.
- Use `networked` mode if the frontend loads remote content or untrusted HTML.

Good fit:

- build-and-publish helpers
- data cleanup utilities
- import pipelines
- recurring internal ops scripts

## Media Organizer

Use this when the product manages local assets and needs a desktop shell around them.

- Declare one or more asset roots such as `footage/`, `exports/`, or `intake/`.
- Track asset metadata, review state, and derived files in SQLite.
- Use `fs.revealPath(...)`, `fs.openPath(...)`, and save dialogs for the common file handoff actions.
- Add packaging metadata early if the app will be handed to non-technical users.

Good fit:

- photo review tools
- podcast clip organizers
- light DAM workflows
- local creative approvals

## Quick Pattern Checks

RustFrame usually fits when:

- the UI is mostly frontend code
- the data is local-first
- the native surface is explicit and small
- the product benefits from a packaged desktop shell

RustFrame usually does not fit when:

- the app is fine as a browser tab
- the native surface is broad from day one
- the product needs a large plugin ecosystem immediately
