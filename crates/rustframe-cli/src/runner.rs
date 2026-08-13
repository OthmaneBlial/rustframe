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
            quoted_literal(&path.to_string_lossy().replace('\\', "/"))
        ));
    }

    Ok(format!(
        "{{ package = \"rustframe-runtime\", version = \"={}\" }}",
        env!("CARGO_PKG_VERSION")
    ))
}

fn quoted_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
