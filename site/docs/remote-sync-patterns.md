# Remote Sync Patterns

RustFrame is local-first by default. Remote sync should be added as an app pattern, not assumed by the runtime.

## Rule Of Thumb

If the product needs sync, keep the local SQLite database as the primary UX surface and treat the network as a replication path.

That usually gives you:

- better offline behavior
- clearer trust boundaries
- easier error recovery

## Pattern 1: Pull-Only Reference Sync

Use this when the app mostly reads remote data and annotates it locally.

Shape:

- fetch remote records or snapshots
- write the imported state into SQLite
- keep local review state or notes separate from the imported fields

Good for:

- research desks
- catalog browsers
- internal read-heavy tools

## Pattern 2: Import / Export Loop

Use this when the team does not need live sync but does need handoff.

Shape:

- import a structured file or archive
- work locally in SQLite
- export a queue, patch set, or reviewed output

Good for:

- compliance packages
- editorial handoff
- ops review cycles

## Pattern 3: Background Reconcile Worker

Use this when the app needs periodic upstream sync without turning the frontend into a network shell.

Shape:

- keep the frontend focused on local data
- expose one allowlisted sync job through the shell capability or an app-owned native runner
- write sync results back into SQLite
- log failures and partial state explicitly

Good for:

- internal tools with scheduled sync
- source-of-truth mirrors
- low-frequency team data refresh

## Pattern 4: Full Bidirectional Sync

Use this only when the product truly needs it.

Requirements usually include:

- conflict handling
- record identity strategy
- retry and idempotency rules
- auth and token management
- migration compatibility across synced versions

This is a product-level feature, not a runtime primitive.

## Trust And Security

If the frontend loads remote content, third-party scripts, or user-authored HTML:

- move the app to `networked` mode
- keep local bridges minimal
- avoid broad filesystem or shell exposure

## What RustFrame Helps With

- local SQLite remains fast and deterministic
- `inspect` makes the local app contract visible
- `package --verify` helps keep distribution predictable
- the runtime does not force a specific sync vendor or backend

## What RustFrame Does Not Yet Do

- no runtime-owned sync engine
- no replication protocol
- no conflict-resolution framework
- no built-in auth abstraction

That is intentional for now.
