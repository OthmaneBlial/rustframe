# Platform Support

This page defines what RustFrame currently promises and what it does not.

## Current Promises

RustFrame currently targets host-native packaging on:

- Linux hosts using the GTK 3 and WebKitGTK stack required by `wry`
- Windows hosts using the MSVC Rust toolchain
- macOS hosts using Xcode command line tools

The repo now treats support as a checked contract, not a vague intention:

- `rustframe-cli doctor` checks the local host prerequisites
- `rustframe-cli platform-check <app>` validates the support matrix row from the current host
- `rustframe-cli package <app> --verify` validates the produced bundle layout and metadata
- the repo CI verifies packaged bundles on Linux, Windows, and macOS hosts

## What "Supported" Means Here

Supported currently means:

- you can build and package on a matching native host
- the CLI exposes host checks and verification paths instead of hiding the constraints
- the repo exercises the packaging flow in CI on supported hosts

Supported does not mean:

- every native API you might want already exists
- cross-host packaging is guaranteed from a different operating system
- signing, notarization, and auto-update workflows are fully automated by the runtime today

## Promises We Are Making

- Linux packaging is a host-native AppDir-style bundle plus archive.
- Windows packaging is a host-native portable directory plus archive.
- macOS packaging is a host-native `.app` bundle plus archive.
- Declared relative filesystem roots are copied into the packaged output.
- Packaging metadata is explicit in `rustframe.json` and can be verified after build.

## Non-Promises Right Now

- no claim that Linux can fully validate Windows or macOS packaging
- no built-in auto-update service
- no built-in code-signing abstraction
- no guarantee that unsigned bundles satisfy enterprise deployment requirements
- no promise that every host-specific distribution convention is covered yet

## Recommended Reading

For shipping work, pair this page with:

- `docs/signing-and-notarization.md`
- `docs/update-strategy.md`
- `docs/release-checklist.md`
