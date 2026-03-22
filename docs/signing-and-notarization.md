# Signing And Notarization

RustFrame packages bundles, but it does not currently sign them for you.

The practical model today is:

1. package the app on a native host
2. verify the produced bundle
3. sign or notarize with host tooling
4. publish the signed artifact

## Before You Sign Anything

Run:

```bash
cargo run -p rustframe-cli -- doctor
cargo run -p rustframe-cli -- package hello-rustframe --verify
```

Do not sign first and debug packaging later.

## Linux

RustFrame currently emits:

- a bundle directory
- an AppDir-style portable layout
- a `.tar.gz` archive

Common Linux practice today is usually one of:

- publish the archive with checksums and detached signatures
- repackage into your distro's native format in a downstream release pipeline

RustFrame does not currently emit distro-native signed packages like `.deb` or `.rpm`.

Recommended minimum:

- generate SHA-256 checksums for the archive
- sign the checksum file or archive with your release key
- publish the signature beside the archive

## Windows

RustFrame currently emits:

- a portable app directory
- PowerShell install and uninstall scripts
- a `.zip` archive

Recommended flow:

1. package the app on Windows
2. sign the executable with your Authenticode certificate
3. if your release flow wraps the portable app in another installer, sign that installer too
4. archive and publish only after signing

Typical host tooling:

- `signtool.exe` from the Windows SDK

Sign the final executable that users run, not just the zip file around it.

## macOS

RustFrame currently emits:

- an `.app` bundle
- shell install and uninstall scripts
- a `.tar.gz` archive

Recommended flow:

1. package on macOS
2. codesign the `.app` bundle with your Developer ID identity
3. notarize the signed bundle with Apple
4. staple the notarization ticket
5. archive and publish the stapled bundle

Typical host tooling:

- `codesign`
- `xcrun notarytool`
- `xcrun stapler`

RustFrame does not currently automate those steps for you.

## Practical Rule

Treat signing and notarization as a release-layer responsibility outside the runtime until the project grows a more opinionated distribution pipeline.
