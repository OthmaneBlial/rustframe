# Choosing RustFrame

## The One-Sentence Answer

Use RustFrame when you are building a local-first desktop workflow tool that should stay mostly frontend code, but still needs a native shell, embedded SQLite, and a small amount of scoped machine access.

## Strong Fit

RustFrame is a good fit when:

- the app is still mostly HTML, CSS, and JavaScript
- local data matters more than cloud-first sync on day one
- you want the runtime to own SQLite, packaging, and capability wiring
- you need scoped filesystem access or allowlisted shell commands
- you want to start without a visible native project and eject later only if needed

Typical jobs:

- a local research desk
- a media review or tagging workbench
- a document or asset organizer
- an operations runbook desktop tool
- an internal workflow app with offline data

## Bad Fit

RustFrame is a poor fit when:

- you already know you need deep native integrations immediately
- you need a mature plugin ecosystem
- the product works perfectly well as a browser tab or PWA
- you need Chromium-level rendering consistency across every host
- your team already has a solid Tauri or Electron setup and no friction with it

## Honest Comparison

| Question | Browser / PWA | RustFrame | Tauri | Electron |
| --- | --- | --- | --- | --- |
| Packaged desktop app by default | No | Yes | Yes | Yes |
| Visible native project from day one | No | No | Usually yes | Usually yes |
| Embedded SQLite managed by the runtime | No | Yes | Possible, app-owned | Possible, app-owned |
| Scoped filesystem access through manifest config | Limited | Yes | Possible | Possible |
| Allowlisted local command execution | No | Yes | Possible | Possible |
| Broad native API surface and ecosystem today | No | No | Yes | Yes |
| Chromium rendering consistency | Browser-dependent | OS WebView | OS WebView | Yes |
| Best use case | Web apps | Local-first workflow tools | General Rust desktop apps | Broad desktop apps with Chromium assumptions |

## Practical Decision Rule

Choose the browser when:

- you do not need packaging, local SQLite, or machine access
- the app can live happily on the web

Choose RustFrame when:

- the app is mostly frontend
- local-first data is central
- you want a native shell plus a small, explicit native surface
- you want a hidden-runner path before committing to a full native project

Choose Tauri when:

- you want a more mature Rust desktop ecosystem
- you expect broader native integration
- owning the native project structure is acceptable

Choose Electron when:

- you need Chromium behavior everywhere
- you depend on Electron's ecosystem or desktop assumptions
- bundle size is less important than consistency and reach

## What RustFrame Is Not Trying To Beat

RustFrame is not trying to beat Tauri or Electron at breadth.

It is trying to be a better fit for one narrower job:

> frontend-first, local-first desktop workflow tools that need a native shell without a large visible desktop-framework footprint.

That narrower goal is the whole point.
