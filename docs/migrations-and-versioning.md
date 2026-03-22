# Migrations And Versioning

RustFrame separates schema evolution from app packaging versioning.

## Database Versioning

`data/schema.json` carries the database schema version.

Use it to describe the latest desired shape of the local SQLite database.

Rules:

- increase the schema version when the shipped database contract changes
- keep `data/seeds/*.json` for first-run defaults only
- add SQL migrations for non-additive changes

## Seed Files

Seeds are for initial rows only.

Treat them as immutable after release because existing users will not replay them automatically.

Good seed content:

- default settings
- starter lanes
- sample rows for fresh local installs

Bad seed usage:

- renaming live columns
- backfilling old rows
- changing production data after users already ran version 1

## Migration Files

Put migrations in:

```text
data/migrations/
```

Name them with a numeric prefix:

```text
002-rename-priority.sql
003-backfill-review-state.sql
```

RustFrame applies them in version order during upgrades before the runtime falls back to additive schema reconciliation.

## Packaging Versioning

`rustframe.json` packaging metadata carries the shipped app version:

```json
{
  "packaging": {
    "version": "0.1.0"
  }
}
```

Use that for the distributed bundle identity.

Do not confuse it with the database schema version. They move together sometimes, but they solve different problems.

## A Practical Release Flow

1. Update `data/schema.json` to the new desired shape.
2. Add SQL migrations for renames, drops, backfills, or type changes.
3. Leave old seed files alone unless the change is only for brand-new installs.
4. Run `rustframe-cli inspect <app>` and check the schema diagnostics.
5. Reset local dev data only when you intentionally want a fresh install simulation.
6. Bump `packaging.version` when you build the next distributed bundle.

## Development Commands

Useful commands while iterating:

```bash
cargo run -p rustframe-cli -- inspect hello-rustframe
cargo run -p rustframe-cli -- reset-data hello-rustframe
```

`inspect` shows the resolved schema version, seeds, migration files, and diagnostics.

`reset-data` deletes the local app data directory so the next `dev` run recreates the database from the embedded assets.
