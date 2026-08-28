# Research Desk

Research Desk is a private, local-first workbench for reviewing Markdown and text archives. Choose one folder, search it with embedded SQLite FTS5, capture decisions, and export or back up the result without uploading the source material.

## What you can do

- choose and later revoke one read-only folder grant;
- incrementally index changed files with visible progress and safe cancellation;
- search title, summary, tags, reviewer, and notes with highlighted matches;
- organize work with collections, status, priority, saved views, notes, and pinning;
- open synchronized reader windows while the original files remain in place;
- export the visible queue as JSON, JSONL, or CSV, or export all app data;
- create and restore consistent SQLite backups;
- inspect, export diagnostics for, or safely erase local app data without deleting source files.

## Choose a download

The release index marks one primary download per host:

- macOS: notarized and stapled `.dmg`;
- Windows: signed and timestamped NSIS `.exe`;
- Linux: AppImage.

Advanced `.app`, `.msi`, and `.deb` formats are also attached. Every release includes `SHA256SUMS`, an SPDX SBOM, per-host verification evidence, and a machine-readable `research-desk-release-index.json`.

## Privacy and storage

Source documents stay in the selected folder. Review state and settings live in the app's local SQLite database. Research Desk has no cloud account, telemetry endpoint, or remote database. The in-app data control center shows the exact local locations and provides export, backup, revoke, and protected deletion flows.

## Upgrade safety

Database version 2 adds content fingerprints for reliable rename detection. The production migration preserves existing notes, pinning, and review status. RustFrame refuses to open a newer database with an older embedded schema; restore the pre-upgrade SQLite backup instead of forcing a destructive downgrade.

## Verify a download

Compare the artifact with `SHA256SUMS`, then verify its GitHub build attestation:

```bash
gh attestation verify <download> --repo OthmaneBlial/rustframe
```

The macOS and Windows release jobs additionally verify the downloaded native signature on a fresh host before GitHub publishes the release. Do not bypass Gatekeeper or Windows security warnings: if native verification fails, the release workflow fails closed.
