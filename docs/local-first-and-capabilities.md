# Local-First and Capability Inspection

RustFrame can turn the effective machine-access policy into reviewable evidence. The commands in this guide read the same `rustframe.json` contract used by validation and runner generation; they do not infer permissions from marketing copy.

## Inspect local ownership

Build the frontend first so the inspector can examine the exact bundled asset directory:

```bash
npm run build
rustframe inspect --local-first
```

The report covers:

- bundled asset count, size, and remote URL references;
- the declared network model and Content Security Policy;
- database schema, migrations, backup, restore, and portable export formats;
- declared filesystem roots and persisted opaque grants;
- effective permissions for every window selector;
- bounded shell commands;
- native packaging, signing, update, and release-verification policy.

Packaging evidence also reports single-instance behavior, declared native file-association count, and associated extensions. These values come from the validated sidecar and do not claim that the app is already the user's OS default.

Use the stable JSON form in CI or attach it to an audit:

```bash
rustframe inspect --local-first --json \
  --output artifacts/local-first-report.json
```

The command exits non-zero when an error finding makes the project non-conformant. Informational findings—such as persisted grants requiring a visible revoke flow—remain in the report without pretending the capability is unsafe by itself.

Every native package also writes `rustframe-local-first-report.json`. Its `policyHash` is recorded in `rustframe-package-manifest.json`, so a reviewer can tie the package metadata to one normalized policy.

Native package CI installs each generated format and launches the installed release binary with an isolated data directory and no frontend server. `scripts/assert_offline_smoke.mjs` fails unless the release uses embedded assets, has no active development URL, exposes the native bridge, opens local SQLite, and resolves the `local-first` security model. The resulting `rustframe-offline-<format>-receipt.json` travels through the release download boundary and is linked from the end-user release index. This proves that the packaged runtime does not require a production server; it does not claim that the operating system blocked every network interface during the check.

## Explain effective capabilities

```bash
rustframe capabilities explain
rustframe capabilities explain --json
```

The normalized JSON is sorted and hashed. It includes the security model, CSP, persisted-grant policy, declared roots, per-window permissions, and complete bounded-shell-command definitions. Environment variable values are never copied into the policy; only declared key names are included.

## Review a policy change

`diff` accepts either two RustFrame manifests or normalized policy snapshots and does not require a project in the current directory:

```bash
rustframe capabilities diff old/rustframe.json new/rustframe.json
rustframe capabilities diff reviewed-policy.json rustframe.json --json
```

The report separates additions, removals, general changes, and privilege expansions. New permissions, filesystem roots, shell commands, persisted grants, a move away from `local-first`, an application-identity change, and any CSP change require explicit review.

## Deny unreviewed expansion in CI

Create a baseline only after reading the human explanation:

```bash
rustframe capabilities explain
rustframe capabilities check --write-baseline
```

The default baseline is `.rustframe/capabilities-baseline.json` inside the project. Commit it, then enforce it:

```bash
rustframe capabilities check --deny-expansion
```

Removals pass the expansion gate but still appear as policy changes. Additions fail until a maintainer reviews the new scope and deliberately replaces the baseline. This keeps privilege review separate from ordinary code review noise.

## Read the boundary correctly

The report describes declared and bundled behavior. It cannot prove that an operating-system certificate belongs to the expected publisher or that a public artifact has GitHub provenance; use `rustframe release verify <artifact> --json` on the downloaded transport for those checks.

Likewise, a remote URL in bundled text is evidence to review, not automatic proof of a network dependency. It may be a help link. RustFrame reports the reference instead of guessing silently.
