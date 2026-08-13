use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub type ProjectResult<T> = Result<T, String>;

pub fn resolve_project(explicit: Option<&Path>) -> ProjectResult<PathBuf> {
    let start = match explicit {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => env::current_dir()
            .map_err(|error| format!("failed to resolve current directory: {error}"))?
            .join(path),
        None => env::current_dir()
            .map_err(|error| format!("failed to resolve current directory: {error}"))?,
    };

    if explicit.is_some() {
        let candidate = if start.is_file() {
            start.parent().unwrap_or(&start).to_path_buf()
        } else {
            start
        };
        return require_manifest(candidate);
    }

    start
        .ancestors()
        .find(|candidate| candidate.join("rustframe.json").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            "could not find rustframe.json; run inside a RustFrame project or pass --project <path>"
                .to_string()
        })
}

fn require_manifest(path: PathBuf) -> ProjectResult<PathBuf> {
    let canonical = fs::canonicalize(&path)
        .map_err(|error| format!("failed to resolve project '{}': {error}", path.display()))?;
    if !canonical.join("rustframe.json").is_file() {
        return Err(format!(
            "'{}' is not a RustFrame project: rustframe.json is missing",
            canonical.display()
        ));
    }
    Ok(canonical)
}

pub fn project_name(project_dir: &Path) -> ProjectResult<String> {
    project_dir
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("project path '{}' has no valid name", project_dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn explicit_projects_require_a_manifest() {
        let temp = tempdir().unwrap();
        assert!(
            resolve_project(Some(temp.path()))
                .unwrap_err()
                .contains("rustframe.json")
        );
        fs::write(temp.path().join("rustframe.json"), "{}").unwrap();
        assert_eq!(
            resolve_project(Some(temp.path())).unwrap(),
            temp.path().canonicalize().unwrap()
        );
    }
}
