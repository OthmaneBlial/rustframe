use std::{env, fs, path::PathBuf};

use super::CliResult;

#[derive(Debug)]
pub struct RunnerProject {
    pub manifest_path: PathBuf,
    pub target_dir: PathBuf,
}

pub fn runtime_dependency() -> CliResult<String> {
    if let Ok(path) = env::var("RUSTFRAME_RUNTIME_PATH") {
        let path = fs::canonicalize(&path)
            .map_err(|error| format!("RUSTFRAME_RUNTIME_PATH '{path}' is invalid: {error}"))?;
        return Ok(format!(
            "{{ package = \"rustframe-runtime\", path = {} }}",
            quoted_literal(&cargo_path(&path.to_string_lossy()))
        ));
    }

    Ok(format!(
        "{{ package = \"rustframe-runtime\", version = \"={}\" }}",
        env!("CARGO_PKG_VERSION")
    ))
}

fn cargo_path(value: &str) -> String {
    let normalized = if let Some(path) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{path}")
    } else {
        value.strip_prefix(r"\\?\").unwrap_or(value).to_string()
    };
    normalized.replace('\\', "/")
}

fn quoted_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::cargo_path;

    #[test]
    fn cargo_paths_remove_windows_extended_drive_prefixes() {
        assert_eq!(
            cargo_path(r"\\?\D:\a\rustframe\crates\rustframe"),
            "D:/a/rustframe/crates/rustframe"
        );
    }

    #[test]
    fn cargo_paths_preserve_windows_unc_roots() {
        assert_eq!(
            cargo_path(r"\\?\UNC\server\share\rustframe"),
            "//server/share/rustframe"
        );
    }
}
