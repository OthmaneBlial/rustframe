# Research Desk database fixtures

`schema-v1.json` is the immutable schema shipped before content fingerprints. The current
`data/schema.json` is version 2, and `data/migrations/002-add-content-fingerprint.sql`
upgrades existing databases without replacing user rows.

The Rust integration test opens a version 1 database, inserts real review state, upgrades
it with the production version 2 schema and migration, and checks that the row survives.
It then attempts to open that version 2 database with the version 1 fixture and verifies
that RustFrame rejects the unsafe downgrade. Recovery uses the pre-upgrade SQLite backup,
not a destructive reverse migration.
