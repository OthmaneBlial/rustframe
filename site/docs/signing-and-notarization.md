# Signing and Notarization

`rustframe package` creates native package formats through cargo-packager. Local builds require no credentials and are explicitly marked unsigned in their output, release notes, and `rustframe-package-manifest.json` metadata. A signed build is not called verified until the final transported artifact has been downloaded and checked on its native host.

Before signing, run:

```bash
rustframe doctor
rustframe package --verify
```

## Linux

RustFrame creates AppImage and Debian packages on Linux. Publish the generated SHA-256 checksum manifest with the artifacts. Organization release pipelines can additionally sign the packages or repository metadata with their normal Linux signing keys.

## Windows

RustFrame creates NSIS and MSI installers on Windows. When `RUSTFRAME_WINDOWS_CERTIFICATE_THUMBPRINT` is present, the native packager signs the application executable and installer with SHA-256. Set `RUSTFRAME_WINDOWS_TIMESTAMP_URL` to the certificate provider's RFC 3161 timestamp service. The trusted Research Desk workflow imports the PFX into the current-user certificate store, packages, then downloads the transported bundle onto a fresh Windows runner and requires `Get-AuthenticodeSignature` to report `Valid` with a timestamp certificate.

## macOS

RustFrame creates `.app` and `.dmg` bundles on macOS. When `RUSTFRAME_MACOS_SIGNING_IDENTITY` is present, the native packager signs the bundle. Standard `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` environment variables activate notarization; an accepted submission is stapled before packaging succeeds. The trusted Research Desk workflow then downloads the transported bundle onto a fresh macOS runner and requires `codesign --verify`, `xcrun stapler validate`, and Gatekeeper assessment to pass.

## Research Desk protected release

The `Research Desk trusted release` workflow runs only for an existing `research-desk-v*` tag or an explicitly confirmed manual dispatch. Its protected `release` environment requires:

- `APPLE_CERTIFICATE_BASE64`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` secrets;
- `WINDOWS_CERTIFICATE_BASE64` and `WINDOWS_CERTIFICATE_PASSWORD` secrets;
- a `WINDOWS_TIMESTAMP_URL` environment variable.

The workflow fails closed when any credential is absent. It produces one primary download per host, advanced formats, `SHA256SUMS`, SPDX SBOMs, GitHub attestations, exact tested-OS metadata, and per-format verification evidence. `scripts/verify_release_artifacts.mjs` verifies checksums and native signatures both before and after workflow-artifact transport. `scripts/assemble_research_desk_release.mjs` refuses publication unless every expected native proof is present.

For local checksum QA of an intentionally unsigned macOS or Windows build, pass `--allow-unsigned-local`; the resulting evidence is marked `unsigned-local` and this escape hatch is rejected inside GitHub Actions. It never qualifies an artifact for publication.

Signing credentials are intentionally external to `rustframe.json`. Configure them only in the protected native release environment; never commit certificates, passwords, notarization credentials, or exported certificate stores. Built-in auto-update remains outside v1. Ordinary users should never be told to bypass Gatekeeper or Windows security warnings for a stable flagship build.
