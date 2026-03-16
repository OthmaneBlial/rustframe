use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

use crate::{Result, RuntimeError};

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
        }
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

        let output = Command::new(&command.program)
            .args(&command.args)
            .args(extra_args)
            .output()?;

        Ok(ShellOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or_default(),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[cfg(test)]
mod tests {
    use std::fs;

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
    }
}
