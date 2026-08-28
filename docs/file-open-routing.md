# Single-instance file-open routing

RustFrame applications with an `app.id` keep one primary process on macOS, Windows, and Linux. Launching the same packaged application again focuses its primary window. Any document paths supplied by the operating system are forwarded to that process instead of opening a second database and window stack.

## Frontend contract

Read files that launched the app after the bridge becomes available:

```ts
import { getRustFrame, type FileDropEntry } from "rustframe-api";

const rustframe = getRustFrame();

async function importFiles(files: FileDropEntry[]): Promise<void> {
  for (const file of files) {
    const contents = await rustframe.fs.readText(file.uri);
    console.log(file.name, contents.length);
  }
}

void importFiles(rustframe.app.openedFiles());

const stopOpenFiles = rustframe.app.onOpenFiles(async ({ files }) => {
  await importFiles(files);
});

// Later: stopOpenFiles();
```

`openedFiles()` returns the launch-time files plus files routed by later invocations. `onOpenFiles` reports only events received after the current WebView initialized. Keep the startup read and subscription together so application boot remains deterministic.

Each entry contains an opaque `grant://` URI and file metadata, matching a drag/drop entry. The grant is read-only and non-persistent. Read it with `fs.readText`, `fs.readBinary`, or `fs.metadata`; do not retain it as long-lived workspace access. The primary window needs `fs:grants:read`, and the filesystem bridge must be enabled.

## Declare native file types

Manifest schema v1 is immutable, so file types live in the separately versioned `.rustframe/file-associations.json` contract:

```json
{
  "$schema": "https://othmaneblial.github.io/rustframe/schemas/file-associations/v1/file-associations.schema.json",
  "schemaVersion": 1,
  "associations": [
    {
      "extensions": ["md", "markdown"],
      "mimeType": "text/markdown",
      "description": "Markdown document",
      "name": "Markdown document",
      "role": "editor"
    }
  ]
}
```

Extensions are lowercase and omit the dot. One extension may appear only once across the file. `mimeType` feeds Linux desktop metadata, `description` feeds Windows Explorer, `name` labels the macOS document type, and `role` accepts `editor`, `viewer`, `shell`, `ql-generator`, or `none`.

`rustframe validate` rejects unknown fields, a wrong schema URL/version, duplicate or noncanonical extensions, malformed MIME types, and oversized labels. `rustframe package` translates the validated declarations into:

- macOS `CFBundleDocumentTypes` entries;
- Linux MIME metadata and an `Exec` document argument;
- Windows installer file-extension registration.

The normalized declarations are also written into `rustframe-package-manifest.json` and the generated release notes. This records what the packager was asked to emit without claiming that a particular machine selected the app as its user default.

## Native boundary

The runtime:

1. acquires an app-scoped OS lock;
2. binds an IPv4 loopback listener on an ephemeral port;
3. writes a versioned endpoint record containing a random 256-bit token in the app data directory;
4. accepts at most 64 canonical, existing files per request;
5. converts accepted paths to temporary read grants before JavaScript sees them;
6. focuses and restores the primary window even when no file was supplied.

On Unix, the endpoint record is written with mode `0600`. Messages are bounded to 64 KiB, authenticated with the endpoint token, accepted only from loopback, and acknowledged before the secondary process exits. Missing paths, directories, duplicates, malformed messages, stale endpoints, and invalid tokens are not delivered.

macOS may deliver Finder document opens through its application event instead of command-line arguments. RustFrame normalizes both sources through the same grant path. Linux and Windows package launchers pass associated documents as process arguments.

## Scope

Single-instance routing is deliberately tied to an app ID. A custom ejected runner that omits `.app_id(...)` opts out. RustFrame does not expose absolute OS paths to frontend code and does not turn a document open into a persistent filesystem permission.

See [Build and package a local tool](./build-in-20-minutes.md) for distribution behavior and platform-specific verification.
