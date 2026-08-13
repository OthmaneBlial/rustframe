# Signing and Notarization

`rustframe package` creates native package formats through cargo-packager. Local builds require no credentials and are explicitly marked unsigned in their output and `rustframe-package-manifest.json` metadata.

Before signing, run:

```bash
rustframe doctor
rustframe package --verify
```

## Linux

RustFrame creates AppImage and Debian packages on Linux. Publish the generated SHA-256 checksum manifest with the artifacts. Organization release pipelines can additionally sign the packages or repository metadata with their normal Linux signing keys.

## Windows

RustFrame creates NSIS and MSI installers on Windows. Sign the application executable and final installers with an Authenticode certificate using the Windows SDK or the organization’s signing service. Verify the signatures after packaging and before publishing.

## macOS

RustFrame creates `.app` and `.dmg` bundles on macOS. A public release should sign the application with a Developer ID identity, notarize it with Apple, staple the result, and validate the final distributed artifact using `codesign`, `xcrun notarytool`, and `xcrun stapler`.

Signing credentials are intentionally external to `rustframe.json`. Configure them in the protected native release environment; never commit certificates, passwords, or notarization credentials. Built-in auto-update remains outside v1.
