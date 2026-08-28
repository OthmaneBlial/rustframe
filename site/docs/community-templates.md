# Community Templates

RustFrame's template registry is a versioned verification contract, not a gallery of renamed CRUD demos. Every listed template states the job it serves, source, author, license, platforms, RustFrame version, native capabilities, current screenshot, and verification date.

The current registry is explicitly **first-party**. It proves the contribution machinery and five useful workflow shapes without implying that an external ecosystem already exists.

## Public contracts

- Catalog: `examples/community-templates/catalog.json`
- Catalog schema: [`schemas/templates/v1/catalog.schema.json`](../schemas/templates/v1/catalog.schema.json)
- Template schema: [`schemas/templates/v1/template.schema.json`](../schemas/templates/v1/template.schema.json)
- Template manifests: `apps/*/.rustframe/template.json`
- Generated showcase data: `site/showcase.json`
- Validator and generator: `scripts/validate_template_registry.mjs`

The immutable schemas are also published at:

```text
https://othmaneblial.github.io/rustframe/schemas/templates/v1/catalog.schema.json
https://othmaneblial.github.io/rustframe/schemas/templates/v1/template.schema.json
```

## Verified workflow set

| Workflow | Template | Source |
| --- | --- | --- |
| Document desk | Daybreak Notes | `apps/daybreak-notes` |
| Media review queue | Prism Gallery | `apps/prism-gallery` |
| Offline inventory | Meridian Inventory | `apps/meridian-inventory` |
| Evidence tracker | Research Desk | `apps/research-desk` |
| Batch operations console | Dispatch Room | `apps/dispatch-room` |

Workflow Queue Starter and Quill Studio remain additional first-party starting points. A template is not "verified" because its card looks finished; its manifest and project must pass the current CLI and fixed verification profile.

## Manifest v1

The catalog contains only manifest paths:

```json
{
  "$schema": "https://othmaneblial.github.io/rustframe/schemas/templates/v1/catalog.schema.json",
  "schemaVersion": 1,
  "templates": [
    { "manifest": "apps/daybreak-notes/.rustframe/template.json" }
  ]
}
```

Each template manifest records declarative evidence. Unknown properties are rejected. Commands, scripts, CI fragments, absolute paths, and path traversal are not valid metadata.

Required evidence includes:

- a specific workflow and audience;
- canonical source path and author credit;
- SPDX license and real license file;
- supported platforms and exact RustFrame version tested;
- declared native capabilities;
- verification state, fixed profile, and date;
- an optimized WebP screenshot with checked dimensions and descriptive alt text.

## Safe verification

Run the complete template gate from the repository root:

```bash
./scripts/verify_templates.sh
```

The validator resolves accepted in-repository paths, rejects stale versions and missing evidence, then generates `site/showcase.json`. Only after the declarative catalog passes does the verification script use one of two maintainer-owned profiles:

- `rustframe-static-v1` for dependency-free static workflow frontends;
- `rustframe-flagship-v1` for Research Desk.

The catalog cannot supply the command that CI executes. This prevents a new catalog row from becoming an arbitrary-code hook. Pull-request code still receives normal source review and runs with GitHub's untrusted-fork restrictions.

## Submit a template or application

1. Open the [template or app submission form](https://github.com/OthmaneBlial/rustframe/issues/new?template=template_submission.yml).
2. Link an exact public revision, license, author profile, real screenshot, and clean-build evidence.
3. Explain the user job and least-privilege capabilities. "It has CRUD" or "it looks nice" is not a workflow.
4. A maintainer reviews the source and reproduction steps before any code is run.
5. An accepted in-repository template adds its app, `.rustframe/template.json`, WebP screenshot, and one catalog manifest path.
6. Run `./scripts/verify_templates.sh` and the relevant browser tests.

Community authors retain visible credit in the manifest, showcase, and release notes. Entries move to `reference` or `archived` when their source, license, screenshot, platform proof, or tested RustFrame version becomes stale.

## External showcase gate

The site must continue to label all current entries as first-party. It may describe a community ecosystem only after at least three independently authored applications or templates pass this same public contract. Until then, submissions can be reviewed and discussed without inflating adoption claims.
