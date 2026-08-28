# Release Verification

`rustframe release verify` verifies a downloaded artifact independently of a RustFrame project. It reads observed bytes and sibling release metadata; it does not accept a caller-supplied `signed=true` or `verified=true` assertion as proof.

```bash
rustframe release verify ./Research-Desk.dmg \
  --repository OthmaneBlial/rustframe \
  --require-provenance \
  --require-sbom
```

Add `--json` for the stable `rustframe.release-verification` report. Schema version 1 includes the artifact digest and size, product identity, metadata source, native signature checks, provenance result, and verified SPDX SBOM digest. `trusted` is false whenever an explicitly allowed local artifact is unsigned.

## Files beside the artifact

The default layout is:

```text
download/
├── Research-Desk.dmg
├── SHA256SUMS
├── rustframe-package-manifest.json
└── research-desk.spdx.json
```

For assembled end-user releases, `*-release-index.json` replaces the package manifest. Use `--manifest`, `--index`, `--checksums`, or `--sbom` only when the files are stored elsewhere. Inferred metadata must describe the artifact by its exact file name, SHA-256 digest, and byte count. Unsafe checksum paths, duplicate records, malformed digests, symbolic links inside directory artifacts, and inconsistent signature metadata are rejected.

## Native trust is host-bound

- macOS `.app` and `.dmg` artifacts are checked with strict `codesign` verification and stapled-ticket validation. Applications also receive a Gatekeeper assessment.
- Windows `.exe` and `.msi` artifacts require a valid Authenticode signature and an observed timestamp certificate.
- Linux artifacts report native signing as `not-applicable`; checksums, optional SPDX evidence, and GitHub provenance remain visible separately.
- A macOS or Windows artifact cannot be declared trusted from another operating system. A transport archive containing an application must be extracted before native verification.

GitHub provenance is checked from the downloaded bytes with:

```bash
gh attestation verify <artifact> --repo OWNER/REPO
```

Pass `--repository OWNER/REPO` to run the check and `--require-provenance` to make it a release gate. Without that option, the report says `not-requested`; it never infers verification from text in an index.

## Local unsigned packages

Local macOS and Windows builds are refused by default:

```bash
rustframe release verify './Research Desk.app'
```

For local checksum QA only, make the exception explicit:

```bash
rustframe release verify './Research Desk.app' --allow-unsigned-local --json
```

The report then says `signature.state: "unsigned-local"` and `trusted: false`. The flag is forbidden when `GITHUB_ACTIONS=true`, so it cannot weaken a hosted release gate.

## CI release gate

Run verification after the artifact has crossed the same upload/download boundary as the end user. Use a native runner for each signed platform and retain the JSON report as release evidence:

```bash
rustframe release verify "$download" \
  --repository OthmaneBlial/rustframe \
  --require-provenance \
  --require-sbom \
  --json > release-verification.json
```

The command exits non-zero before emitting a success report when integrity, required evidence, native trust, or identity consistency fails.
