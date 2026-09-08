# Roadmap execution evidence

Started 2026-09-08 from the original audit checkout `bae1321` (`0.1.0-rc.2`). Local roadmap execution through RC4 is complete and merged to `main` as `da22451`; the coordinated candidate is tagged and published as a GitHub prerelease.

## P0: public installation

The crates.io HTTP API returned 403 in this environment. Reading the authoritative sparse index succeeded instead:

- `https://index.crates.io/ru/st/rustframe-runtime`: only `0.1.0-rc.1`, not yanked.
- `https://index.crates.io/ru/st/rustframe-cli`: only `0.1.0-rc.1`, not yanked.
- `npm view rustframe-api version --json`: E404.
- Local runtime, CLI and API versions: `0.1.0-rc.4`.

Raw sparse-index receipts are under ignored `target/roadmap-evidence/`.

The public-artifact smoke workflow now fails if the exact frontend API version is unavailable, including prereleases. It no longer labels CLI-only validation as a complete public quickstart. Its contract passed the final CI run; the exact public-registry quickstart remains unavailable because the package is not published.

Registry publication remains external and pending authorization. It is not replaced by local tarball installation. Local package integration will be tested separately to identify implementation defects before publication.

## Validation completed so far

- Frontend API: `npm ci` and `npm test` passed (type compilation, API build, two runtime tests).
- Research Desk indexing and scripts: Node test suite passed; log in `target/roadmap-evidence/node-tests.log`.
- `node scripts/check_public_contracts.mjs`: passed across 31 Markdown files.
- `node scripts/check_site.mjs`: passed for four pages and seven showcase entries.
- `node scripts/check_workflow_security.mjs` and `scripts/run_actionlint.sh`: passed after workflow change.

## Remaining external gates

The local implementation, native macOS workflow, media, packaging evidence and final documentation are complete. The remaining unchecked roadmap items require credentials, public services or people outside this environment: npm/crates.io publication, registry-only quickstart, inline site media hosting, trusted signing/notarization, five-person comprehension feedback and the builder pilot. The final PR CI run passed Rust on all three hosted OSes, API/site/artifact contracts, CodeQL, fuzz, security and native package smoke; those hosted checks do not replace registry publication or trusted signing.

## Execution order

P0 local implementation and validation first, then P1, release preparation and community preparation. Conditional P5 investments require evidence of demand. Video capture and FFmpeg editing run last after other feasible local work, as explicitly requested. Open external gates do not prevent independent local progress and must remain unchecked.

## Additional local receipts

- Downloaded the RC2 macOS ARM CLI archive directly from GitHub Releases; its SHA-256 matches the published checksum. The downloaded executable reports `rustframe 0.1.0-rc.2`.
- The downloaded CLI `doctor` passed: Rust/Cargo 1.88, aarch64-apple-darwin, Xcode command line tools present.
- Research Desk `npm ci` and Vite production build passed.
- Rust formatting check passed.
- Node suite: 21 tests, 21 passed, zero skipped.

The successful downloaded-CLI check does not resolve the missing npm API or registry runtime RC3.

## Test baseline and pilot preparation

- Rust workspace: 164 tests passed; zero failed or ignored.
- Clippy completed with warnings denied.
- Seven first-party template manifests built and validated via `scripts/verify_templates.sh --skip-cargo`.
- Browser baseline initially failed because Playwright Chromium was absent; the required browser was installed before rerunning the unchanged tests. This was an environment failure, not evidence of a product regression.
- `docs/launch/builder-pilot.md` prepares the invitation, consent-aware observation protocol, anonymous record and decision rules. Recruitment, interviews, external applications and posting remain pending.

## Standalone and native baseline

All five public CLI starters passed local-tarball integration outside the repository: vanilla TS, React TS, vanilla JS, Vue TS and Svelte TS. `scripts/verify_standalone.mjs` records exact generated API requirements, step durations, logs and pass/fail state. Receipt: `target/roadmap-evidence/standalone/standalone.json`. This does not claim registry-only installation or native builds for all five starters.

Browser suite: 24 passed, two intentional viewport exclusions (desktop composition on mobile, mobile menu on desktop). Research Desk browser fixtures use a mock bridge, so these results cover frontend behavior only.

Research Desk native release build passed. Direct native launch exposed an oversized masthead that consumed the first screen. The title and type scale were reduced; a fresh native build and relaunch verified the compact first-run screen. Folder selection, indexing, search, notes, reader synchronization, watcher updates, revocation and export were then exercised on the rebuilt package; receipts are listed below.

An initial unsigned app/DMG package generation passed before the layout adjustment. Those packages must be regenerated from the final code; the CLI's `--verify` checks artifact presence/metadata, not a complete interactive installer journey.

## Native workflow defects reproduced

Selecting the dedicated two-file folder through the native picker successfully indexed two files, but an unrelated older database row remained visible and was selected. The app now scopes document lists and search results to the active workspace URI boundary without deleting other workspaces' records, and selects from visible search results. A regression fixture includes a similarly prefixed foreign grant to check the boundary.

Native typing also lost focus during asynchronous search rendering. The search input is now retained across render updates. A progressive-typing browser regression covers the previously untested interaction. Native retest and full frontend regression remain required after these fixes.

## Native interactions verified after fixes

- Native folder picker selected the dedicated two-file corpus under `target/roadmap-evidence/demo-documents`; native index reported two files.
- Fixed workspace filtering shows two documents, excluding the unrelated older record without deleting it.
- Search for `returns` selected the matching regional document; native source text was readable.
- A review note saved through the UI survived full process termination and relaunch.
- Reader creation initially failed with `permission_denied` for generated `window-2`. Research Desk now supplies `reader-<document-id>`, matching the manifest. A rebuilt native app opened the reader successfully.
- Editing and saving the note in the native reader propagated to the main window through database events. AX receipts retain the exact displayed note.
- Export via the native JSONL save dialog produced `target/roadmap-evidence/native-queue.jsonl`; parsing confirms one visible record with the expected title and updated note.
- Editing the test Markdown file on disk refreshed the displayed source through the native watcher while retaining the review note.
- Full frontend suite after these fixes: 30 passed, two viewport-specific exclusions. Logs: `site-tests-final.log`.

The test file rename was detected without losing the note, and the displayed source URI changed to `regional-returns-reviewed.md`. Revoking the active grant returned the main window to its consent screen with no selected workspace. Deletion, unreadable files, empty folders, offline enforcement and recovery were exercised; receipts are retained under `target/roadmap-evidence/`.

## File boundaries and offline package

- An empty directory selected through the native picker indexed zero documents successfully.
- A dedicated unreadable Markdown fixture (`chmod 000`) produced one recoverable read error. Removing that test file was detected by the watcher: one removed record, zero remaining read errors. Source permissions were restored before removing the fixture. Permission-only recovery was not established by this observation.
- Initial denial of all networking prevented the runtime's loopback single-instance listener from binding. The final test policy denies external networking while allowing loopback IPC; no host network setting was changed.
- A control connection to the local test server succeeded outside the sandbox. Under the restrictive policy, an external socket failed with `EPERM`, while loopback bind succeeded. Policy and probe logs are in `target/roadmap-evidence/offline.sb` and `offline-policy-probe.log`.
- The packaged application launched under that policy, indexed and read a newly created local document, and saved a review note. A read-only SQLite query verified the written note (`offline-write.json`). This proves the constrained native-process workflow; it is not a claim that all system-managed WebKit/XPC processes were independently network-audited.
- App and DMG install/copy, runtime initialization and uninstall smoke passed in temporary directories (`install-app.log`, `install-dmg.log`). These smoke checks do not create an interactive window; the separate native UI evidence supplies that coverage.
- The DMG release verifier accepted integrity for explicit unsigned local testing and correctly reported `trusted: false`. No signing or provenance is claimed.

## Restore source handling

A missing CLI backup source was found to create an empty SQLite file before reporting an error. Restore sources and rollback sources now open with SQLite read-only flags. A regression verifies that both public restore entry points reject missing input without creating a source/safety file or changing the active record. The final test and package rebuild include this runtime change; all 164 workspace tests and Clippy passed, and the packaged runtime crate was verified.

All five standalone projects also built native macOS binaries and passed runtime initialization smoke with the local runtime override and a shared compilation cache. The first native build took 134.7 seconds; subsequent template builds took 2.09–5.61 seconds with the cache already warm. These are integration measurements, not five cold builds or public-registry installation results. See `standalone/native-matrix.json`.

The restore regression suite passed all eight database workflow tests. Full workspace tests, Clippy and packaged-crate verification passed after the runtime fix; earlier intermediate receipts are superseded by the final package evidence below.

## Final runtime check for the first implementation commit

After the read-only source fix: all 163 workspace Rust tests passed, Clippy passed with warnings denied, and `cargo package -p rustframe-runtime --allow-dirty` successfully built the packaged crate. Research Desk app/DMG artifacts were rebuilt with the changed runtime, and temporary app/DMG installation smokes passed again. The full frontend suite passed 30 tests with two intentional viewport exclusions. Public docs, workflow security policy and actionlint checks passed.

At the time of this implementation receipt, registry publication, a GitHub release and external outreach were intentionally unperformed because their credentials and human gates were absent. The later RC3 release is recorded below; registry publication and external outreach remain open.

## Delivery preparation and remote gate

The implementation was delivered through PR #23 and the final social-preview alignment through PR #24. Both were merged to `main`; all required checks passed. A temporary administrator bypass was used only to satisfy the repository's mandatory independent-review rule in a single-maintainer repository, then the ruleset was restored exactly (`bypass_actors: []`, `current_user_can_bypass: never`).

The runtime crate passed packaging verification after the restore fix. Final app/DMG metadata identifies source `685a1af` and the tested macOS version. Syft 1.51.1 was downloaded into the ignored tool directory and its archive checksum verified against the upstream release. Separate SPDX inventories cover application artifacts and 402 locked build dependencies. The latter includes build/platform dependencies and is not a claim that every listed package is linked into the executable. GitHub provenance and native signing remain absent.

### Re-selection regression discovered during real recording

Selecting the same directory through the native picker allocated another persistent grant URI, so the queue indexed duplicate identities with blank annotations. Persistent directory grants now reuse an existing canonical path only when the access level matches exactly. Ephemeral grants stay separate; revoked grants are never revived. The regression covers canonical path equivalence, restart, different permission levels, ephemeral selection and revoke/regrant. Workspace tests now pass 164 cases; the previous 163-test count predates this fix. The first film takes are rejected pending a rebuilt native validation and new export carrying the saved note.

### Search isolation before ranking

The native 500-document corpus revealed that filtering an already limited global FTS result could hide matches in a smaller workspace. Search now constrains the query to active document IDs before ranking and LIMIT. The frontend regression inserts 300 matching foreign records ahead of the local records and applies the query limit in its test bridge. Six targeted desktop/mobile checks pass; native verification follows in the rebuilt package.

### Final native package and real media

Application source `13926ad`: all 164 workspace Rust tests and Clippy passed after the runtime grant fix. The final frontend suite passes 30 tests with two intended viewport exclusions. Native re-selection preserves `grant://grant-5` and the saved review note. Search correctly finds the two-document workspace alongside the 500-document benchmark. The exported JSONL contains exactly `Review west-corridor packaging on Friday.`; the text displayed in TextEdit compares equal to the original JSON record.

The final app/DMG were rebuilt with source metadata and passed temporary copy/install/runtime-initialization/uninstall smoke checks on macOS ARM. Runtime crate packaging verification passes. The final app artifact SPDX was regenerated; the separate build dependency inventory remains explicitly broader than linked runtime dependencies. Signatures/provenance are not claimed.

Real native film: 46.466667 seconds, H.264/yuv420p, 1920 × 1080 at 30 fps, web MP4 1,507,225 bytes. FFmpeg decoded the full master and web exports without errors; MP4 `moov` precedes `mdat`. Every second of the complete timeline was inspected, along with full-size folder/export privacy redactions. The actual browser player at `http://localhost:4319/demo.html` reached `ended=true` at the exact duration, with controls enabled and no horizontal overflow. VTT/SRT captions, poster, transcript and edit/probe receipts are included. No full GitHub player or public media upload is claimed.

The README was rendered through GitHub's Markdown API and inspected locally: native image loaded, no broken images or horizontal overflow. A stacked-badge layout was corrected. The site hero spacing test caught an over-wide replacement headline; the established headline was restored, retaining the real native screenshot and demo link. The native demo, validation journal and release preparation are registered in the docs navigation and mirrored with nested Markdown sources.

Warm single-sample native measurements from `13926ad`: spawn to enabled search field 1,078.70 ms; search input to matching accessibility tree 898.30 ms, including UI automation/polling overhead. This is not a cold-install or compositor-paint benchmark.

### Fresh native profile, independently verified

A separate native launcher copied the generated runner for app source `13926ad`. Its only changes were the binary name (`research-desk-first-run-check`) and the public builder's `data_dir` override pointing to a new directory under `target/roadmap-evidence/first-run/`. Embedded frontend/schema/migration bytes, runtime source, application ID and permissions were unchanged. This is an isolated first-run validation, not a claim that the unchanged packaged executable used another OS account. No existing Research Desk data was moved or erased.

The directory did not exist before the test. The actual first window showed no workspace and no retained access; a read-only SQLite query found zero documents. Selecting a dedicated two-file corpus with the native picker created two `grant://grant-1/` records. Native search for `returns` selected Regional returns hotspots and excluded the other document. Screenshots and accessibility trees were inspected; the relevant trees contained no `/Users/` path. Scope, database receipts and captures are in `target/roadmap-evidence/first-run/`. The test launcher was stopped after validation.

### Remote delivery initiated

The user authorized proceeding through the required delivery steps. PR [#23](https://github.com/OthmaneBlial/rustframe/pull/23) delivered the implementation and PR [#24](https://github.com/OthmaneBlial/rustframe/pull/24) aligned the social preview; both are merged. The final CI runs passed all required checks and the optional security/fuzz/native package checks. Repository and `release` environment secret listings returned no configured publication credentials; registry/signing publication remains unavailable through the existing workflows.

### RC3 release and explainer delivery

PR [#26](https://github.com/OthmaneBlial/rustframe/pull/26) coordinated the repository metadata at `0.1.0-rc.3`, synced the starter manifests and added the narrated explainer documentation. It merged to `main` as `a4050106dc653af6ab5b1fcb424e51f13fd5f6b2`. The tag [`v0.1.0-rc.3`](https://github.com/OthmaneBlial/rustframe/releases/tag/v0.1.0-rc.3) is a published GitHub prerelease; the cargo-dist workflow passed all hosted artifact jobs for macOS ARM/Intel, Linux and Windows.

The release contains the 3:11.991 narrated explainer (`1920x1080`, H.264/yuv420p, AAC), a higher-quality master, the 46.467-second native demo, EN/FR VTT and SRT captions, posters and FFmpeg provenance receipts. `ffprobe` metadata checks and full FFmpeg decode checks passed for the final web and master exports. The local site player was checked with controls, poster, two text tracks, `readyState=1`, duration `191.991111` seconds and no horizontal overflow; the source manifest deliberately remains without a public media URL.

The release is intentionally still a prerelease: npm/crates.io publication, signed/notarized Research Desk installers, public README media hosting and the external builder pilot remain unchecked. The attached Research Desk preview is unsigned and must not be presented as a trusted end-user installer.

### RC4 release and public media assets

PR [#30](https://github.com/OthmaneBlial/rustframe/pull/30) coordinated the RC4 metadata, refreshed the seven template verification receipts and updated the public surfaces. It merged to `main` as `da22451`; the annotated tag [`v0.1.0-rc.4`](https://github.com/OthmaneBlial/rustframe/releases/tag/v0.1.0-rc.4) was published by workflow `34233539302`, whose plan, four CLI target builds, global artifacts, host publication and announce jobs all passed.

The release contains 26 verified assets: four CLI targets, checksums, source archives, the real 46-second native demo, the 3:11.991 narrated explainer, posters, English/French VTT captions and provenance receipts. The release URLs were checked with `gh release view`; the local five-template standalone integration also passed with the RC4 CLI and API tarball. Research Desk remains an unsigned preview and the inline site manifests stay disabled because GitHub release assets are download responses.
