use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{Result, RuntimeError};

const DEFAULT_SHELL_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_SHELL_MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Default)]
pub struct FsCapability {
    roots: Vec<PathBuf>,
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
        let path = self.resolve(requested.as_ref())?;
        fs::read_to_string(path).map_err(Into::into)
    }

    pub fn resolve(&self, requested: &Path) -> Result<PathBuf> {
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
                return Ok(canonical);
            }
        }

        Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            requested.display()
        )))
    }

    fn ensure_allowed(&self, canonical: PathBuf, requested: &Path) -> Result<PathBuf> {
        if self.roots.iter().any(|root| canonical.starts_with(root)) {
            return Ok(canonical);
        }

        Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            requested.display()
        )))
    }
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

        let mut child = child.spawn()?;
        let stdout_reader = spawn_reader(child.stdout.take(), command.max_output_bytes);
        let stderr_reader = spawn_reader(child.stderr.take(), command.max_output_bytes);
        let status = wait_for_exit(&mut child, command.timeout)?;

        let status = match status {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait()?;
                collect_reader(stdout_reader)?;
                collect_reader(stderr_reader)?;
                return Err(RuntimeError::TimedOut(format!(
                    "shell command '{name}' timed out after {} ms",
                    command.timeout.as_millis()
                )));
            }
        };

        let stdout = collect_reader(stdout_reader)?;
        let stderr = collect_reader(stderr_reader)?;

        Ok(ShellOutput {
            stdout: String::from_utf8_lossy(&stdout.bytes).into_owned(),
            stderr: String::from_utf8_lossy(&stderr.bytes).into_owned(),
            exit_code: status.code().unwrap_or_default(),
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
        })
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

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use tempfile::tempdir;

    use super::{FsCapability, ShellCapability, ShellCommand};

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
    }
}
