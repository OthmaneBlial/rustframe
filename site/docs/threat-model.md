# Threat Model

RustFrame assumes the frontend is trusted only when you say it is.

## Default Posture

- `local-first` is the default trust model
- the database bridge is available when the app ships `data/schema.json`
- filesystem and shell access stay off unless the manifest declares them
- the runtime rejects disabled bridges in both native IPC and the injected JS API

This means a new app starts with a small surface even before you tighten anything further.

## Trust Models

### `local-first`

Use this only when the frontend is effectively part of the trusted app.

Typical case:

- bundled frontend assets
- no third-party scripts
- no remote HTML rendering
- low XSS exposure

### `networked`

Use this when the frontend behaves more like a hosted web app.

Typical case:

- remote scripts
- user-authored HTML or unsafe markdown rendering
- embedded third-party widgets
- meaningful XSS risk

In `networked` mode, the window bridge remains available and the other bridges stay off unless you re-enable them deliberately.

## Filesystem Rules

Prefer narrow roots:

- good: `workspace/`
- good: `${EXE_DIR}/imports`
- bad: `/`
- bad: the whole home directory unless the product truly needs it

Why:

- smaller blast radius if the frontend is compromised
- easier packaging because the app owns the data boundary
- easier reasoning in review and support

## Shell Rules

Prefer named jobs over general shell access.

- expose `indexWorkspace`, not `/bin/sh`
- keep args fixed or strictly allowlisted
- set `cwd`, `timeoutMs`, and `maxOutputBytes`
- review env vars before you pass them through

The dev audit log exists so you can see what the UI actually asked the runtime to execute while shaping the contract.

## Packaging And Distribution

Packaging does not change the threat model by itself.

You still need to decide:

- what the frontend can access
- what local roots ship with the bundle
- which commands are allowed
- whether the app should stay `local-first` or move to `networked`

Use `rustframe-cli inspect <app>` before packaging and `rustframe-cli package <app> --verify` after packaging.

## Safe Default Checklist

- start in `local-first` only if the frontend is bundled and trusted
- move to `networked` when remote content enters the picture
- scope filesystem access to one product-owned root
- allowlist commands one by one
- keep seeds immutable after release
- use migrations for schema evolution
