# Portable Data Exports

RustFrame database backups are the lossless recovery format. Portable exports are the human-readable exit format: declared tables become JSON, JSONL, or CSV files with a versioned manifest and SHA-256 checksums.

## Export one consistent snapshot

```bash
rustframe db export --format json
rustframe db export ./handoff --format jsonl --batch-size 500
rustframe db export ./spreadsheet-handoff --format csv
```

Relative destinations resolve inside the RustFrame project. The command refuses to replace an existing directory.

Before reading rows, RustFrame creates a SQLite backup snapshot. Every table is then paged from that immutable copy in primary-key order. Concurrent application writes cannot produce row counts from one moment and table files from another.

`--batch-size` accepts 1 through 10,000 and controls peak row-buffer memory. JSON arrays, JSONL records, and CSV rows are written incrementally; the exporter does not collect the whole database in memory.

## Output contract

```text
handoff/
├── export-manifest.json
├── SHA256SUMS
└── tables/
    ├── documents.jsonl
    └── settings.jsonl
```

`export-manifest.json` is schema version 1 and records:

- `kind: rustframe.portable-data-export`;
- application ID;
- database schema version and export version;
- selected format and collection time;
- `consistentSnapshot: true`;
- file, row count, byte count, and SHA-256 digest for every declared table.

`SHA256SUMS` covers every table file and the export manifest. Verify it before importing or transferring the directory:

```bash
shasum -a 256 -c SHA256SUMS
```

## Format behavior

### JSON

Each table is one JSON array. JSON columns remain real arrays or objects. This format is convenient for application-level importers and small to medium handoffs.

### JSONL

Each line is one complete JSON object. The format streams naturally into command-line tools and data pipelines and remains usable when a table is large.

### CSV

Each table has a stable header: `id`, declared schema columns, `createdAt`, and `updatedAt`. Commas, quotes, and newlines use RFC-style double-quote escaping. JSON values remain serialized JSON text inside their CSV field so structure is not silently discarded.

## Compatibility boundary

The export manifest identifies both database schema and export schema versions. An application-owned importer should reject unsupported versions explicitly and map known older schema versions through reviewed migrations.

Rich product exports can add Markdown, media bundles, or domain-specific summaries. Keep them above this runtime primitive; the RustFrame export exists so every application has a predictable, checksummed way to leave.
