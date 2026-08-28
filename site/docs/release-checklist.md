# Release Checklist

## Compatibility and source

- Version `rustframe-runtime`, `rustframe-cli`, and `rustframe-api` compatibly.
- Keep manifest schema v1 URLs immutable and add migration guidance for public changes.
- Update `CHANGELOG.md`, API references, error codes, permission references, and the MSRV when relevant.
- Run formatting, Clippy with warnings denied, the full test suite, frontend checks, dependency advisories, and license checks.

## Publish order

1. Publish `rustframe-runtime` to crates.io.
2. Install and smoke-test the packaged runtime dependency, then publish `rustframe-cli`.
3. Publish `rustframe-api` to npm.
4. Build CLI binaries with `dist`/`cargo-dist`, checksums, and provenance.
5. Build Research Desk installers on Linux, Windows, and macOS.
6. Publish versioned documentation and announcement material.

Registry publication is intentionally manual. Run the **Publish registry packages** GitHub workflow with the coordinated version and the confirmation value `publish`. The protected `release` environment must provide `CARGO_REGISTRY_TOKEN` and `NPM_TOKEN`. The workflow refuses existing versions, verifies all three package versions, publishes the runtime first, waits for it to become downloadable, and then publishes the CLI and frontend API. A tag matching the same version separately starts the generated `dist` workflow for CLI binaries, installers, checksums, attestations, and the GitHub Release.

After the GitHub release and coordinated registry publication complete, run the **Public artifact smoke** workflow for the exact release tag. It downloads the release CLI instead of building the checkout, resolves `rustframe-api` from npm, creates a standalone project, compiles the registry runtime, and smoke-launches the result on macOS, Windows, and Linux.

Research Desk uses a separate, fail-closed path. Push an existing tag matching `research-desk-v<packaging.version>`, or select that same tag in the Actions ref picker and manually dispatch **Research Desk trusted release** with confirmation `publish-signed-research-desk`. Dispatching from a branch is rejected so GitHub provenance identifies the release source commit. The workflow will not create a release unless Apple and Windows signing credentials are available, all six native formats build, every transported artifact passes a fresh-host signature and install smoke test, and SBOM/provenance/checksum evidence is assembled successfully.

## App and package verification

- Create a project outside this repository using the release CLI.
- Confirm generated projects contain no repository path dependencies.
- Run `rustframe validate`, `build`, `package --verify`, and `eject` on the standalone project.
- Inspect package manifests and checksums; make unsigned or signed status explicit.
- Run `node scripts/verify_release_artifacts.mjs --dir <bundle> --platform <macos|windows|linux> --require-sbom` against downloaded bundles.
- Install and launch each native artifact, exercise database and filesystem flows, upgrade from the previous release, and uninstall.
- Test backup and restore with valid, invalid, incompatible, and interrupted inputs without losing active data.

The native package workflow installs, smoke-launches, and uninstalls every generated format on its native GitHub runner. Upgrade checks still require a previously published release candidate and must be recorded in the release notes.

## Release candidates

Publish at least one release candidate before stable v1. Do not call a release stable while any critical native-host check is skipped or ignored. Record exact tested operating-system versions and any signing limitations in the release notes.
