# Update Strategy

RustFrame does not currently ship a built-in auto-update system.

That is a deliberate boundary for now.

## Recommended Update Story Today

Use a manual or host-assisted update flow:

1. publish versioned bundles for each host OS
2. surface the current app version in the UI or release notes
3. let users download the new bundle
4. reinstall or replace the existing app with the new packaged output

This is enough for many internal tools and small-team desktop workflows.

## Why This Is The Current Recommendation

- it keeps the runtime surface smaller while the core workflow wedge is still maturing
- it avoids pretending that update policy is solved when signing and trust requirements differ by host
- it works well for internal and team-shared tools where deployment is already controlled

## Host-Assisted Update Pattern

If you want something better than "download from a release page" without building a full updater, use:

- an internal release page or artifact registry
- a "Check for updates" UI action that opens the release page
- release notes that map directly to `packaging.version`
- a short install instruction that matches the host bundle

Examples:

- Linux: replace the portable folder or rerun `install.sh`
- Windows: replace the portable directory or rerun `install.ps1`
- macOS: replace the installed `.app` bundle or rerun `install.sh`

## What RustFrame Does Help With

- packaging version metadata lives in `rustframe.json`
- the bundle layout is predictable per host
- `package --verify` gives you a repeatable pre-release gate

## What RustFrame Does Not Yet Do

- no runtime-owned background updater
- no signed update manifest format
- no delta patching
- no built-in channel switching

If your product needs those now, plan them as app- or release-pipeline-owned infrastructure.
