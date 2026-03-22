# Community Templates

RustFrame should grow template surface only where the workflow shape is already credible.

That means:

- start from real jobs, not renamed CRUD demos
- map templates to existing reference apps or starters
- keep the capability story explicit

## Current Catalog

The machine-readable template catalog lives at:

```text
examples/community-templates/catalog.json
```

It currently points builders at:

- the workflow queue starter in `apps/hello-rustframe`
- the flagship `apps/research-desk`
- dense status-board and editorial references such as `apps/dispatch-room` and `apps/quill-studio`
- frontend starters such as `examples/frontend-starters/svelte-vite`

## What Makes A Good Template

A good community template should answer these questions clearly:

- what job is this for?
- who would adopt it?
- which runtime capabilities does it use?
- why is RustFrame a good fit for this workflow?

If the answer is just "it looks nice" or "it has CRUD," it is not enough.

## Contribution Rules

Prefer templates that are:

- file-centric
- local-first
- workflow-shaped
- narrow in native capability scope

Avoid templates that are:

- generic dashboards with fake data only
- broad framework demos with no clear user task
- examples that imply unsupported native depth

## Suggested Contribution Format

When adding a new template entry:

1. add or update the reference app or starter
2. add an entry to `examples/community-templates/catalog.json`
3. describe the workflow fit and capabilities honestly
4. add a screenshot if the UI materially helps explain the job

## Why This Exists

The template catalog is the ecosystem layer that should help people start faster without pushing RustFrame back into "generic wrapper" positioning.
