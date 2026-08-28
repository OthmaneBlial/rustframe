# Workflow guides

Use these shapes as starting points, not product templates. Each one begins with the local job, then adds only the native permissions that job needs.

## Document workbench

Best for a person choosing a folder of Markdown or text files, building a local index, and reviewing findings.

1. Model documents, review state, notes, and tags in SQLite.
2. Ask for one read-only directory grant with `dialog.openDirectory()`.
3. Walk only the granted URI and limit extensions.
4. Store grant URIs, never absolute host paths.
5. Add a bounded index command only when frontend parsing is insufficient.

```typescript
import { getRustFrame } from "rustframe-api";

const rustframe = getRustFrame();
const folder = await rustframe.dialog.openDirectory({ title: "Choose a document folder" });
if (folder) {
  const entries = await rustframe.fs.walk(folder.uri, {
    recursive: true,
    extensions: ["md", "txt"],
    limit: 10_000,
  });
  console.log(`Found ${entries.length} local documents`);
}
```

Start from [Research Desk](https://github.com/OthmaneBlial/rustframe/tree/main/apps/research-desk) and read the [threat model](./threat-model.md) before enabling shell indexing.

## Review queue

Best for structured triage, approvals, incident work, or an internal operator desk.

- Keep queue items, status, assignments, and decisions in one declared table set.
- Use `db.batch()` when one user action changes several records.
- Subscribe to `events.onDatabaseChange()` when secondary windows must stay synchronized.
- Export the queue in an application-owned readable format before introducing sync.

```typescript
import { getRustFrame as getQueueRustFrame } from "rustframe-api";

const queueRustframe = getQueueRustFrame();
const unsubscribe = queueRustframe.events.onDatabaseChange((event) => {
  if (event.tables.includes("work_items")) console.log("Refresh the visible queue");
});

window.addEventListener("beforeunload", unsubscribe, { once: true });
```

## Media library

Best for local image, audio, or video review where original files stay outside the application.

1. Request read access to one library.
2. Save only metadata, decisions, and grant URIs in SQLite.
3. Use `metadata()` before loading a file and enforce product-level size limits.
4. Treat rename, delete, and revoked-grant events as normal user states.
5. Package thumbnails only when the user explicitly chooses an export.

Do not copy an entire library into application data merely to make access easier. The grant boundary is part of the product experience.

## Offline operations tool

Best for a runbook, inventory, field queue, or batch desk that must survive without a server.

- Bundle every production asset and keep the security model `local-first`.
- Make the core create, read, update, search, backup, and export flow work with networking disabled.
- Use fixed or allowlisted shell arguments; never pass user input as a shell source string.
- Show pending local work clearly if optional synchronization is added later.
- Test database backup and restore before packaging.

Run the offline check against the packaged binary, not only the Vite development server. See [Build in 20 minutes](./build-in-20-minutes.md) and the [release checklist](./release-checklist.md).
