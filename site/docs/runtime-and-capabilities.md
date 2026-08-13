# Runtime and Capabilities

RustFrame embeds a Vite-built frontend in a native `wry` WebView and injects a promise-based bridge. Development may use the manifest's loopback `frontend.devUrl`; production serves bundled `app://localhost/` assets and rejects remote navigation.

## Authorization model

Every IPC request carries a method and structured parameters. The native dispatcher enforces a 1 MiB request limit, validates the method, checks the current window's exact or prefix-scoped permission, applies sensitive-operation rate limits, parses typed parameters, and only then dispatches work. The JavaScript surface mirrors these checks but is not trusted as the boundary.

`local-first` is intended for bundled trusted application code. `networked` cannot receive database, filesystem, or shell permissions. Generated projects include restrictive CSP declarations.

The manifest `security.csp` is emitted as the native `Content-Security-Policy` response header for embedded production assets. Validation rejects incomplete policies and unsafe script directives; the generated HTML carries the matching meta policy during Vite development.

## Public namespaces

- `db`: info, typed CRUD, filtered list/search/count, atomic batch, backup, and restore.
- `fs`: opaque URI metadata/read/write, directory listing, bounded recursive walk, user grants, revocation, and watching.
- `events`: database commits, filesystem changes, restore notification, and temporary drag/drop grants.
- `window`: current/list/open/close/minimize/maximize/title for declared in-app windows.
- `dialog`: native open/save flows.
- `clipboard`: permission-scoped read and write.
- `shell`: named direct-process commands with fixed/allowlisted arguments, timeout, output limits, controlled environment, and audit records.
- `path`: platform-neutral URI/path string helpers.

The typed surface is published as `rustframe-api`; plain JavaScript retains `window.RustFrame`.

## Filesystem lifecycle

A declared root such as `workspace` is exposed as `root://workspace`. Root IDs come from the final path component, and duplicate IDs are rejected. A selected file or folder becomes an opaque `grant://<id>` URI. Persisted grants are reloaded only when the target still resolves safely. Every operation rechecks the resolved root and grant access mode. Walks support extension filters and entry limits. Watchers coalesce polling changes into create, modify, rename, and delete events and are stopped on revocation or owner-window close.

Open and save dialogs return temporary grants, never absolute paths. Use the returned `uri` for subsequent filesystem calls. `fs.copyFrom` accepts authorized root or grant URIs for its source and destination and enforces read and write permissions separately.

Drag/drop paths are immediately converted into temporary read grants before the event reaches JavaScript.

## Database lifecycle

The runtime creates SQLite in the user's application-data directory, applies checked migrations and immutable seeds, and reconciles the validated schema. Batches run in one transaction. Mutation events reach every open window only after commit.

Backup uses SQLite's online backup mechanism. Restore validates the embedded app identity and compatible schema, creates a safety backup, replaces state atomically, reapplies required migrations/schema work, and broadcasts a reload event.

## Generated and ejected runners

The CLI generates its native runner under `<project>/target/rustframe/runner` and builds into `<project>/target/rustframe/native`. The runner pins `rustframe-runtime` to the CLI-compatible registry version. `rustframe eject` materializes the same contract under `<project>/native`; subsequent commands prefer it.

Ejection is for application-specific native customization. Features that Research Desk needs must remain public generic APIs rather than private runner code.
