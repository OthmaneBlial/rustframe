use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::Serialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{Result, RuntimeError};

const DEFAULT_SHELL_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_SHELL_MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Default)]
pub struct FsCapability {
    roots: Vec<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEntry {
    pub path: String,
    pub absolute_path: String,
    pub name: String,
    pub parent: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub size: u64,
    pub extension: Option<String>,
    pub modified_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsBinaryContents {
    #[serde(flatten)]
    pub file: FsEntry,
    pub byte_length: usize,
    pub base64: String,
}

#[derive(Clone, Debug)]
struct ResolvedFsPath {
    root: PathBuf,
    absolute: PathBuf,
    relative: PathBuf,
}

impl FsCapability {
    pub fn new<I, P>(roots: I) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let mut resolved_roots = Vec::new();
        for root in roots {
            let path = root.into();
            let canonical = path.canonicalize().map_err(|error| {
                RuntimeError::InvalidConfiguration(format!(
                    "fs root '{}' is invalid: {error}",
                    path.display()
                ))
            })?;

            if !canonical.is_dir() {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "fs root '{}' is not a directory",
                    canonical.display()
                )));
            }

            resolved_roots.push(canonical);
        }

        Ok(Self {
            roots: resolved_roots,
        })
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn read_text<P>(&self, requested: P) -> Result<String>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        fs::read_to_string(resolved.absolute).map_err(Into::into)
    }

    pub fn read_binary<P>(&self, requested: P) -> Result<FsBinaryContents>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        let bytes = fs::read(&resolved.absolute)?;
        Ok(FsBinaryContents {
            file: self.entry_for_resolved(&resolved)?,
            byte_length: bytes.len(),
            base64: BASE64_STANDARD.encode(bytes),
        })
    }

    pub fn metadata<P>(&self, requested: P) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        self.entry_for_resolved(&resolved)
    }

    pub fn list_dir<P>(&self, requested: P) -> Result<Vec<FsEntry>>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        if !resolved.absolute.is_dir() {
            return Err(RuntimeError::InvalidParameter(format!(
                "'{}' is not a directory",
                display_requested_path(requested.as_ref())
            )));
        }

        let mut entries = fs::read_dir(&resolved.absolute)?
            .map(|entry| {
                let entry = entry?;
                let child_absolute = entry.path();
                let child_relative = child_absolute
                    .strip_prefix(&resolved.root)
                    .map(PathBuf::from)
                    .map_err(|_| {
                        RuntimeError::PermissionDenied(format!(
                            "path '{}' is outside the configured filesystem roots",
                            child_absolute.display()
                        ))
                    })?;

                self.entry_for_paths(&resolved.root, &child_absolute, &child_relative)
            })
            .collect::<Result<Vec<_>>>()?;

        entries.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                .then_with(|| left.path.cmp(&right.path))
        });

        Ok(entries)
    }

    pub fn write_text<P>(&self, requested: P, contents: &str) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_for_write(requested.as_ref())?;
        self.prepare_write_target(&resolved)?;
        fs::write(&resolved.absolute, contents)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn write_binary<P>(&self, requested: P, contents_base64: &str) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let bytes = BASE64_STANDARD.decode(contents_base64).map_err(|error| {
            RuntimeError::InvalidParameter(format!("binary payload is not valid base64: {error}"))
        })?;
        let resolved = self.resolve_for_write(requested.as_ref())?;
        self.prepare_write_target(&resolved)?;
        fs::write(&resolved.absolute, bytes)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn copy_from<P, Q>(&self, source: P, destination: Q) -> Result<FsEntry>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let source = source.as_ref();
        if !source.is_absolute() {
            return Err(RuntimeError::InvalidParameter(
                "copy source must be an absolute path".into(),
            ));
        }

        let source_metadata = fs::metadata(source).map_err(|error| {
            RuntimeError::InvalidParameter(format!(
                "unable to read copy source '{}': {error}",
                source.display()
            ))
        })?;

        if !source_metadata.is_file() {
            return Err(RuntimeError::InvalidParameter(format!(
                "copy source '{}' is not a file",
                source.display()
            )));
        }

        let resolved = self.resolve_for_write(destination.as_ref())?;
        self.prepare_write_target(&resolved)?;
        fs::copy(source, &resolved.absolute)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn open_path<P>(&self, requested: P) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        open_in_default_app(&resolved.absolute)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn reveal_path<P>(&self, requested: P) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        reveal_in_file_manager(&resolved.absolute)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn resolve(&self, requested: &Path) -> Result<PathBuf> {
        Ok(self.resolve_existing(requested)?.absolute)
    }

    fn resolve_existing(&self, requested: &Path) -> Result<ResolvedFsPath> {
        if self.roots.is_empty() {
            return Err(RuntimeError::PermissionDenied(
                "no filesystem roots have been allowed".into(),
            ));
        }

        if requested.as_os_str().is_empty() {
            return Err(RuntimeError::InvalidParameter(
                "path must not be empty".into(),
            ));
        }

        if requested.is_absolute() {
            let canonical = requested.canonicalize().map_err(|error| {
                RuntimeError::InvalidParameter(format!(
                    "unable to resolve '{}': {error}",
                    requested.display()
                ))
            })?;

            return self.ensure_allowed(canonical, requested);
        }

        for root in &self.roots {
            let joined = root.join(requested);
            let canonical = match joined.canonicalize() {
                Ok(path) => path,
                Err(_) => continue,
            };

            if canonical.starts_with(root) {
                return Ok(ResolvedFsPath {
                    root: root.clone(),
                    relative: canonical
                        .strip_prefix(root)
                        .map(PathBuf::from)
                        .unwrap_or_default(),
                    absolute: canonical,
                });
            }
        }

        Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            requested.display()
        )))
    }

    fn resolve_for_write(&self, requested: &Path) -> Result<ResolvedFsPath> {
        if self.roots.is_empty() {
            return Err(RuntimeError::PermissionDenied(
                "no filesystem roots have been allowed".into(),
            ));
        }

        if requested.as_os_str().is_empty() {
            return Err(RuntimeError::InvalidParameter(
                "path must not be empty".into(),
            ));
        }

        if requested.is_absolute() {
            for root in &self.roots {
                if let Ok(resolved) = resolve_candidate_for_write(root, requested) {
                    return Ok(resolved);
                }
            }

            return Err(RuntimeError::PermissionDenied(format!(
                "path '{}' is outside the configured filesystem roots",
                requested.display()
            )));
        }

        for root in &self.roots {
            let candidate = root.join(requested);
            if let Ok(resolved) = resolve_candidate_for_write(root, &candidate) {
                return Ok(resolved);
            }
        }

        Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            requested.display()
        )))
    }

    fn ensure_allowed(&self, canonical: PathBuf, requested: &Path) -> Result<ResolvedFsPath> {
        if let Some(root) = self
            .roots
            .iter()
            .find(|root| canonical.starts_with(root.as_path()))
        {
            return Ok(ResolvedFsPath {
                root: root.clone(),
                relative: canonical
                    .strip_prefix(root)
                    .map(PathBuf::from)
                    .unwrap_or_default(),
                absolute: canonical,
            });
        }

        Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            requested.display()
        )))
    }

    fn prepare_write_target(&self, resolved: &ResolvedFsPath) -> Result<()> {
        if let Some(parent) = resolved.absolute.parent() {
            fs::create_dir_all(parent)?;
        }

        if resolved.absolute.exists() && resolved.absolute.is_dir() {
            return Err(RuntimeError::InvalidParameter(format!(
                "'{}' is a directory",
                display_requested_path(&resolved.relative)
            )));
        }

        Ok(())
    }

    fn entry_for_resolved(&self, resolved: &ResolvedFsPath) -> Result<FsEntry> {
        self.entry_for_paths(&resolved.root, &resolved.absolute, &resolved.relative)
    }

    fn entry_for_paths(&self, root: &Path, absolute: &Path, relative: &Path) -> Result<FsEntry> {
        let metadata = fs::metadata(absolute)?;
        Ok(FsEntry {
            path: display_requested_path(relative),
            absolute_path: absolute.to_string_lossy().to_string(),
            name: absolute
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| root.to_string_lossy().to_string()),
            parent: relative
                .parent()
                .map(display_requested_path)
                .unwrap_or_else(|| ".".into()),
            is_dir: metadata.is_dir(),
            is_file: metadata.is_file(),
            size: metadata.len(),
            extension: absolute
                .extension()
                .map(|value| value.to_string_lossy().to_string()),
            modified_at: modified_at_string(&metadata).ok(),
        })
    }
}

fn resolve_candidate_for_write(root: &Path, candidate: &Path) -> Result<ResolvedFsPath> {
    let mut existing_ancestor = candidate.to_path_buf();
    let mut missing_segments = Vec::new();

    while !existing_ancestor.exists() {
        let segment = existing_ancestor.file_name().ok_or_else(|| {
            RuntimeError::InvalidParameter(format!(
                "unable to resolve '{}' for writing",
                candidate.display()
            ))
        })?;
        missing_segments.push(segment.to_os_string());
        existing_ancestor = existing_ancestor
            .parent()
            .ok_or_else(|| {
                RuntimeError::InvalidParameter(format!(
                    "unable to resolve '{}' for writing",
                    candidate.display()
                ))
            })?
            .to_path_buf();
    }

    let canonical_ancestor = existing_ancestor.canonicalize().map_err(|error| {
        RuntimeError::InvalidParameter(format!(
            "unable to resolve '{}': {error}",
            candidate.display()
        ))
    })?;

    if !canonical_ancestor.starts_with(root) {
        return Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            candidate.display()
        )));
    }

    let mut absolute = canonical_ancestor.clone();
    for segment in missing_segments.iter().rev() {
        absolute.push(segment);
    }

    let relative = absolute
        .strip_prefix(root)
        .map(PathBuf::from)
        .map_err(|_| {
            RuntimeError::PermissionDenied(format!(
                "path '{}' is outside the configured filesystem roots",
                candidate.display()
            ))
        })?;

    Ok(ResolvedFsPath {
        root: root.to_path_buf(),
        absolute,
        relative,
    })
}

fn display_requested_path(path: &Path) -> String {
    let rendered = path.to_string_lossy().replace('\\', "/");
    if rendered.is_empty() {
        ".".into()
    } else {
        rendered
    }
}

fn modified_at_string(metadata: &fs::Metadata) -> std::io::Result<String> {
    let modified_at = metadata.modified()?;
    let timestamp = OffsetDateTime::from(modified_at)
        .format(&Rfc3339)
        .map_err(std::io::Error::other)?;
    Ok(timestamp)
}

#[derive(Clone, Debug)]
pub struct ShellCommand {
    pub program: String,
    pub args: Vec<String>,
    allowed_extra_args: BTreeSet<String>,
    cwd: Option<PathBuf>,
    env: BTreeMap<String, String>,
    clear_env: bool,
    timeout: Duration,
    max_output_bytes: usize,
}

impl ShellCommand {
    pub fn new<I, S>(program: impl Into<String>, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            allowed_extra_args: BTreeSet::new(),
            cwd: None,
            env: BTreeMap::new(),
            clear_env: false,
            timeout: DEFAULT_SHELL_TIMEOUT,
            max_output_bytes: DEFAULT_SHELL_MAX_OUTPUT_BYTES,
        }
    }

    pub fn allow_extra_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_extra_args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn clear_env(mut self) -> Self {
        self.clear_env = true;
        self
    }

    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout = Duration::from_millis(timeout_ms);
        self
    }

    pub fn max_output_bytes(mut self, max_output_bytes: usize) -> Self {
        self.max_output_bytes = max_output_bytes;
        self
    }

    fn validate(&self, name: &str) -> Result<()> {
        if self.program.trim().is_empty() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "shell command '{name}' must declare a program"
            )));
        }

        if self.timeout.is_zero() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "shell command '{name}' timeout must be greater than zero"
            )));
        }

        if self.max_output_bytes == 0 {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "shell command '{name}' max output bytes must be greater than zero"
            )));
        }

        if let Some(cwd) = &self.cwd {
            if !cwd.exists() {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "shell command '{name}' cwd '{}' does not exist",
                    cwd.display()
                )));
            }

            if !cwd.is_dir() {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "shell command '{name}' cwd '{}' is not a directory",
                    cwd.display()
                )));
            }
        }

        for key in self.env.keys() {
            if key.is_empty() || key.contains('=') || key.contains('\0') {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "shell command '{name}' defines invalid env key '{key}'"
                )));
            }
        }

        if self.env.values().any(|value| value.contains('\0')) {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "shell command '{name}' defines env values containing NUL bytes"
            )));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct ShellCapability {
    commands: BTreeMap<String, ShellCommand>,
}

impl ShellCapability {
    pub fn new(commands: BTreeMap<String, ShellCommand>) -> Self {
        Self { commands }
    }

    pub fn try_new(commands: BTreeMap<String, ShellCommand>) -> Result<Self> {
        for (name, command) in &commands {
            command.validate(name)?;
        }

        Ok(Self { commands })
    }

    pub fn insert(&mut self, name: impl Into<String>, command: ShellCommand) {
        self.commands.insert(name.into(), command);
    }

    pub fn command_names(&self) -> Vec<&str> {
        self.commands.keys().map(String::as_str).collect()
    }

    pub fn exec(&self, name: &str, extra_args: &[String]) -> Result<ShellOutput> {
        let Some(command) = self.commands.get(name) else {
            return Err(RuntimeError::PermissionDenied(format!(
                "shell command '{name}' is not allowed"
            )));
        };

        command.validate(name)?;

        if !extra_args.is_empty() && command.allowed_extra_args.is_empty() {
            return Err(RuntimeError::PermissionDenied(format!(
                "shell command '{name}' does not allow frontend arguments"
            )));
        }

        for arg in extra_args {
            if !command.allowed_extra_args.contains(arg) {
                return Err(RuntimeError::PermissionDenied(format!(
                    "shell command '{name}' does not allow argument '{arg}'"
                )));
            }
        }

        let mut child = Command::new(&command.program);
        child
            .args(&command.args)
            .args(extra_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(cwd) = &command.cwd {
            child.current_dir(cwd);
        }

        if command.clear_env {
            child.env_clear();
        }

        if !command.env.is_empty() {
            child.envs(command.env.iter());
        }

        let mut child = child.spawn().map_err(|error| {
            audit_shell_event(
                name,
                command,
                extra_args,
                None,
                None,
                None,
                Some(format!("failed to spawn process: {error}")),
            );
            error
        })?;
        let stdout_reader = spawn_reader(child.stdout.take(), command.max_output_bytes);
        let stderr_reader = spawn_reader(child.stderr.take(), command.max_output_bytes);
        let status = wait_for_exit(&mut child, command.timeout)?;

        let status = match status {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait()?;
                let stdout = collect_reader(stdout_reader)?;
                let stderr = collect_reader(stderr_reader)?;
                audit_shell_event(
                    name,
                    command,
                    extra_args,
                    None,
                    Some(&stdout),
                    Some(&stderr),
                    Some(format!(
                        "shell command '{name}' timed out after {} ms",
                        command.timeout.as_millis()
                    )),
                );
                return Err(RuntimeError::TimedOut(format!(
                    "shell command '{name}' timed out after {} ms",
                    command.timeout.as_millis()
                )));
            }
        };

        let stdout = collect_reader(stdout_reader)?;
        let stderr = collect_reader(stderr_reader)?;
        let output = ShellOutput {
            stdout: String::from_utf8_lossy(&stdout.bytes).into_owned(),
            stderr: String::from_utf8_lossy(&stderr.bytes).into_owned(),
            exit_code: status.code().unwrap_or_default(),
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
            timeout_ms: command.timeout.as_millis() as u64,
            max_output_bytes: command.max_output_bytes,
        };
        audit_shell_event(
            name,
            command,
            extra_args,
            Some(output.exit_code),
            Some(&stdout),
            Some(&stderr),
            None,
        );

        Ok(output)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
}

struct CapturedStream {
    bytes: Vec<u8>,
    truncated: bool,
}

fn spawn_reader(
    stream: Option<impl Read + Send + 'static>,
    max_output_bytes: usize,
) -> Option<thread::JoinHandle<std::io::Result<CapturedStream>>> {
    stream.map(|mut stream| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 8192];
            let mut truncated = false;

            loop {
                let read = stream.read(&mut buffer)?;
                if read == 0 {
                    break;
                }

                let remaining = max_output_bytes.saturating_sub(bytes.len());
                let kept = remaining.min(read);

                if kept > 0 {
                    bytes.extend_from_slice(&buffer[..kept]);
                }

                if kept < read {
                    truncated = true;
                }
            }

            Ok(CapturedStream { bytes, truncated })
        })
    })
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }

        if Instant::now() >= deadline {
            return Ok(None);
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn collect_reader(
    reader: Option<thread::JoinHandle<std::io::Result<CapturedStream>>>,
) -> Result<CapturedStream> {
    match reader {
        Some(reader) => reader
            .join()
            .map_err(|_| RuntimeError::InvalidConfiguration("shell output reader panicked".into()))?
            .map_err(Into::into),
        None => Ok(CapturedStream {
            bytes: Vec::new(),
            truncated: false,
        }),
    }
}

fn open_in_default_app(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        run_desktop_command("open", [path.as_os_str()])
    }

    #[cfg(target_os = "windows")]
    {
        run_desktop_command(
            "cmd",
            [
                std::ffi::OsStr::new("/C"),
                std::ffi::OsStr::new("start"),
                std::ffi::OsStr::new(""),
                path.as_os_str(),
            ],
        )
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        run_desktop_command("xdg-open", [path.as_os_str()])
    }
}

fn reveal_in_file_manager(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        run_desktop_command("open", [std::ffi::OsStr::new("-R"), path.as_os_str()])
    }

    #[cfg(target_os = "windows")]
    {
        let select_arg = format!("/select,{}", path.display());
        run_desktop_command("explorer", [std::ffi::OsStr::new(select_arg.as_str())])
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let parent = path.parent().unwrap_or(path);
        run_desktop_command("xdg-open", [parent.as_os_str()])
    }
}

fn run_desktop_command<I, S>(program: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let status = process::Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(RuntimeError::InvalidParameter(format!(
            "desktop helper '{}' exited with status {}",
            program, status
        )))
    }
}

fn audit_shell_event(
    name: &str,
    command: &ShellCommand,
    extra_args: &[String],
    exit_code: Option<i32>,
    stdout: Option<&CapturedStream>,
    stderr: Option<&CapturedStream>,
    error: Option<String>,
) {
    let Some(path) = env::var_os("RUSTFRAME_AUDIT_LOG") else {
        return;
    };

    let timestamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let payload = serde_json::json!({
        "timestamp": timestamp,
        "name": name,
        "program": command.program,
        "args": command.args,
        "extraArgs": extra_args,
        "cwd": command.cwd.as_ref().map(|value| value.to_string_lossy().to_string()),
        "envKeys": command.env.keys().cloned().collect::<Vec<_>>(),
        "clearEnv": command.clear_env,
        "timeoutMs": command.timeout.as_millis() as u64,
        "maxOutputBytes": command.max_output_bytes,
        "exitCode": exit_code,
        "stdoutBytes": stdout.map(|value| value.bytes.len()).unwrap_or(0),
        "stderrBytes": stderr.map(|value| value.bytes.len()).unwrap_or(0),
        "stdoutTruncated": stdout.map(|value| value.truncated).unwrap_or(false),
        "stderrTruncated": stderr.map(|value| value.truncated).unwrap_or(false),
        "error": error,
    });

    if let Some(parent) = Path::new(&path).parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }

    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{payload}");
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use base64::Engine as _;
    use tempfile::tempdir;

    use super::{BASE64_STANDARD, FsCapability, ShellCapability, ShellCommand};

    #[test]
    fn reads_file_inside_allowed_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("note.txt");
        fs::write(&file, "console.log('hello');").unwrap();

        let capability = FsCapability::new([root]).unwrap();

        let content = capability.read_text("note.txt").unwrap();

        assert_eq!(content, "console.log('hello');");
    }

    #[test]
    fn rejects_parent_escape_outside_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("safe.txt"), "safe").unwrap();
        fs::write(temp.path().join("secret.txt"), "secret").unwrap();

        let capability = FsCapability::new([root]).unwrap();
        let error = capability.read_text("../secret.txt").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("outside the configured filesystem roots")
        );
    }

    #[test]
    fn lists_directory_entries_and_metadata() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join("notes/brief.md"), "# Brief").unwrap();
        fs::write(root.join("summary.txt"), "ready").unwrap();

        let capability = FsCapability::new([root]).unwrap();
        let entries = capability.list_dir(".").unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "notes");
        assert!(entries[0].is_dir);
        assert_eq!(entries[1].path, "summary.txt");
        assert!(entries[1].is_file);
        assert_eq!(entries[1].extension.as_deref(), Some("txt"));
    }

    #[test]
    fn writes_text_inside_allowed_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();

        let capability = FsCapability::new([root.clone()]).unwrap();
        let written = capability
            .write_text("imports/brief.md", "# Imported")
            .unwrap();

        assert_eq!(written.path, "imports/brief.md");
        assert_eq!(
            fs::read_to_string(root.join("imports/brief.md")).unwrap(),
            "# Imported"
        );
    }

    #[test]
    fn reads_and_writes_binary_payloads() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();

        let capability = FsCapability::new([root.clone()]).unwrap();
        capability
            .write_binary("assets/icon.bin", &BASE64_STANDARD.encode([0_u8, 1, 2, 3]))
            .unwrap();

        let binary = capability.read_binary("assets/icon.bin").unwrap();

        assert_eq!(binary.byte_length, 4);
        assert_eq!(binary.base64, BASE64_STANDARD.encode([0_u8, 1, 2, 3]));
        assert_eq!(
            fs::read(root.join("assets/icon.bin")).unwrap(),
            vec![0_u8, 1, 2, 3]
        );
    }

    #[test]
    fn copies_external_files_into_allowed_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();
        let external = temp.path().join("source.md");
        fs::write(&external, "# External").unwrap();

        let capability = FsCapability::new([root.clone()]).unwrap();
        let copied = capability
            .copy_from(&external, "imports/source.md")
            .unwrap();

        assert_eq!(copied.path, "imports/source.md");
        assert_eq!(
            fs::read_to_string(root.join("imports/source.md")).unwrap(),
            "# External"
        );
    }

    #[test]
    fn rejects_unknown_shell_command() {
        let capability = ShellCapability::new(Default::default());
        let error = capability.exec("missing", &[]).unwrap_err();

        assert!(error.to_string().contains("is not allowed"));
    }

    #[test]
    fn executes_allowlisted_shell_command() {
        let mut capability = ShellCapability::default();
        capability.insert("print", ShellCommand::new("printf", ["rustframe"]));

        let output = capability.exec("print", &[]).unwrap();

        assert_eq!(output.stdout, "rustframe");
        assert_eq!(output.exit_code, 0);
        assert!(!output.stdout_truncated);
        assert!(!output.stderr_truncated);
        assert_eq!(output.timeout_ms, 10_000);
        assert_eq!(output.max_output_bytes, 64 * 1024);
    }

    #[test]
    fn rejects_frontend_args_when_none_are_allowlisted() {
        let mut capability = ShellCapability::default();
        capability.insert("print", ShellCommand::new("printf", ["rustframe"]));

        let error = capability
            .exec("print", &[String::from("--json")])
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("does not allow frontend arguments")
        );
    }

    #[test]
    fn rejects_frontend_args_outside_allowlist() {
        let mut capability = ShellCapability::default();
        capability.insert(
            "print",
            ShellCommand::new("printf", ["rustframe"]).allow_extra_args(["--json"]),
        );

        let error = capability
            .exec("print", &[String::from("--yaml")])
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("does not allow argument '--yaml'")
        );
    }

    #[test]
    fn executes_allowlisted_frontend_args() {
        let mut capability = ShellCapability::default();
        capability.insert(
            "print",
            ShellCommand::new("printf", ["%s%s", "rustframe"]).allow_extra_args(["--json"]),
        );

        let output = capability.exec("print", &[String::from("--json")]).unwrap();

        assert_eq!(output.stdout, "rustframe--json");
    }

    #[test]
    fn rejects_invalid_shell_configuration() {
        let error = ShellCapability::try_new(BTreeMap::from([(
            "print".to_string(),
            ShellCommand::new("printf", ["rustframe"]).timeout_ms(0),
        )]))
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("timeout must be greater than zero")
        );
    }

    #[cfg(unix)]
    #[test]
    fn shell_commands_can_run_in_declared_cwd() {
        let temp = tempdir().unwrap();
        let nested = temp.path().join("nested");
        fs::create_dir_all(&nested).unwrap();

        let mut capability = ShellCapability::default();
        capability.insert(
            "pwd",
            ShellCommand::new("pwd", std::iter::empty::<&str>()).current_dir(&nested),
        );

        let output = capability.exec("pwd", &[]).unwrap();

        assert_eq!(output.stdout.trim(), nested.to_string_lossy());
    }

    #[cfg(unix)]
    #[test]
    fn shell_commands_apply_explicit_env_overrides() {
        let mut capability = ShellCapability::default();
        capability.insert(
            "printenv",
            ShellCommand::new("printenv", ["RUSTFRAME_TEST_ENV"]).env("RUSTFRAME_TEST_ENV", "ok"),
        );

        let output = capability.exec("printenv", &[]).unwrap();

        assert_eq!(output.stdout.trim(), "ok");
    }

    #[cfg(unix)]
    #[test]
    fn shell_commands_time_out() {
        let mut capability = ShellCapability::default();
        capability.insert("sleep", ShellCommand::new("sleep", ["1"]).timeout_ms(25));

        let error = capability.exec("sleep", &[]).unwrap_err();

        assert!(error.to_string().contains("timed out"));
    }

    #[test]
    fn shell_output_is_truncated_to_limit() {
        let mut capability = ShellCapability::default();
        capability.insert(
            "print",
            ShellCommand::new("printf", ["rustframe"]).max_output_bytes(4),
        );

        let output = capability.exec("print", &[]).unwrap();

        assert_eq!(output.stdout, "rust");
        assert!(output.stdout_truncated);
        assert!(!output.stderr_truncated);
        assert_eq!(output.max_output_bytes, 4);
    }
}
