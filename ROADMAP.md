# RustFrame Roadmap

> Dependency-ordered roadmap for turning RustFrame into a compelling, trusted, and contribution-friendly open-source product. This is not a promise of virality: stars should follow real developer value, visible proof, and a healthy community loop.

## Product thesis

RustFrame should not become a smaller clone of Electron or Tauri.

RustFrame should own one clear category:

> **The local-workflow desktop kit for frontend teams. Define the data and least-privilege machine access, build the workflow in TypeScript, and ship a native app without maintaining a native project.**

The repeatable job is not “open a WebView.” It is:

- model structured local data in SQLite;
- work safely with user-selected files and folders;
- run bounded machine tasks;
- keep permissions understandable and auditable;
- recover, export, package, and upgrade the application;
- eject to owned Rust only when the stock runtime is no longer enough.

The primary users are TypeScript and JavaScript developers building research desks, document and media workbenches, review queues, offline catalogs, operations tools, and small-team internal applications.

## Current audit

RustFrame already has unusually strong foundations for an early project:

- a standalone CLI and Vite project model;
- TypeScript, JavaScript, React, Vue, and Svelte starters;
- a schema-versioned manifest and deterministic database code generation;
- embedded SQLite with transactions, migrations, backup, and restore;
- opaque filesystem grants, watchers, per-window permissions, bounded commands, and audit records;
- native packaging and multi-host CI;
- a flagship Research Desk workflow;
- threat-model, security, migration, packaging, and contribution documentation;
- published Rust crates, release artifacts, fuzzing, and dependency checks.

The main problem is no longer lack of code. It is a broken and unconvincing public conversion path.

### Critical gaps found in the audit

| Gap | Why it blocks adoption |
| --- | --- |
| `rustframe-api@0.1.0-rc.1` is still unavailable on npm | A generated public project cannot install its required frontend package. The advertised quickstart is therefore broken. |
| `https://rustframe.dev/schemas/v1/rustframe.schema.json` does not resolve | Generated manifests advertise a dead schema URL, damaging editor support and trust. |
| The README leads with `cargo install` while prebuilt CLI binaries already exist | New users pay compilation and toolchain friction unnecessarily. |
| The docs contain contract drift such as `persistent` versus the actual `persist` option | Copy-pasted code can fail even when the runtime is correct. |
| Research Desk packages are unsigned and have almost no public download proof | The flagship is described, but visitors cannot experience a trusted product. |
| The website is visually generic, verbose, and internally focused | It explains the architecture before creating desire, and it does not make the product feel memorable or alive. |
| There is no stable release story, external case study, active issue queue, discussion space, or merged external contributor | Visitors see engineering effort but not an ecosystem or momentum. |
| `main` is unprotected and release workflows still need stronger supply-chain controls | Trust claims should be backed by repository policy, not only badges. |
| Linux is currently X11-only | The cross-platform claim needs an explicit Wayland plan or a precise support boundary. |

## Success model

RustFrame should optimize this funnel:

```text
Understand -> See proof -> Install -> Build one useful workflow
           -> Ship a trusted package -> Ask for help -> Contribute
```

GitHub stars are a lagging indicator, not the operating metric. Measure:

- clean-machine quickstart success on macOS, Windows, and Linux;
- time from install command to first native window;
- generated projects that install only public artifacts;
- release, CLI, npm, and crate downloads;
- Research Desk installs and successful upgrades;
- documentation task completion with first-time users;
- external applications added to the showcase;
- contributor-ready issues claimed and merged;
- first-response time and repeat contributors.

## P0 — Repair the public promise

Nothing should be promoted before this gate is green.

### Publish the complete package set

- Publish `rustframe-api@0.1.0-rc.1` to npm under the `next` tag.
- Verify it while signed out and from a clean npm cache.
- Configure npm Trusted Publishing from a protected GitHub release environment after the initial 2FA-authorized publication.
- Publish runtime, CLI, and API versions in dependency order from one coordinated release workflow.
- Refuse a release when one of the three public packages is absent or has a mismatched version.
- Test `npm install`, TypeScript compilation, runtime loading, and `npm pack` from the exact registry artifact.

Exit criteria:

- `npm view rustframe-api@0.1.0-rc.1` succeeds publicly;
- a fresh generated project installs with no local tarball or repository override;
- registry install tests pass on all supported hosts;
- release credentials are never stored in the repository or exposed to pull requests.

### Fix the schema and documentation contract

- Host the immutable v1 schema at a real, monitored HTTPS URL.
- Either configure `rustframe.dev` correctly or change the pre-stable generated URL to a durable Pages-owned path.
- Add an HTTP/content test for every public schema URL.
- Compile every TypeScript documentation snippet in CI.
- Run every shell quickstart against public artifacts.
- Fix `persistent`/`persist` drift and add a terminology/API consistency check.
- Add versioned documentation so RC and stable instructions cannot silently mix.

Exit criteria:

- the schema URL returns the expected JSON with the correct content type;
- VS Code resolves and validates a generated `rustframe.json`;
- all copyable examples are CI-tested;
- a docs link and snippet checker runs on every pull request.

### Make the fastest install path the default

- Put the prebuilt CLI installer first in the README, website, and getting-started guide.
- Keep `cargo install` as the transparent source-build alternative.
- Add Homebrew and Scoop installation only when those formulas are release-tested automatically.
- Make `rustframe doctor` print exact host-specific remediation commands and links.
- Record install source and CLI version in `rustframe doctor --json` without telemetry.

Exit criteria:

- the CLI installs without compiling RustFrame itself on supported hosts;
- checksum and GitHub attestation verification are documented beside each command;
- a clean host reaches `rustframe --version` through one primary command.

### Prove activation from public artifacts

Create a release gate that runs only against published packages and downloaded binaries:

```text
install CLI -> rustframe doctor -> rustframe new --install
-> rustframe dev -> typed SQLite mutation -> rustframe build
-> rustframe package --verify -> install -> launch -> uninstall
```

Publish the receipts for macOS, Windows, and Linux. Measure cold and warm activation separately instead of making unsupported speed claims.

## P1 — Rebuild the website and documentation experience

The current site at `https://othmaneblial.github.io/rustframe/` needs a complete redesign, not a palette adjustment.

### New creative direction

- Replace the generic dark-card/neon-grid look with a distinctive “local workbench” visual system inspired by native tools, structured data, files, and permission boundaries.
- Create a recognizable RustFrame mark, typography system, icon language, color system, motion rules, and screenshot treatment.
- Remove inward-facing copy such as “wedge,” “do not pretend,” and framework-planning language from the marketing surface.
- Use short, confident product language backed by visible proof.
- Make the design feel like a serious desktop product, not a generated SaaS landing page.

### New information architecture

```text
Home
├── Why RustFrame
├── Download / Install
├── Research Desk
├── Use cases
├── Security and local data
├── Compare
└── Community

Docs
├── Quickstart
├── Guides
├── API reference
├── Manifest reference
├── Capabilities
├── Packaging and release
├── Migration
└── Troubleshooting

Showcase
├── Real applications
├── Verified templates
└── Submit an app
```

### Homepage that converts

The first screen must contain:

- one concrete sentence explaining the category;
- a short real recording of `new -> dev -> packaged app`;
- a polished Research Desk screen in a native window;
- primary CTA: **Install RustFrame**;
- secondary CTA: **Download Research Desk**;
- a small, honest host-support and release-status indicator;
- a link to “When should I use Tauri, Electron, or something else?”

The remainder should show, in this order:

1. the workflow RustFrame removes;
2. the local data and permission model;
3. a real developer transcript;
4. the flagship app;
5. packaging proof;
6. an honest comparison;
7. community applications and contributors;
8. one final install CTA.

### Interactive proof

- Build an in-browser manifest and capability explorer.
- Let visitors edit window permissions and see the effective policy change.
- Show schema-to-TypeScript generation with a small real example.
- Provide a downloadable generated starter after validation.
- Never simulate a native capability and present it as a live desktop result.

### Documentation product

- Add fast full-text search, stable deep links, version selection, copy buttons, previous/next navigation, and visible platform notes.
- Generate API reference from the published TypeScript types and manifest schema.
- Add task-oriented guides for document workbenches, review queues, media libraries, and offline operations tools.
- Put errors, diagnostics, and troubleshooting next to the step where they occur.
- Use real screenshots and terminal output from the current release.

### Quality bar

- responsive from 320 px through large desktop screens;
- no horizontal overflow at supported viewports;
- keyboard-complete navigation with visible focus and restored focus after menus/dialogs;
- WCAG 2.2 AA contrast and semantics;
- reduced-motion support;
- Lighthouse performance, accessibility, best-practices, and SEO scores of at least 95 on representative pages;
- LCP under 2.5 seconds and CLS under 0.1 on a typical mobile connection;
- correct Open Graph/social preview, canonical URLs, sitemap, robots rules, and software/project structured data;
- browser tests for navigation, code copying, search, mobile menu, external links, and zero console errors.

Exit criteria:

- five unfamiliar frontend developers can explain RustFrame, identify whether it fits them, and find the install command without assistance;
- the site links only to existing public packages and current release artifacts;
- the visual identity is consistent across site, docs, social preview, screenshots, and release pages;
- production Pages behavior is verified on desktop and mobile after deployment.

## P2 — Turn Research Desk into undeniable proof

Research Desk should become a product people would install even if they did not care which framework built it.

### Product completion

- polished first-run folder selection and consent explanation;
- fast incremental indexing with cancellation, progress, and recoverable errors;
- SQLite FTS5-backed search with highlighted matches;
- collections, tags, notes, status, saved filters, pinning, and synchronized reader windows;
- rename/delete/revoke handling that tells the truth about lost access;
- portable JSON/JSONL/CSV export in addition to database backup;
- “show my data,” “export everything,” and safe “delete local data” flows;
- crash/diagnostic bundle export with sensitive paths redacted;
- upgrade and rollback fixtures across schema versions;
- complete keyboard use and accessibility QA.

### Trusted downloads

- Developer ID sign, notarize, and staple the macOS build.
- Authenticode-sign the Windows executable and installers.
- Verify signatures after downloading, not only during the build.
- Record SBOM, checksums, provenance, signature state, and tested OS version in release metadata.
- Provide one obvious download per host, with advanced formats below it.
- Never instruct ordinary users to bypass Gatekeeper or Windows security warnings for the flagship stable build.

### Proof media

- one 30–45 second silent product recording for the README and homepage;
- one complete “build this workflow” video with chapters;
- screenshots for first run, search, permission revocation, backup/restore, and packaging;
- a reproducible benchmark page for package size, cold start, memory, indexing, and warm rebuilds;
- an architecture case study that uses only public RustFrame APIs.

Exit criteria:

- a clean machine installs and launches the downloaded app without security-bypass instructions;
- the core read/search/review/export flow works with the network disabled;
- every advertised capability is traceable to public APIs and an automated test;
- the release page is useful to end users, not only framework maintainers.

## P3 — Make local ownership and security visible

RustFrame already has a stronger security model than its public proof suggests. Turn it into product behavior.

### Local-first conformance report

Add `rustframe inspect --local-first` in human and stable JSON formats. Report:

- bundled versus remote assets;
- network model and CSP;
- database, migrations, backup, and portable export support;
- filesystem roots and persisted-grant policy;
- window capabilities and shell commands;
- packaging, signing, and update policy;
- undeclared remote dependencies or unsafe exceptions.

Package a redacted report and policy hash with each artifact. Add an offline packaged-app test that proves the flagship workflow survives without a server.

### Explainable capabilities

Add a command family such as:

```bash
rustframe capabilities explain
rustframe capabilities diff old.json new.json
rustframe capabilities check --deny-expansion
```

It must answer which window can perform which operation against which scope. CI should flag new shell commands, wider roots, persisted grants, remote content, or privilege expansion until explicitly reviewed.

### Data ownership kit

- consistent transactional export of declared tables to JSON, JSONL, and CSV;
- export manifest with app ID, schema/export versions, row counts, and checksums;
- streaming behavior for large datasets;
- compatibility tests across schema evolution;
- application-owned rich export formats layered above the runtime primitive.

### Release verification

Add `rustframe release verify <artifact> --json` to check checksums, provenance, macOS signing/notarization/stapling, and Windows Authenticode where applicable. Generated package metadata must describe observed verification, never a caller-supplied claim.

## P4 — Make the developer loop feel exceptional

### Faster path to the first window

- benchmark CLI install, first native compile, warm rebuild, frontend reload, and package generation;
- cache the generated runner and dependencies by runtime/target/features;
- keep frontend hot reload independent of native recompilation;
- explain the slowest step instead of appearing frozen;
- investigate a precompiled stock runner that loads a validated app bundle, so non-ejected projects may eventually avoid a Rust toolchain;
- accept that architecture only if capability enforcement, asset integrity, debugging, and package size remain strong.

### Better diagnostics

- `rustframe doctor --json` with stable error codes and remediation URLs;
- `rustframe dev --open-devtools` and structured native/frontend logs;
- redacted `rustframe diagnostics export` bundle;
- clear failure states for missing WebView dependencies, ports, stale types, invalid grants, signing, and packaging tools;
- tested error copy that tells the developer what happened, why, and the next command.

### Desktop workflow essentials

Prioritize primitives that strengthen the owned use case:

- single-instance behavior and file-open routing;
- file associations for workspace/document applications;
- background job progress and cancellation;
- FTS5 schema/codegen support;
- structured audit/run receipts;
- reliable ejection compatibility tests.

Do not add tray, global shortcuts, notifications, deep native APIs, or an updater merely to match a competitor checklist. Add them only when real applications demonstrate repeated demand.

## P5 — Build a contribution and template flywheel

### Open real contribution paths

- Enable GitHub Discussions with Q&A, ideas, “show what you built,” and announcements.
- Seed a welcome post and explain when a discussion becomes an accepted issue.
- Publish a maintained queue of 8–12 genuinely executable issues.
- Every contributor-ready issue must include context, affected area, acceptance criteria, test command, and non-goals.
- Use `good first issue` only for low-context work and `help wanted` only after a design is accepted.
- Add `ready-for-contributor`, `needs-design`, `needs-reproduction`, `blocked`, and `maintainer-only` labels.
- Expand `CONTRIBUTING.md` with a component map, targeted test loops, local-crate example workflow, docs preview, and issue-claim policy.

### Verified template registry

- Define a versioned template manifest.
- Validate templates against supported CLI/runtime/API versions in CI.
- Offer workflow-shaped templates, not cosmetic framework clones: document desk, media review queue, offline inventory, evidence tracker, and batch operations console.
- Generate screenshots and compatibility badges from automated builds.
- Let community authors submit templates without giving the catalog arbitrary code execution.
- Archive or mark stale entries automatically when they stop passing.

### Showcase real applications

- Replace generic reference-card copy with installable external applications.
- Require source, license, current screenshot, supported platforms, RustFrame version, and reproducible build.
- Publish short builder case studies: problem, why RustFrame, what was difficult, and measured result.
- Credit contributors and application authors in release notes and on the site.

Exit criteria:

- a first-time contributor can find, run, test, and complete a scoped task without private guidance;
- at least three external applications or templates pass the public verification contract before the showcase implies an ecosystem;
- questions receive a useful first response and accepted work does not remain ambiguous.

## P6 — Harden trust and ship a stable contract

### Repository and supply chain

- Protect `main` with required CI, review, and no force pushes.
- Run releases only from protected tags/environments.
- Pin third-party GitHub Actions to immutable commit SHAs and maintain them automatically.
- Add Dependabot or Renovate, CodeQL, OpenSSF Scorecard, SBOM generation, and provenance verification.
- Keep workflow permissions least-privilege and test them with a workflow security linter.
- Add `CODEOWNERS` for runtime, CLI, schema, API, packaging, and security-sensitive paths.
- Publish a version support and vulnerability-response policy.

### Stable-release gate

Do not call RustFrame stable until:

- all coordinated packages are publicly installable;
- the schema URL and versioned docs are durable;
- clean-machine activation passes on every supported host;
- Research Desk is signed, trusted, offline-capable, and upgrade-tested;
- public API, permission, error, manifest, database, and migration compatibility gates pass;
- the ejection path preserves behavior and has upgrade guidance;
- platform limitations, including Wayland, are accurately documented;
- at least several external builders have completed the quickstart and their feedback is resolved or explicitly deferred.

Resolve the naming mismatch before this release: manifest schema v1 and package version `0.1.x` may coexist, but “public v1,” “release candidate,” and “stable” must each mean one precise thing.

## Discovery and launch loop

Promotion begins only after P0 is green and the redesigned site points to working artifacts.

For each meaningful release:

1. publish an outcome-focused changelog and upgrade path;
2. verify exact public artifacts from clean environments;
3. update the demo and benchmark receipts when behavior changes;
4. publish one technical story around a real workflow or engineering decision;
5. share the result in relevant Rust, frontend, local-first, and desktop communities without asking for stars;
6. invite concrete feedback through Discussions and a reproducible issue path;
7. credit users and contributors in the next release.

Repository discovery work:

- replace vague topics such as `alternative` with accurate topics such as `local-first`, `offline-first`, `typescript`, and `developer-tools`;
- create a custom social preview consistent with the redesigned site;
- keep repository description, homepage, README status, and latest install command synchronized;
- add an honest comparison page using reproducible evidence rather than unsupported superiority claims;
- publish launch posts only when the demonstrated artifact is ready for the traffic.

## Explicit non-goals

Until real demand changes the decision, RustFrame will not pursue:

- mobile targets;
- a bundled Chromium runtime;
- a general Tauri-style plugin marketplace;
- built-in accounts, cloud storage, or a sync vendor;
- a generic CRDT/collaboration engine;
- arbitrary native or shell access from the frontend;
- pixel-identical rendering across operating systems;
- cross-host installer builds presented as native verification;
- an unsigned background updater;
- dozens of decorative demo apps that do not prove new public behavior.

## Definition of “amazing”

RustFrame is compelling when:

- a visitor understands its unique job in 30 seconds;
- a frontend developer opens a native app from a normal Vite project without reading Rust code;
- the flagship download feels like a trustworthy product, not a framework demo;
- a user can see permissions, work offline, export readable data, back up, restore, and leave;
- a reviewer can diff the effective security policy and verify the shipped artifact;
- the website is visually memorable and proves the claims instead of narrating them;
- a contributor can pick a bounded issue and merge a verified improvement;
- community projects create the next examples, guides, and contributors.

That is the path to sustained GitHub attention: a narrow promise, a frictionless first success, visible proof, trustworthy releases, and a contribution loop that compounds.

## Research basis

This roadmap combines a source audit with current official guidance and competitor documentation:

- [Tauri capabilities and permissions](https://v2.tauri.app/security/capabilities/)
- [Tauri plugin surface](https://tauri.app/plugin/)
- [Electron process model](https://www.electronjs.org/docs/latest/tutorial/process-model)
- [Electron distribution](https://www.electronjs.org/docs/latest/tutorial/distribution-overview)
- [Wails introduction](https://wails.io/docs/introduction/)
- [Neutralinojs documentation](https://neutralino.js.org/docs/)
- [Dioxus](https://dioxuslabs.com/)
- [Local-First Software paper](https://www.inkandswitch.com/essay/local-first/local-first.pdf)
- [GitHub repository topics](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/classifying-your-repository-with-topics)
- [GitHub issue and pull-request templates](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/about-issue-and-pull-request-templates)
- [GitHub Discussions](https://docs.github.com/en/discussions/quickstart)
- [OpenSSF Scorecard](https://scorecard.dev/)
- [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [Microsoft Windows distribution guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/choose-distribution-path)
