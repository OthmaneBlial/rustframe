# GitHub Actions Samples

These workflow files are for app builders who want a starting point for CI and release automation in a RustFrame-based repository.

Included samples:

- `verify-packaging.yml`
  - runs `doctor`
  - packages the app
  - verifies the produced bundle on each supported host
- `release-bundles.yml`
  - packages the app on all supported hosts
  - uploads the produced bundle folders and archives as workflow artifacts
  - leaves placeholders for signing, notarization, and release publication

Replace `my-app` with your app directory name before using them.
