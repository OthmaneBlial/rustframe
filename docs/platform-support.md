# Platform Support

RustFrame v1 targets native development and packaging on Linux, Windows, and macOS. Packaging runs on the matching host; cross-host installers are not promised.

| Host | Runtime prerequisites | Package formats |
|---|---|---|
| Linux | GTK 3, WebKitGTK, Rust GNU toolchain | AppImage, Debian |
| Windows | MSVC Rust toolchain and Windows SDK | NSIS, MSI |
| macOS | Xcode command-line tools | `.app`, DMG |

`rustframe doctor` checks the current host. Native CI compiles, tests, creates a standalone project, and exercises a package smoke format on each host. A release is supported only after its installer has been installed, launched, upgraded, and uninstalled on the target OS; compilation alone is not release evidence.

Unsigned local builds are supported for development and are marked as unsigned in package metadata. Public distribution may require Windows code signing, Apple signing and notarization, or Linux repository signing. RustFrame supplies hooks and documentation but does not own credentials.

Automatic updating is outside v1. Use manual downloads, managed organization deployment, or an application-owned update system and state that choice clearly.
