# Community Templates

`catalog.json` is the versioned, declarative index for RustFrame's verified workflow templates. Each row points to an `apps/*/.rustframe/template.json`; metadata is not duplicated here and cannot contain executable commands.

Run the full contract with:

```bash
./scripts/verify_templates.sh
```

The gate validates source ownership, author credit, SPDX license, platforms, capabilities, current RustFrame version, screenshots, fixed verification profiles, and the five required workflow shapes. It also regenerates and checks `site/showcase.json`.

Read [`docs/community-templates.md`](../../docs/community-templates.md) before proposing an entry. The current catalog is first-party and must not be presented as evidence of an external ecosystem.
