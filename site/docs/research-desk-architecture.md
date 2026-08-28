# Research Desk Architecture Case Study

Research Desk is the flagship proof that a useful local desktop product can stay frontend-owned. Its application directory contains HTML, CSS, JavaScript, a data schema, one additive SQL migration, and a RustFrame manifest. It has no app-owned Rust crate, custom native plugin, private IPC method, remote API, or shell command.

## The boundary

```text
Markdown and text files chosen by the person
                    |
          opaque grant:// workspace
                    |
                    v
Research Desk JavaScript ---- public rustframe-api calls
        |                              |
        |                              v
        |                    RustFrame native runtime
        |                       |             |
        v                       v             v
UI and workflow state     embedded SQLite   scoped filesystem
        |
        v
JSON / JSONL / CSV exports and redacted diagnostics
```

The frontend receives an opaque grant URI, not a reusable absolute-path capability. The runtime resolves and authorizes every later read, watch, and revoke operation. Review state lives in the application SQLite database; source documents remain in the selected folder.

## End-to-end workflow

1. The main window calls `fs.requestGrant({ kind: "directory", access: "read", persist: true })` after explaining the consent boundary.
2. `fs.walk` lists only Markdown and text documents beneath that grant. `fs.readText` reads changed entries; the app parser produces metadata and a content fingerprint.
3. One `db.batch` commits inserts, updates, rename-preserving patches, and removals. Cancellation happens before this atomic commit, so a partial scan never becomes partial database state.
4. `db.search` uses the runtime search contract over the `documents` table. The schema declares an FTS5 index for title, summary, tags, reviewer, and note.
5. `window.open` creates a synchronized reader route. Database and filesystem events refresh other windows without granting them the main window's backup, restore, or dialog capabilities.
6. The frontend builds human-readable JSON, JSONL, CSV, full-data, and diagnostic exports, then hands the bytes to `dialog.saveText`. Database recovery uses `db.backup` and `db.restore`.
7. `fs.revokeGrant` removes future source access without deleting source files or silently erasing indexed review state.

## Public API traceability

| Product behavior | Public RustFrame surface | Declared permission | Automated evidence |
| --- | --- | --- | --- |
| Choose and remember one folder | `fs.requestGrant`, `fs.listGrants` | `fs:grants:read` | `first run explains the exact consent boundary and data controls` |
| Incremental discovery and read | `fs.walk`, `fs.readText`, `fs.metadata` | `fs:grants:read` | `incremental indexing can cancel without a partial database commit` |
| Atomic changed-only commit | `db.list`, `db.batch` | `db:read`, `db:write` | cancellation and rename-preservation browser tests; runtime transaction tests |
| Full-text search | `db.search` | `db:read` | `FTS results are highlighted and filter views persist` |
| Notes, tags, status, pinning | generated table client plus `db.update` | `db:write` | keyboard, filter, and data-control browser journeys |
| Reader windows | `window.list`, `window.open`, `window.setTitle` | `window:create` | public manifest window validation and multi-window runtime tests |
| Live refresh | `events.onDatabaseChange`, `events.onFilesystemChange`, `fs.watch` | `fs:grants:watch` | runtime event routing and watcher ownership tests |
| Portable exports | `db.list`, `dialog.saveText` | `db:read`, `dialog:save` | JSON/JSONL/CSV and redacted diagnostic browser test |
| Backup and restore | `db.backup`, `db.restore`, `events.onRestore` | `db:backup`, `db:restore` | runtime backup/restore integration test and migration fixture |
| Permission revocation | `fs.revokeGrant`, `fs.unwatch` | `fs:grants:write`, `fs:grants:watch` | revoke handling in the protected deletion journey |

The source of truth is [`apps/research-desk/app.js`](https://github.com/OthmaneBlial/rustframe/blob/main/apps/research-desk/app.js), the pure indexing path is [`indexing.mjs`](https://github.com/OthmaneBlial/rustframe/blob/main/apps/research-desk/indexing.mjs), and the public bridge contract is [`packages/rustframe-api/src/index.ts`](https://github.com/OthmaneBlial/rustframe/blob/main/packages/rustframe-api/src/index.ts).

## Least privilege by window

The main window can read and write the database, back up and restore, manage grants, save files, create reader windows, and copy text. Reader windows can read and update review state and read already-granted source documents, but they cannot back up, restore, open save dialogs, watch folders, or revoke grants. The manifest declares no filesystem roots and no shell commands.

This is important: a synchronized reader is not a second master window. The runtime checks the current window's declared permissions for every request.

## Failure and recovery truth

- A read failure creates an explicit unreadable record and a redacted recoverable-error entry.
- Canceling a scan commits nothing from that scan.
- A rename keeps the existing row ID, note, pin, and review status when content or file metadata identifies one unambiguous move.
- Revocation disconnects source access but leaves source files untouched.
- Protected local-data deletion requires a successful database backup first.
- Schema v2 adds content fingerprints through an additive migration. The integration fixture proves v1 rows survive the upgrade and that an older schema cannot silently open the newer database.

## Performance receipt

The [public benchmark page](https://othmaneblial.github.io/rustframe/benchmarks.html) records package size, fresh-data native initialization, peak resident memory, exact parser/fingerprint throughput, unchanged-source checks, and repeated production builds. Its JSON includes raw samples, host details, the source commit, and limitations. The indexing number deliberately excludes native filesystem IPC and SQLite commit time rather than presenting a partial measurement as end-to-end throughput.

Reproduce it from the repository:

```bash
npm --prefix apps/research-desk ci
node scripts/benchmark_research_desk.mjs
```

## What this case study does not claim

- It does not claim that a runtime smoke start equals time to an interactive native window.
- It does not claim that parser throughput equals full native indexing throughput.
- It does not claim signed public downloads until the protected release workflow completes with real Apple and Windows credentials.
- It does not hide the npm initial-publication gate behind a local tarball.

Those boundaries keep the flagship useful as evidence: each positive claim points to source, a public API, a permission, and a repeatable check.
