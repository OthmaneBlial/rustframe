# Threat Model

RustFrame is designed for bundled local-first workflow tools. Its security boundary is native IPC, not the JavaScript wrapper.

## Protected assets

- The user's SQLite data, backups, and restored state.
- Files reachable through declared roots or user-selected grants.
- Commands and environment values exposed as bounded automation.
- Native windows and cross-window application state.

## Threats in scope

- A compromised renderer attempting undeclared IPC calls.
- XSS or malformed frontend inputs trying path traversal, symlink escape, oversized requests, unauthorized writes, or arbitrary command execution.
- A secondary window trying to use the main window's permissions.
- Revoked grants or closed windows retaining filesystem watchers.
- Partial database batches and invalid or interrupted restore inputs.
- Sensitive command arguments leaking into audit output.

## Trust presets

`local-first` is for bundled frontend assets controlled by the application. It still requires explicit window permissions. Generated projects ship a restrictive Content Security Policy and do not allow remote production navigation.

`networked` treats the renderer as a remote-content surface. Validation rejects configurations that combine it with local capabilities accidentally. Adding remote scripts, unsafe user HTML, or third-party widgets is a security-model change, not a routine frontend edit.

## Capability enforcement

Permissions are attached to an exact window ID or an explicit suffix pattern such as `reader-*`. Unknown permissions and undeclared windows are denied. Examples include:

```text
db:read
db:write
db:backup
db:restore
fs:grants:read
fs:grants:write
fs:grants:watch
fs:workspace:read
fs:workspace:write
fs:workspace:watch
shell:index-workspace
dialog:open
window:create
```

The native dispatcher checks permissions before execution. Hiding a method from JavaScript is defense in depth, not authorization.

## Filesystem boundary

The primary API exposes `grant://` and `root://` URIs instead of arbitrary absolute paths. Native resolution canonicalizes targets, rejects traversal and root escapes, checks grant access mode, and treats revocation as immediate. Recursive walks require an entry limit and can restrict extensions.

Persist a user-selected grant only after explicit consent. Drag-and-drop selections should become temporary grants until the application asks to retain them.

## Database boundary

Database table and field names come from the validated schema. Batches execute in one SQLite transaction and publish change events only after commit. Backup uses SQLite's online backup support. Restore validates app identity and schema compatibility, creates a safety backup, replaces state atomically, and notifies windows to reload.

## Shell boundary

RustFrame exposes named commands, never a general shell. Programs and fixed arguments are application-declared; frontend arguments are denied unless allowlisted. Timeouts, output limits, a controlled working directory, and environment clearing reduce impact. Structured audit records include argument counts, environment key names, exit state, and byte counts; argument values, environment values, working-directory paths, and output contents are redacted.

## Limits and owner responsibilities

RustFrame does not make untrusted remote HTML safe to combine with local capabilities. It does not provide an OS sandbox, malware scanning, encrypted database storage, secret management, automatic updates, or signing credentials. Application authors remain responsible for frontend dependencies, safe Markdown/HTML rendering, data retention, release signing, and least-privilege manifests.

Report vulnerabilities using the private process in [SECURITY.md](https://github.com/OthmaneBlial/rustframe/blob/main/SECURITY.md).
