use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::Value;
use tempfile::tempdir;

#[cfg(unix)]
use std::os::unix::fs::symlink;

static NEXT_APP_ID: AtomicUsize = AtomicUsize::new(0);

fn cli_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rustframe-cli"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

#[cfg(not(unix))]
fn copy_dir_recursive(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();

    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = entry.metadata().unwrap();

        if metadata.is_dir() {
            copy_dir_recursive(&source_path, &destination_path);
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).unwrap();
        }
    }
}

fn link_dir_or_copy(source: &Path, destination: &Path) {
    #[cfg(unix)]
    {
        symlink(source, destination).unwrap();
    }

    #[cfg(not(unix))]
    {
        copy_dir_recursive(source, destination);
    }
}

fn create_test_workspace() -> PathBuf {
    let temp = tempdir().unwrap();
    let root = temp.path().to_path_buf();
    std::mem::forget(temp);

    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = []\n# crates/rustframe\n",
    )
    .unwrap();
    fs::copy(repo_root().join("Cargo.lock"), root.join("Cargo.lock")).unwrap();
    fs::create_dir_all(root.join("crates")).unwrap();
    link_dir_or_copy(
        &repo_root().join("crates/rustframe"),
        &root.join("crates/rustframe"),
    );
    link_dir_or_copy(&repo_root().join("target"), &root.join("target"));

    root
}

fn next_app_name(prefix: &str) -> String {
    format!(
        "{prefix}-{}",
        NEXT_APP_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn app_title(name: &str) -> String {
    name.split('-')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut capitalized = String::new();
            capitalized.push(first.to_ascii_uppercase());
            capitalized.extend(chars);
            capitalized
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn run_command(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    if !output.status.success() {
        panic!(
            "command failed: {:?}\nstdout:\n{}\nstderr:\n{}",
            command,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    output
}

fn run_cli(workspace: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(cli_binary());
    command.current_dir(workspace).args(args);
    run_command(&mut command)
}

fn run_cli_smoke(
    workspace: &Path,
    args: &[&str],
    report_path: &Path,
    data_dir: &Path,
) -> Output {
    let mut command = Command::new(cli_binary());
    command
        .current_dir(workspace)
        .args(args)
        .env("RUSTFRAME_SMOKE_TEST", "1")
        .env("RUSTFRAME_SMOKE_OUTPUT", report_path)
        .env("RUSTFRAME_SMOKE_DATA_DIR", data_dir);
    run_command(&mut command)
}

fn read_report(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn new_creates_manifest_first_app_scaffold() {
    let workspace = create_test_workspace();
    let app_name = next_app_name("scaffold-smoke");

    run_cli(&workspace, &["new", &app_name]);

    let app_dir = workspace.join("apps").join(&app_name);
    assert!(app_dir.join("index.html").exists());
    assert!(app_dir.join("styles.css").exists());
    assert!(app_dir.join("app.js").exists());
    assert!(app_dir.join("rustframe.json").exists());
    assert!(app_dir.join("data/schema.json").exists());
    assert!(app_dir.join("data/seeds/001-welcome.json").exists());
    assert!(!app_dir.join("bridge.js").exists());

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(app_dir.join("rustframe.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["appId"], app_name);
    assert_eq!(manifest["window"]["title"], app_title(&app_name));
    assert_eq!(manifest["window"]["width"], 1280);
    assert_eq!(manifest["window"]["height"], 820);
}

#[test]
fn dev_and_export_support_runtime_smoke_checks() {
    let workspace = create_test_workspace();
    let app_name = next_app_name("runtime-smoke");
    let expected_title = app_title(&app_name);
    run_cli(&workspace, &["new", &app_name]);

    let smoke_dir = workspace.join("smoke");
    fs::create_dir_all(&smoke_dir).unwrap();
    let dev_report_path = smoke_dir.join("dev-report.json");
    let dev_data_dir = smoke_dir.join("dev-data");

    run_cli_smoke(
        &workspace,
        &["dev", &app_name, "http://127.0.0.1:43123"],
        &dev_report_path,
        &dev_data_dir,
    );

    let dev_report = read_report(&dev_report_path);
    assert_eq!(dev_report["appId"], app_name);
    assert_eq!(dev_report["launchMode"], "dev-server");
    assert_eq!(dev_report["activeDevUrl"], "http://127.0.0.1:43123");
    assert_eq!(dev_report["window"]["title"], expected_title);
    assert_eq!(dev_report["hasIndexHtml"], true);
    assert_eq!(dev_report["bridgeInjected"], true);
    assert_eq!(dev_report["database"]["schemaVersion"], 1);
    assert!(
        workspace
            .join("target/rustframe/apps")
            .join(&app_name)
            .join("runner/src/main.rs")
            .exists()
    );

    run_cli(&workspace, &["export", &app_name]);
    let exported_binary = workspace.join("apps").join(&app_name).join("dist").join(&app_name);
    assert!(exported_binary.exists());

    let export_report_path = smoke_dir.join("export-report.json");
    let export_data_dir = smoke_dir.join("export-data");
    let mut binary_command = Command::new(&exported_binary);
    binary_command
        .current_dir(&workspace)
        .env("RUSTFRAME_SMOKE_TEST", "1")
        .env("RUSTFRAME_SMOKE_OUTPUT", &export_report_path)
        .env("RUSTFRAME_SMOKE_DATA_DIR", &export_data_dir);
    run_command(&mut binary_command);

    let export_report = read_report(&export_report_path);
    assert_eq!(export_report["appId"], app_name);
    assert_eq!(export_report["launchMode"], "embedded");
    assert_eq!(export_report["activeDevUrl"], Value::Null);
    assert_eq!(export_report["window"]["title"], expected_title);
    assert_eq!(export_report["database"]["schemaVersion"], 1);
    assert!(export_report["database"]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "notes"));
}
