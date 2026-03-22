use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use flate2::{Compression, write::GzEncoder};
use rustframe::{DatabaseMigrationFile, DatabaseSchema, DatabaseSeedFile};
use serde::Deserialize;
use serde_json::json;
use tar::Builder as TarBuilder;
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

type CliResult<T> = Result<T, String>;

const TEMPLATE_RUNNER_CARGO_TOML: &str =
    include_str!("../templates/generated-runner/Cargo.toml.tmpl");
const TEMPLATE_RUNNER_MAIN_RS: &str = include_str!("../templates/generated-runner/main.rs.tmpl");
const TEMPLATE_EJECTED_RUNNER_CARGO_TOML: &str =
    include_str!("../templates/ejected-runner/Cargo.toml.tmpl");
const TEMPLATE_EJECTED_RUNNER_MAIN_RS: &str =
    include_str!("../templates/ejected-runner/main.rs.tmpl");
const TEMPLATE_DATA_SCHEMA: &str = include_str!("../templates/data/schema.json");
const TEMPLATE_DATA_SEED: &str = include_str!("../templates/data/seeds/001-welcome.json");
const TEMPLATE_INDEX_HTML: &str = include_str!("../templates/frontend/index.html");
const TEMPLATE_STYLES_CSS: &str = include_str!("../templates/frontend/styles.css");
const TEMPLATE_APP_JS: &str = include_str!("../templates/frontend/app.js");
const TEMPLATE_MANIFEST_JSON: &str = include_str!("../templates/frontend/rustframe.json");
const TEMPLATE_APP_ICON_SVG: &str = include_str!("../templates/frontend/assets/icon.svg");
const DATABASE_FILE_NAME: &str = "app.db";

#[derive(Debug)]
struct AppProject {
    name: String,
    app_dir: PathBuf,
    asset_dir: PathBuf,
    config: AppConfig,
}

#[derive(Debug)]
struct AppConfig {
    app_id: String,
    title: String,
    width: f64,
    height: f64,
    dev_url: Option<String>,
    security: AppSecurityConfig,
    fs_roots: Vec<String>,
    shell_commands: Vec<AppShellCommand>,
    packaging: AppPackagingConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppSecurityModel {
    LocalFirst,
    Networked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppSecurityConfig {
    model: AppSecurityModel,
    database: bool,
    filesystem: bool,
    shell: bool,
}

impl AppSecurityConfig {
    fn local_first() -> Self {
        Self {
            model: AppSecurityModel::LocalFirst,
            database: true,
            filesystem: true,
            shell: true,
        }
    }

    fn networked() -> Self {
        Self {
            model: AppSecurityModel::Networked,
            database: false,
            filesystem: false,
            shell: false,
        }
    }
}

#[derive(Debug, Default)]
struct HtmlConfigFallback {
    title: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
    dev_url: Option<String>,
}

#[derive(Debug)]
struct RunnerProject {
    manifest_path: PathBuf,
    target_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct PlatformCheckRequest {
    name: String,
    targets: Vec<String>,
    uses_default_matrix: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlatformTargetSpec {
    label: &'static str,
    triple: &'static str,
    status: &'static str,
}

const DEFAULT_PLATFORM_TARGETS: [PlatformTargetSpec; 4] = [
    PlatformTargetSpec {
        label: "Linux",
        triple: "x86_64-unknown-linux-gnu",
        status: "dev/export/package on Linux hosts",
    },
    PlatformTargetSpec {
        label: "Windows",
        triple: "x86_64-pc-windows-msvc",
        status: "dev/export/package on Windows hosts",
    },
    PlatformTargetSpec {
        label: "macOS (Intel)",
        triple: "x86_64-apple-darwin",
        status: "dev/export/package on macOS hosts",
    },
    PlatformTargetSpec {
        label: "macOS (Apple Silicon)",
        triple: "aarch64-apple-darwin",
        status: "dev/export/package on macOS hosts",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlatformCheckOutcome {
    target: String,
    label: String,
    support_status: String,
    result: PlatformCheckResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PlatformCheckResult {
    Supported,
    NeedsNativeHost(String),
    MissingTarget,
    Failed(String),
}

#[derive(Debug)]
struct EmbeddedAsset {
    request_path: String,
    source_path: PathBuf,
}

#[derive(Debug, Clone)]
struct AppShellCommand {
    name: String,
    program: String,
    args: Vec<String>,
    allowed_args: Vec<String>,
    cwd: Option<String>,
    env: BTreeMap<String, String>,
    clear_env: bool,
    timeout_ms: Option<u64>,
    max_output_bytes: Option<usize>,
}

#[derive(Debug, Clone)]
struct AppPackagingConfig {
    version: String,
    description: String,
    publisher: Option<String>,
    homepage: Option<String>,
    linux: LinuxPackagingConfig,
    #[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
    windows: WindowsPackagingConfig,
    #[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
    macos: MacOsPackagingConfig,
}

#[derive(Debug, Clone)]
struct LinuxPackagingConfig {
    categories: Vec<String>,
    keywords: Vec<String>,
    icon_path: Option<PathBuf>,
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
#[derive(Debug, Clone)]
struct WindowsPackagingConfig {
    icon_path: Option<PathBuf>,
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
#[derive(Debug, Clone)]
struct MacOsPackagingConfig {
    bundle_identifier: String,
    icon_path: Option<PathBuf>,
}

#[derive(Debug)]
struct LinuxPackageOutput {
    bundle_dir: PathBuf,
    app_dir: PathBuf,
    archive_path: PathBuf,
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
#[derive(Debug)]
struct WindowsPackageOutput {
    bundle_dir: PathBuf,
    portable_dir: PathBuf,
    archive_path: PathBuf,
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
#[derive(Debug)]
struct MacOsPackageOutput {
    bundle_dir: PathBuf,
    app_bundle: PathBuf,
    archive_path: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AppManifest {
    #[serde(default)]
    app_id: Option<String>,
    #[serde(default)]
    dev_url: Option<String>,
    #[serde(default)]
    window: Option<ManifestWindow>,
    #[serde(default)]
    security: Option<ManifestSecurity>,
    #[serde(default)]
    filesystem: Option<ManifestFilesystem>,
    #[serde(default)]
    shell: Option<ManifestShell>,
    #[serde(default)]
    packaging: Option<ManifestPackaging>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestWindow {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestSecurity {
    #[serde(default)]
    model: Option<ManifestSecurityModel>,
    #[serde(default)]
    bridge: Option<ManifestSecurityBridge>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ManifestSecurityModel {
    LocalFirst,
    Networked,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestSecurityBridge {
    #[serde(default)]
    database: Option<bool>,
    #[serde(default)]
    filesystem: Option<bool>,
    #[serde(default)]
    shell: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestFilesystem {
    #[serde(default)]
    roots: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestShell {
    #[serde(default)]
    commands: Vec<ManifestShellCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestShellCommand {
    name: String,
    program: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    allowed_args: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    clear_env: bool,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    max_output_bytes: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestPackaging {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    linux: Option<ManifestLinuxPackaging>,
    #[serde(default)]
    windows: Option<ManifestWindowsPackaging>,
    #[serde(default)]
    macos: Option<ManifestMacOsPackaging>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestLinuxPackaging {
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestWindowsPackaging {
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestMacOsPackaging {
    #[serde(default)]
    bundle_identifier: Option<String>,
    #[serde(default)]
    icon: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("rustframe-cli: {error}");
        std::process::exit(1);
    }
}

fn run() -> CliResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("new") => {
            let name = args
                .get(1)
                .ok_or_else(|| "missing app name: rustframe-cli new <name>".to_string())?;
            command_new(name)
        }
        Some("dev") => {
            let workspace = find_workspace_root()?;
            let (name, dev_url) = parse_dev_args(&workspace, &args[1..])?;
            command_dev(&workspace, &name, dev_url)
        }
        Some("export") => {
            let workspace = find_workspace_root()?;
            let name = parse_export_args(&workspace, &args[1..])?;
            command_export(&workspace, &name)
        }
        Some("package") => {
            let workspace = find_workspace_root()?;
            let name = parse_package_args(&workspace, &args[1..])?;
            command_package(&workspace, &name)
        }
        Some("platform-check") => {
            let workspace = find_workspace_root()?;
            let request = parse_platform_check_args(&workspace, &args[1..])?;
            command_platform_check(&workspace, &request)
        }
        Some("inspect") => {
            let workspace = find_workspace_root()?;
            let name = parse_inspect_args(&workspace, &args[1..])?;
            command_inspect(&workspace, &name)
        }
        Some("reset-data") => {
            let workspace = find_workspace_root()?;
            let name = parse_reset_data_args(&workspace, &args[1..])?;
            command_reset_data(&workspace, &name)
        }
        Some("eject") => {
            let workspace = find_workspace_root()?;
            let name = parse_eject_args(&workspace, &args[1..])?;
            command_eject(&workspace, &name)
        }
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("unknown command '{other}'")),
    }
}

fn command_new(name: &str) -> CliResult<()> {
    validate_app_name(name)?;

    let workspace = find_workspace_root()?;
    let app_dir = workspace.join("apps").join(name);
    if app_dir.exists() {
        return Err(format!(
            "app directory already exists: {}",
            app_dir.display()
        ));
    }

    let title = humanize_name(name);
    let replacements = vec![
        ("{{app_name}}", name.to_string()),
        ("{{app_title}}", title.clone()),
        ("{{app_description}}", title.clone()),
        ("{{app_icon_path}}", "assets/icon.svg".to_string()),
        ("{{window_width}}", "1280".to_string()),
        ("{{window_height}}", "820".to_string()),
    ];

    write_text_file(
        &app_dir.join("index.html"),
        &render_template(TEMPLATE_INDEX_HTML, &replacements),
    )?;
    write_text_file(&app_dir.join("styles.css"), TEMPLATE_STYLES_CSS)?;
    write_text_file(
        &app_dir.join("app.js"),
        &render_template(
            TEMPLATE_APP_JS,
            &[
                ("{{app_title}}", title.clone()),
                ("{{app_name}}", name.to_string()),
            ],
        ),
    )?;
    write_text_file(
        &app_dir.join("assets/icon.svg"),
        &render_template(
            TEMPLATE_APP_ICON_SVG,
            &[
                ("{{app_title}}", title.clone()),
                ("{{app_monogram}}", icon_monogram(&title)),
            ],
        ),
    )?;
    write_text_file(
        &app_dir.join("rustframe.json"),
        &render_template(
            TEMPLATE_MANIFEST_JSON,
            &[
                ("{{app_name}}", name.to_string()),
                ("{{app_title}}", title.clone()),
                ("{{app_description}}", title.clone()),
                ("{{app_icon_path}}", "assets/icon.svg".to_string()),
                ("{{window_width}}", "1280".to_string()),
                ("{{window_height}}", "820".to_string()),
            ],
        ),
    )?;
    write_text_file(&app_dir.join("data/schema.json"), TEMPLATE_DATA_SCHEMA)?;
    write_text_file(
        &app_dir.join("data/seeds/001-welcome.json"),
        &render_template(
            TEMPLATE_DATA_SEED,
            &[
                ("{{app_title}}", title.clone()),
                ("{{app_name}}", name.to_string()),
            ],
        ),
    )?;
    fs::create_dir_all(app_dir.join("dist")).map_err(|error| {
        format!(
            "failed to create dist directory '{}': {error}",
            app_dir.join("dist").display()
        )
    })?;

    println!("Created RustFrame app: {}", app_dir.display());
    println!("Edit these files directly:");
    println!("  {}/index.html", app_dir.display());
    println!("  {}/styles.css", app_dir.display());
    println!("  {}/app.js", app_dir.display());
    println!("  {}/rustframe.json", app_dir.display());
    println!("Run it with: cargo run -p rustframe-cli -- dev {name}");
    Ok(())
}

fn command_dev(workspace: &Path, name: &str, dev_url: Option<String>) -> CliResult<()> {
    let app = load_app_project(workspace, name)?;
    let runner = resolve_runner_project(workspace, &app)?;

    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("--manifest-path")
        .arg(&runner.manifest_path)
        .arg("--bin")
        .arg(name)
        .current_dir(workspace)
        .env("CARGO_TARGET_DIR", &runner.target_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(url) = dev_url {
        command.env("RUSTFRAME_DEV_URL", url);
    }

    let status = command
        .status()
        .map_err(|error| format!("failed to launch cargo run: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo run failed with status {status}"))
    }
}

fn command_export(workspace: &Path, name: &str) -> CliResult<()> {
    let app = load_app_project(workspace, name)?;
    let runner = resolve_runner_project(workspace, &app)?;
    let source = build_release_binary(workspace, name, &runner)?;
    let binary_name = executable_name(name);

    let dist_dir = app.app_dir.join("dist");
    fs::create_dir_all(&dist_dir).map_err(|error| {
        format!(
            "failed to create dist directory '{}': {error}",
            dist_dir.display()
        )
    })?;

    let destination = dist_dir.join(&binary_name);
    copy_with_permissions(&source, &destination)?;
    sync_declared_fs_roots(&app, &dist_dir)?;

    let size = fs::metadata(&destination)
        .map_err(|error| format!("failed to stat '{}': {error}", destination.display()))?
        .len();

    println!("Exported {}", destination.display());
    println!("Size: {}", format_size(size));
    Ok(())
}

fn command_package(workspace: &Path, name: &str) -> CliResult<()> {
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = workspace;
        let _ = name;
        return Err("`package` currently supports Linux, Windows, and macOS hosts only".into());
    }

    #[cfg(target_os = "linux")]
    {
        let app = load_app_project(workspace, name)?;
        let runner = resolve_runner_project(workspace, &app)?;
        let source_binary = build_release_binary(workspace, name, &runner)?;
        let output = build_linux_package(&app, &source_binary)?;

        println!("Packaged {}", app.name);
        println!("Bundle: {}", output.bundle_dir.display());
        println!("AppDir: {}", output.app_dir.display());
        println!("Archive: {}", output.archive_path.display());
        println!("Install: {}/install.sh", output.bundle_dir.display());
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let app = load_app_project(workspace, name)?;
        let runner = resolve_runner_project(workspace, &app)?;
        let source_binary = build_release_binary(workspace, name, &runner)?;
        let output = build_windows_package(&app, &source_binary)?;

        println!("Packaged {}", app.name);
        println!("Bundle: {}", output.bundle_dir.display());
        println!("Portable app: {}", output.portable_dir.display());
        println!("Archive: {}", output.archive_path.display());
        println!("Install: {}\\install.ps1", output.bundle_dir.display());
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let app = load_app_project(workspace, name)?;
        let runner = resolve_runner_project(workspace, &app)?;
        let source_binary = build_release_binary(workspace, name, &runner)?;
        let output = build_macos_package(&app, &source_binary)?;

        println!("Packaged {}", app.name);
        println!("Bundle: {}", output.bundle_dir.display());
        println!("App: {}", output.app_bundle.display());
        println!("Archive: {}", output.archive_path.display());
        println!("Install: {}/install.sh", output.bundle_dir.display());
        Ok(())
    }
}

fn command_platform_check(workspace: &Path, request: &PlatformCheckRequest) -> CliResult<()> {
    let app = load_app_project(workspace, &request.name)?;
    let runner = resolve_runner_project(workspace, &app)?;
    let sysroot = rust_sysroot()?;
    let outcomes = request
        .targets
        .iter()
        .map(|target| {
            check_platform_target(
                workspace,
                &app,
                &runner,
                &sysroot,
                target,
                request.uses_default_matrix,
            )
        })
        .collect::<CliResult<Vec<_>>>()?;

    println!("Platform support matrix for {}", app.name);
    println!();

    for outcome in &outcomes {
        match &outcome.result {
            PlatformCheckResult::Supported => println!(
                "[ok]      {} ({})  {}",
                outcome.label, outcome.target, outcome.support_status
            ),
            PlatformCheckResult::NeedsNativeHost(message) => println!(
                "[host]    {} ({})  {}",
                outcome.label, outcome.target, message
            ),
            PlatformCheckResult::MissingTarget => println!(
                "[missing] {} ({})  install with `rustup target add {}`",
                outcome.label, outcome.target, outcome.target
            ),
            PlatformCheckResult::Failed(summary) => {
                println!(
                    "[fail]    {} ({})  {}",
                    outcome.label, outcome.target, outcome.support_status
                );
                println!("{summary}");
            }
        }
    }

    println!();
    println!("Packaging:");
    println!(
        "  Linux: `rustframe-cli package` builds an AppDir bundle and tarball on Linux hosts."
    );
    println!(
        "  Windows: `rustframe-cli package` builds a portable bundle, scripts, and a .zip on Windows hosts."
    );
    println!(
        "  macOS: `rustframe-cli package` builds an .app bundle, scripts, and a tarball on macOS hosts."
    );

    let failures = outcomes
        .iter()
        .filter(|outcome| {
            !matches!(
                outcome.result,
                PlatformCheckResult::Supported | PlatformCheckResult::NeedsNativeHost(_)
            )
        })
        .count();
    if failures == 0 {
        Ok(())
    } else {
        Err(format!(
            "platform validation failed for {failures} target(s)"
        ))
    }
}

fn command_inspect(workspace: &Path, name: &str) -> CliResult<()> {
    let app = load_app_project(workspace, name)?;
    let inspection = build_app_inspection(&app)?;
    let rendered = serde_json::to_string_pretty(&inspection)
        .map_err(|error| format!("failed to render inspection output: {error}"))?;
    println!("{rendered}");
    Ok(())
}

fn command_reset_data(workspace: &Path, name: &str) -> CliResult<()> {
    let app = load_app_project(workspace, name)?;
    let data_dir = default_app_data_dir(&app.config.app_id)?;

    if data_dir.exists() {
        fs::remove_dir_all(&data_dir).map_err(|error| {
            format!(
                "failed to remove app data directory '{}': {error}",
                data_dir.display()
            )
        })?;
        println!("Removed {}", data_dir.display());
    } else {
        println!("No app data directory exists at {}", data_dir.display());
    }

    println!(
        "Next `rustframe-cli dev {}` run will recreate the database, migrations, and seed data.",
        app.name
    );
    Ok(())
}

fn command_eject(workspace: &Path, name: &str) -> CliResult<()> {
    let app = load_app_project(workspace, name)?;
    let runner_dir = ejected_runner_dir(&app);
    if runner_dir.join("Cargo.toml").exists() {
        return Err(format!(
            "app '{}' is already ejected at {}",
            name,
            runner_dir.display()
        ));
    }

    let runner = prepare_ejected_runner(workspace, &app)?;

    println!("Ejected {}", app.name);
    println!("Native runner: {}", runner.manifest_path.display());
    println!("Customize it under: {}", runner_dir.display());
    println!("`dev` and `export` will now use the ejected runner automatically.");
    Ok(())
}

fn parse_dev_args(workspace: &Path, args: &[String]) -> CliResult<(String, Option<String>)> {
    match args {
        [] => Ok((resolve_current_app_name(workspace)?, None)),
        [only] if looks_like_url(only) => {
            Ok((resolve_current_app_name(workspace)?, Some(only.clone())))
        }
        [name] => Ok((name.clone(), None)),
        [name, dev_url, ..] => Ok((name.clone(), Some(dev_url.clone()))),
    }
}

fn parse_export_args(workspace: &Path, args: &[String]) -> CliResult<String> {
    match args {
        [] => resolve_current_app_name(workspace),
        [name, ..] => Ok(name.clone()),
    }
}

fn parse_package_args(workspace: &Path, args: &[String]) -> CliResult<String> {
    match args {
        [] => resolve_current_app_name(workspace),
        [name, ..] => Ok(name.clone()),
    }
}

fn parse_inspect_args(workspace: &Path, args: &[String]) -> CliResult<String> {
    match args {
        [] => resolve_current_app_name(workspace),
        [name, ..] => Ok(name.clone()),
    }
}

fn parse_reset_data_args(workspace: &Path, args: &[String]) -> CliResult<String> {
    match args {
        [] => resolve_current_app_name(workspace),
        [name, ..] => Ok(name.clone()),
    }
}

fn parse_eject_args(workspace: &Path, args: &[String]) -> CliResult<String> {
    match args {
        [] => resolve_current_app_name(workspace),
        [name, ..] => Ok(name.clone()),
    }
}

fn parse_platform_check_args(workspace: &Path, args: &[String]) -> CliResult<PlatformCheckRequest> {
    let mut name = None;
    let mut targets = Vec::new();
    let mut index = 0usize;

    while let Some(argument) = args.get(index) {
        if argument == "--target" {
            let value = args.get(index + 1).ok_or_else(|| {
                "missing target triple after --target: rustframe-cli platform-check [name] --target <triple>"
                    .to_string()
            })?;
            extend_targets(&mut targets, value)?;
            index += 2;
            continue;
        }

        if let Some(value) = argument.strip_prefix("--target=") {
            extend_targets(&mut targets, value)?;
            index += 1;
            continue;
        }

        if argument.starts_with("--") {
            return Err(format!("unknown platform-check flag '{argument}'"));
        }

        if name.is_some() {
            return Err(
                "platform-check accepts one app name and optional --target flags".to_string(),
            );
        }
        name = Some(argument.clone());
        index += 1;
    }

    let name = match name {
        Some(name) => name,
        None => resolve_current_app_name(workspace)?,
    };

    let uses_default_matrix = targets.is_empty();
    if targets.is_empty() {
        targets = DEFAULT_PLATFORM_TARGETS
            .iter()
            .map(|target| target.triple.to_string())
            .collect();
    } else {
        dedupe_preserving_order(&mut targets);
    }

    Ok(PlatformCheckRequest {
        name,
        targets,
        uses_default_matrix,
    })
}

fn extend_targets(targets: &mut Vec<String>, raw: &str) -> CliResult<()> {
    let mut parsed_any = false;

    for target in raw.split(',') {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            continue;
        }
        parsed_any = true;
        targets.push(trimmed.to_string());
    }

    if parsed_any {
        Ok(())
    } else {
        Err("platform-check target triples must not be empty".into())
    }
}

fn dedupe_preserving_order(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn resolve_current_app_name(workspace: &Path) -> CliResult<String> {
    let current_dir = env::current_dir()
        .and_then(fs::canonicalize)
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    resolve_current_app_name_from(workspace, &current_dir)
}

fn resolve_current_app_name_from(workspace: &Path, current_dir: &Path) -> CliResult<String> {
    let apps_dir = fs::canonicalize(workspace.join("apps"))
        .map_err(|error| format!("failed to resolve apps directory: {error}"))?;

    current_dir
        .ancestors()
        .find_map(|candidate| {
            (candidate.parent() == Some(apps_dir.as_path()))
                .then(|| {
                    candidate
                        .file_name()
                        .map(|value| value.to_string_lossy().to_string())
                })
                .flatten()
        })
        .ok_or_else(|| {
            "missing app name: run this command from apps/<name> or pass the app name explicitly"
                .to_string()
        })
}

fn find_workspace_root() -> CliResult<PathBuf> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    find_workspace_root_from(&current_dir)
}

fn find_workspace_root_from(current_dir: &Path) -> CliResult<PathBuf> {
    current_dir
        .ancestors()
        .find(|candidate| is_workspace_root(candidate))
        .map(Path::to_path_buf)
        .ok_or_else(|| "could not locate RustFrame workspace root".to_string())
}

fn is_workspace_root(path: &Path) -> bool {
    let manifest = path.join("Cargo.toml");
    let Ok(contents) = fs::read_to_string(manifest) else {
        return false;
    };

    contents.contains("[workspace]") && contents.contains("crates/rustframe")
}

fn load_app_project(workspace: &Path, name: &str) -> CliResult<AppProject> {
    validate_app_name(name)?;

    let app_dir = workspace.join("apps").join(name);
    if !app_dir.exists() {
        return Err(format!(
            "app '{name}' does not exist at {}",
            app_dir.display()
        ));
    }

    let asset_dir = if app_dir.join("index.html").exists() {
        app_dir.clone()
    } else if app_dir.join("frontend/index.html").exists() {
        app_dir.join("frontend")
    } else {
        return Err(format!(
            "app '{name}' is missing index.html at {}",
            app_dir.display()
        ));
    };

    let config = read_app_config(name, &app_dir, &asset_dir)?;

    Ok(AppProject {
        name: name.to_string(),
        app_dir,
        asset_dir,
        config,
    })
}

fn read_app_config(name: &str, app_dir: &Path, asset_dir: &Path) -> CliResult<AppConfig> {
    let index_path = asset_dir.join("index.html");
    let html = fs::read_to_string(&index_path)
        .map_err(|error| format!("failed to read '{}': {error}", index_path.display()))?;
    let manifest = read_app_manifest(app_dir)?;
    let html_fallback = read_html_config_fallback(&html)?;
    let window = manifest.window.unwrap_or_default();

    let title = normalize_optional_string("window.title", window.title)?
        .or(html_fallback.title)
        .unwrap_or_else(|| humanize_name(name));
    let width = if let Some(value) = window.width {
        validate_dimension("window.width", value)?
    } else {
        html_fallback.width.unwrap_or(1280.0)
    };
    let height = if let Some(value) = window.height {
        validate_dimension("window.height", value)?
    } else {
        html_fallback.height.unwrap_or(820.0)
    };
    let dev_url = manifest
        .dev_url
        .map(|value| validate_dev_url("devUrl", &value))
        .transpose()?
        .or(html_fallback.dev_url);
    let security = read_security_config(manifest.security);
    let app_id =
        normalize_optional_string("appId", manifest.app_id)?.unwrap_or_else(|| name.to_string());
    validate_app_id(&app_id)?;

    let fs_roots = manifest
        .filesystem
        .unwrap_or_default()
        .roots
        .into_iter()
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();
    validate_fs_roots(&fs_roots)?;
    let shell_commands = manifest
        .shell
        .unwrap_or_default()
        .commands
        .into_iter()
        .map(|command| AppShellCommand {
            name: command.name.trim().to_string(),
            program: command.program.trim().to_string(),
            args: command.args,
            allowed_args: command
                .allowed_args
                .into_iter()
                .map(|value| value.trim().to_string())
                .collect(),
            cwd: command.cwd.map(|value| value.trim().to_string()),
            env: command
                .env
                .into_iter()
                .map(|(key, value)| (key.trim().to_string(), value))
                .collect(),
            clear_env: command.clear_env,
            timeout_ms: command.timeout_ms,
            max_output_bytes: command.max_output_bytes,
        })
        .collect::<Vec<_>>();
    validate_shell_commands(&shell_commands)?;
    let packaging = read_packaging_config(app_dir, &app_id, &title, manifest.packaging)?;

    Ok(AppConfig {
        app_id,
        title,
        width,
        height,
        dev_url,
        security,
        fs_roots,
        shell_commands,
        packaging,
    })
}

fn read_html_config_fallback(html: &str) -> CliResult<HtmlConfigFallback> {
    let title = extract_title(html).map(|value| value.trim().to_string());
    let width = extract_meta_content(html, "rustframe:width")
        .map(|value| parse_dimension("rustframe:width", &value))
        .transpose()?;
    let height = extract_meta_content(html, "rustframe:height")
        .map(|value| parse_dimension("rustframe:height", &value))
        .transpose()?;
    let dev_url = extract_meta_content(html, "rustframe:dev-url")
        .map(|value| validate_dev_url("rustframe:dev-url", &value))
        .transpose()?;

    Ok(HtmlConfigFallback {
        title,
        width,
        height,
        dev_url,
    })
}

fn read_app_manifest(app_dir: &Path) -> CliResult<AppManifest> {
    let manifest_path = app_dir.join("rustframe.json");
    if !manifest_path.exists() {
        return Ok(AppManifest::default());
    }

    let source = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read '{}': {error}", manifest_path.display()))?;
    serde_json::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", manifest_path.display()))
}

fn read_security_config(manifest: Option<ManifestSecurity>) -> AppSecurityConfig {
    let manifest = manifest.unwrap_or_default();
    let mut security = match manifest.model.unwrap_or(ManifestSecurityModel::LocalFirst) {
        ManifestSecurityModel::LocalFirst => AppSecurityConfig::local_first(),
        ManifestSecurityModel::Networked => AppSecurityConfig::networked(),
    };

    if let Some(bridge) = manifest.bridge {
        if let Some(database) = bridge.database {
            security.database = database;
        }
        if let Some(filesystem) = bridge.filesystem {
            security.filesystem = filesystem;
        }
        if let Some(shell) = bridge.shell {
            security.shell = shell;
        }
    }

    security
}

fn resolve_runner_project(workspace: &Path, app: &AppProject) -> CliResult<RunnerProject> {
    if let Some(runner) = find_ejected_runner(workspace, app) {
        return Ok(runner);
    }

    prepare_generated_runner(workspace, app)
}

fn prepare_generated_runner(workspace: &Path, app: &AppProject) -> CliResult<RunnerProject> {
    let runner_dir = workspace
        .join("target")
        .join("rustframe")
        .join("apps")
        .join(&app.name)
        .join("runner");
    let manifest_path = runner_dir.join("Cargo.toml");
    let main_path = runner_dir.join("src/main.rs");
    let target_dir = workspace.join("target").join("rustframe");
    let assets = collect_embedded_assets(&app.asset_dir)?;
    let dev_url_chain = app
        .config
        .dev_url
        .as_ref()
        .map(|url| format!("\n        .dev_url({})", quoted_literal(url)))
        .unwrap_or_default();
    let app_id_chain = format!("\n        .app_id({})", quoted_literal(&app.config.app_id));
    let database_chain = render_database_chain(&assets);
    let security_chain = render_security_chain(&app.config.security);
    let fs_root_chain = render_fs_root_chain(&app.config.fs_roots);
    let shell_command_chain = render_shell_command_chain(&app.config.shell_commands);

    let manifest_contents = render_template(
        TEMPLATE_RUNNER_CARGO_TOML,
        &[
            (
                "{{runner_package_name}}",
                format!("rustframe-app-{}", app.name),
            ),
            ("{{binary_name}}", app.name.clone()),
            (
                "{{rustframe_path}}",
                quoted_literal(&slash_path(&workspace.join("crates").join("rustframe"))),
            ),
        ],
    );

    let main_contents = render_template(
        TEMPLATE_RUNNER_MAIN_RS,
        &[
            (
                "{{source_app_dir}}",
                quoted_literal(&slash_path(&app.app_dir)),
            ),
            (
                "{{source_asset_dir}}",
                quoted_literal(&slash_path(&app.asset_dir)),
            ),
            ("{{window_title}}", quoted_literal(&app.config.title)),
            ("{{window_width}}", format_float(app.config.width)),
            ("{{window_height}}", format_float(app.config.height)),
            ("{{app_id_chain}}", app_id_chain),
            ("{{dev_url_chain}}", dev_url_chain),
            ("{{database_chain}}", database_chain),
            ("{{security_chain}}", security_chain),
            ("{{fs_root_chain}}", fs_root_chain),
            ("{{shell_command_chain}}", shell_command_chain),
            ("{{asset_match_arms}}", render_asset_match_arms(&assets)),
        ],
    );

    write_text_file(&manifest_path, &manifest_contents)?;
    write_text_file(&main_path, &main_contents)?;

    Ok(RunnerProject {
        manifest_path,
        target_dir,
    })
}

fn prepare_ejected_runner(workspace: &Path, app: &AppProject) -> CliResult<RunnerProject> {
    let runner_dir = ejected_runner_dir(app);
    let manifest_path = runner_dir.join("Cargo.toml");
    let main_path = runner_dir.join("src/main.rs");
    let target_dir = workspace
        .join("target")
        .join("rustframe")
        .join("ejected")
        .join(&app.name);
    fs::create_dir_all(&runner_dir).map_err(|error| {
        format!(
            "failed to create ejected runner directory '{}': {error}",
            runner_dir.display()
        )
    })?;
    let assets = collect_embedded_assets(&app.asset_dir)?;
    let dev_url_chain = app
        .config
        .dev_url
        .as_ref()
        .map(|url| format!("\n        .dev_url({})", quoted_literal(url)))
        .unwrap_or_default();
    let app_id_chain = format!("\n        .app_id({})", quoted_literal(&app.config.app_id));
    let database_chain = render_database_chain(&assets);
    let security_chain = render_security_chain(&app.config.security);
    let fs_root_chain = render_fs_root_chain(&app.config.fs_roots);
    let shell_command_chain = render_shell_command_chain(&app.config.shell_commands);
    let rustframe_path = quoted_literal(&relative_path(
        &runner_dir,
        &workspace.join("crates").join("rustframe"),
    )?);
    let asset_folder = quoted_literal(&relative_path(&runner_dir, &app.asset_dir)?);
    let relative_app_dir = quoted_literal(&relative_path(&runner_dir, &app.app_dir)?);
    let relative_asset_dir = quoted_literal(&relative_path(&runner_dir, &app.asset_dir)?);

    let manifest_contents = render_template(
        TEMPLATE_EJECTED_RUNNER_CARGO_TOML,
        &[
            (
                "{{runner_package_name}}",
                format!("rustframe-app-{}", app.name),
            ),
            ("{{binary_name}}", app.name.clone()),
            ("{{rustframe_path}}", rustframe_path),
        ],
    );

    let main_contents = render_template(
        TEMPLATE_EJECTED_RUNNER_MAIN_RS,
        &[
            ("{{asset_folder}}", asset_folder),
            ("{{relative_app_dir}}", relative_app_dir),
            ("{{relative_asset_dir}}", relative_asset_dir),
            ("{{window_title}}", quoted_literal(&app.config.title)),
            ("{{window_width}}", format_float(app.config.width)),
            ("{{window_height}}", format_float(app.config.height)),
            ("{{app_id_chain}}", app_id_chain),
            ("{{dev_url_chain}}", dev_url_chain),
            ("{{database_chain}}", database_chain),
            ("{{security_chain}}", security_chain),
            ("{{fs_root_chain}}", fs_root_chain),
            ("{{shell_command_chain}}", shell_command_chain),
        ],
    );

    write_text_file(&manifest_path, &manifest_contents)?;
    write_text_file(&main_path, &main_contents)?;

    Ok(RunnerProject {
        manifest_path,
        target_dir,
    })
}

fn find_ejected_runner(workspace: &Path, app: &AppProject) -> Option<RunnerProject> {
    let manifest_path = ejected_runner_dir(app).join("Cargo.toml");
    manifest_path.exists().then(|| RunnerProject {
        manifest_path,
        target_dir: workspace
            .join("target")
            .join("rustframe")
            .join("ejected")
            .join(&app.name),
    })
}

fn ejected_runner_dir(app: &AppProject) -> PathBuf {
    app.app_dir.join("native")
}

fn collect_embedded_assets(asset_dir: &Path) -> CliResult<Vec<EmbeddedAsset>> {
    let mut assets = Vec::new();
    walk_assets(asset_dir, asset_dir, &mut assets)?;
    assets.sort_by(|left, right| left.request_path.cmp(&right.request_path));

    if !assets
        .iter()
        .any(|asset| asset.request_path == "index.html")
    {
        return Err(format!(
            "app assets at '{}' must contain index.html",
            asset_dir.display()
        ));
    }

    Ok(assets)
}

fn walk_assets(root: &Path, directory: &Path, assets: &mut Vec<EmbeddedAsset>) -> CliResult<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read '{}': {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read '{}': {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if file_name.starts_with('.') {
            continue;
        }

        if directory == root && file_name == "dist" && path.is_dir() {
            continue;
        }

        if path.is_dir() {
            walk_assets(root, &path, assets)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let request_path = slash_path(
            &path
                .strip_prefix(root)
                .map_err(|error| format!("failed to resolve '{}': {error}", path.display()))?,
        );

        assets.push(EmbeddedAsset {
            request_path,
            source_path: path,
        });
    }

    Ok(())
}

fn render_asset_match_arms(assets: &[EmbeddedAsset]) -> String {
    let mut arms = String::new();

    for asset in assets {
        let request_path = quoted_literal(&asset.request_path);
        let source_path = quoted_literal(&slash_path(&asset.source_path));
        arms.push_str(&format!(
            "            {request_path} => Some(Cow::Borrowed(include_bytes!({source_path}).as_slice())),\n"
        ));
    }

    arms.push_str("            _ => None,\n");
    arms
}

fn render_database_chain(assets: &[EmbeddedAsset]) -> String {
    let has_schema = assets
        .iter()
        .any(|asset| asset.request_path == "data/schema.json");
    if !has_schema {
        return String::new();
    }

    let seed_paths = assets
        .iter()
        .filter(|asset| asset.request_path.starts_with("data/seeds/"))
        .map(|asset| quoted_literal(&asset.request_path))
        .collect::<Vec<_>>();
    let migration_paths = assets
        .iter()
        .filter(|asset| asset.request_path.starts_with("data/migrations/"))
        .map(|asset| quoted_literal(&asset.request_path))
        .collect::<Vec<_>>();

    if migration_paths.is_empty() {
        return format!(
            "\n        .embedded_database({}, &[{}])",
            quoted_literal("data/schema.json"),
            seed_paths.join(", ")
        );
    }

    format!(
        "\n        .embedded_database_with_migrations({}, &[{}], &[{}])",
        quoted_literal("data/schema.json"),
        seed_paths.join(", "),
        migration_paths.join(", ")
    )
}

fn render_security_chain(security: &AppSecurityConfig) -> String {
    let defaults = match security.model {
        AppSecurityModel::LocalFirst => AppSecurityConfig::local_first(),
        AppSecurityModel::Networked => AppSecurityConfig::networked(),
    };

    let mut chain = match security.model {
        AppSecurityModel::LocalFirst => "rustframe::FrontendSecurity::local_first()".to_string(),
        AppSecurityModel::Networked => "rustframe::FrontendSecurity::networked()".to_string(),
    };

    if security.database != defaults.database {
        chain.push_str(&format!(".database({})", security.database));
    }

    if security.filesystem != defaults.filesystem {
        chain.push_str(&format!(".filesystem({})", security.filesystem));
    }

    if security.shell != defaults.shell {
        chain.push_str(&format!(".shell({})", security.shell));
    }

    format!("\n        .frontend_security({chain})")
}

fn render_fs_root_chain(roots: &[String]) -> String {
    roots
        .iter()
        .map(|root| {
            format!(
                "\n        .allow_fs_root(resolve_declared_fs_root({}))",
                quoted_literal(root)
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn render_shell_command_chain(commands: &[AppShellCommand]) -> String {
    commands
        .iter()
        .map(|command| {
            let args = if command.args.is_empty() {
                "Vec::<String>::new()".to_string()
            } else {
                format!(
                    "vec![{}]",
                    command
                        .args
                        .iter()
                        .map(|arg| {
                            format!("resolve_declared_shell_value({})", quoted_literal(arg))
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };

            let mut command_chain = format!(
                "rustframe::ShellCommand::new(resolve_declared_shell_value({}), {args})",
                quoted_literal(&command.program),
            );

            if !command.allowed_args.is_empty() {
                let allowed_args = command
                    .allowed_args
                    .iter()
                    .map(|arg| format!("resolve_declared_shell_value({})", quoted_literal(arg)))
                    .collect::<Vec<_>>()
                    .join(", ");
                command_chain.push_str(&format!(".allow_extra_args(vec![{allowed_args}])"));
            }

            if let Some(cwd) = &command.cwd {
                command_chain.push_str(&format!(
                    ".current_dir(resolve_declared_shell_dir({}))",
                    quoted_literal(cwd)
                ));
            }

            for (key, value) in &command.env {
                command_chain.push_str(&format!(
                    ".env({}, resolve_declared_shell_value({}))",
                    quoted_literal(key),
                    quoted_literal(value)
                ));
            }

            if command.clear_env {
                command_chain.push_str(".clear_env()");
            }

            if let Some(timeout_ms) = command.timeout_ms {
                command_chain.push_str(&format!(".timeout_ms({timeout_ms})"));
            }

            if let Some(max_output_bytes) = command.max_output_bytes {
                command_chain.push_str(&format!(".max_output_bytes({max_output_bytes})"));
            }

            format!(
                "\n        .allow_shell_command_configured({}, {command_chain})",
                quoted_literal(&command.name),
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    let title = html[start..end].trim();
    (!title.is_empty()).then(|| title.to_string())
}

fn extract_meta_content(html: &str, name: &str) -> Option<String> {
    let mut index = 0;

    while let Some(start_offset) = html[index..].find("<meta") {
        let start = index + start_offset;
        let end_offset = html[start..].find('>')?;
        let end = start + end_offset + 1;
        let tag = &html[start..end];

        if extract_attribute(tag, "name").as_deref() == Some(name) {
            return extract_attribute(tag, "content");
        }

        index = end;
    }

    None
}

fn extract_attribute(tag: &str, attribute: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let pattern = format!("{attribute}={quote}");
        if let Some(start) = tag.find(&pattern) {
            let value_start = start + pattern.len();
            let value_end = tag[value_start..].find(quote)? + value_start;
            return Some(tag[value_start..value_end].to_string());
        }
    }

    None
}

fn parse_dimension(field: &str, value: &str) -> CliResult<f64> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("{field} must be a number, received '{value}'"))?;

    validate_dimension(field, parsed)
}

fn validate_dimension(field: &str, value: f64) -> CliResult<f64> {
    if value > 0.0 {
        Ok(value)
    } else {
        Err(format!("{field} must be greater than zero"))
    }
}

fn normalize_optional_string(field: &str, value: Option<String>) -> CliResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(format!("{field} must not be empty"));
    }

    Ok(Some(normalized))
}

fn validate_dev_url(field: &str, value: &str) -> CliResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if !looks_like_url(normalized) {
        return Err(format!(
            "{field} must start with http:// or https://, received '{value}'"
        ));
    }

    Ok(normalized.to_string())
}

fn validate_app_id(value: &str) -> CliResult<()> {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return Err("appId must not be empty".into());
    };

    if !matches!(first, 'a'..='z' | 'A'..='Z' | '_') {
        return Err(format!(
            "appId '{}' must start with a letter or underscore",
            value
        ));
    }

    if !characters
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(format!(
            "appId '{}' may only contain letters, digits, underscores, and hyphens",
            value
        ));
    }

    Ok(())
}

fn validate_fs_roots(roots: &[String]) -> CliResult<()> {
    for root in roots {
        if root.trim().is_empty() {
            return Err("filesystem.roots entries must not be empty".into());
        }
    }

    Ok(())
}

fn validate_shell_commands(commands: &[AppShellCommand]) -> CliResult<()> {
    let mut seen = BTreeSet::new();

    for command in commands {
        if command.name.trim().is_empty() {
            return Err("shell.commands[].name must not be empty".into());
        }
        if !seen.insert(command.name.as_str()) {
            return Err(format!(
                "shell.commands defines '{}' more than once",
                command.name
            ));
        }
        if command.program.trim().is_empty() {
            return Err(format!(
                "shell.commands['{}'].program must not be empty",
                command.name
            ));
        }

        if command.args.iter().any(|value| value.trim().is_empty()) {
            return Err(format!(
                "shell.commands['{}'].args entries must not be empty",
                command.name
            ));
        }

        if command
            .allowed_args
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(format!(
                "shell.commands['{}'].allowedArgs entries must not be empty",
                command.name
            ));
        }

        if matches!(command.cwd.as_deref(), Some("")) {
            return Err(format!(
                "shell.commands['{}'].cwd must not be empty",
                command.name
            ));
        }

        if matches!(command.timeout_ms, Some(0)) {
            return Err(format!(
                "shell.commands['{}'].timeoutMs must be greater than zero",
                command.name
            ));
        }

        if matches!(command.max_output_bytes, Some(0)) {
            return Err(format!(
                "shell.commands['{}'].maxOutputBytes must be greater than zero",
                command.name
            ));
        }

        for key in command.env.keys() {
            if key.is_empty() || key.contains('=') || key.contains('\0') {
                return Err(format!(
                    "shell.commands['{}'].env defines invalid key '{}'",
                    command.name, key
                ));
            }
        }

        if command.env.values().any(|value| value.contains('\0')) {
            return Err(format!(
                "shell.commands['{}'].env values must not contain NUL bytes",
                command.name
            ));
        }
    }

    Ok(())
}

fn read_packaging_config(
    app_dir: &Path,
    app_id: &str,
    title: &str,
    manifest: Option<ManifestPackaging>,
) -> CliResult<AppPackagingConfig> {
    let manifest = manifest.unwrap_or_default();
    let linux = manifest.linux.unwrap_or_default();
    let windows = manifest.windows.unwrap_or_default();
    let macos = manifest.macos.unwrap_or_default();
    let version = manifest
        .version
        .unwrap_or_else(|| "0.1.0".to_string())
        .trim()
        .to_string();
    let description = manifest
        .description
        .unwrap_or_else(|| title.to_string())
        .trim()
        .to_string();
    let publisher = manifest
        .publisher
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let homepage = manifest
        .homepage
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let categories = if linux.categories.is_empty() {
        vec!["Utility".to_string()]
    } else {
        linux
            .categories
            .into_iter()
            .map(|value| value.trim().to_string())
            .collect::<Vec<_>>()
    };
    let keywords = linux
        .keywords
        .into_iter()
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();
    let icon_path = linux
        .icon
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| resolve_manifest_path(app_dir, &value));
    let windows_icon_path = windows
        .icon
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| resolve_manifest_path(app_dir, &value));
    let macos_icon_path = macos
        .icon
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| resolve_manifest_path(app_dir, &value));
    let macos_bundle_identifier = macos
        .bundle_identifier
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_macos_bundle_identifier(app_id));

    validate_packaging_metadata(&version, &description, &categories, &keywords)?;
    if let Some(path) = &icon_path {
        validate_packaging_icon(path, &["svg", "png"], "packaging.linux.icon")?;
    }
    if let Some(path) = &windows_icon_path {
        validate_packaging_icon(path, &["ico", "png", "svg"], "packaging.windows.icon")?;
    }
    if let Some(path) = &macos_icon_path {
        validate_packaging_icon(path, &["icns", "png", "svg"], "packaging.macos.icon")?;
    }
    validate_macos_bundle_identifier(&macos_bundle_identifier)?;

    Ok(AppPackagingConfig {
        version,
        description,
        publisher,
        homepage,
        linux: LinuxPackagingConfig {
            categories,
            keywords,
            icon_path,
        },
        windows: WindowsPackagingConfig {
            icon_path: windows_icon_path,
        },
        macos: MacOsPackagingConfig {
            bundle_identifier: macos_bundle_identifier,
            icon_path: macos_icon_path,
        },
    })
}

fn resolve_manifest_path(app_dir: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        app_dir.join(candidate)
    }
}

fn validate_packaging_metadata(
    version: &str,
    description: &str,
    categories: &[String],
    keywords: &[String],
) -> CliResult<()> {
    if version.is_empty() {
        return Err("packaging.version must not be empty".into());
    }
    if description.is_empty() {
        return Err("packaging.description must not be empty".into());
    }
    if categories.is_empty() {
        return Err("packaging.linux.categories must not be empty".into());
    }
    for category in categories {
        if category.is_empty() || category.contains(';') {
            return Err(format!(
                "packaging.linux.categories contains an invalid entry: '{}'",
                category
            ));
        }
    }
    for keyword in keywords {
        if keyword.is_empty() || keyword.contains(';') {
            return Err(format!(
                "packaging.linux.keywords contains an invalid entry: '{}'",
                keyword
            ));
        }
    }

    Ok(())
}

fn validate_packaging_icon(path: &Path, allowed_extensions: &[&str], field: &str) -> CliResult<()> {
    if !path.exists() {
        return Err(format!(
            "{field} points to a missing file: {}",
            path.display()
        ));
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| format!("{field} uses an unsupported file type: {}", path.display()))?;

    if !allowed_extensions.contains(&extension.as_str()) {
        return Err(format!(
            "{field} must end with one of [{}]: {}",
            allowed_extensions
                .iter()
                .map(|extension| format!(".{extension}"))
                .collect::<Vec<_>>()
                .join(", "),
            path.display()
        ));
    }

    Ok(())
}

fn default_macos_bundle_identifier(app_id: &str) -> String {
    format!("dev.rustframe.{}", app_id.replace('_', "-"))
}

fn validate_macos_bundle_identifier(value: &str) -> CliResult<()> {
    let segments = value.split('.').collect::<Vec<_>>();
    if segments.len() < 2 || segments.iter().any(|segment| segment.is_empty()) {
        return Err(format!(
            "packaging.macos.bundleIdentifier '{}' must contain at least two dot-separated segments",
            value
        ));
    }

    let valid = segments.iter().all(|segment| {
        segment
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    });

    if !valid {
        return Err(format!(
            "packaging.macos.bundleIdentifier '{}' may only contain letters, digits, hyphens, and dots",
            value
        ));
    }

    Ok(())
}

fn validate_app_name(name: &str) -> CliResult<()> {
    if name.is_empty() {
        return Err("app name must not be empty".into());
    }

    let valid = name.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    });

    if !valid
        || !name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
    {
        return Err(
            "app name must start with a lowercase letter and contain only lowercase letters, digits, and '-'"
                .into(),
        );
    }

    Ok(())
}

fn humanize_name(name: &str) -> String {
    name.split('-')
        .filter(|segment| !segment.is_empty())
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

fn icon_monogram(title: &str) -> String {
    let letters = title
        .split_whitespace()
        .filter_map(|segment| {
            segment
                .chars()
                .find(|character| character.is_ascii_alphanumeric())
        })
        .take(2)
        .map(|character| character.to_ascii_uppercase())
        .collect::<String>();

    if letters.is_empty() {
        "RF".to_string()
    } else {
        letters
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => {
            let mut result = first.to_ascii_uppercase().to_string();
            result.push_str(chars.as_str());
            result
        }
        None => String::new(),
    }
}

fn render_template(template: &str, replacements: &[(&str, String)]) -> String {
    let mut rendered = template.to_string();

    for (needle, value) in replacements {
        rendered = rendered.replace(needle, value);
    }

    rendered
}

fn write_text_file(path: &Path, contents: &str) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!("failed to create directory '{}': {error}", parent.display())
        })?;
    }

    fs::write(path, contents)
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))
}

fn relative_path(from_dir: &Path, to: &Path) -> CliResult<String> {
    let from = fs::canonicalize(from_dir)
        .map_err(|error| format!("failed to resolve '{}': {error}", from_dir.display()))?;
    let to = fs::canonicalize(to)
        .map_err(|error| format!("failed to resolve '{}': {error}", to.display()))?;

    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let mut common = 0usize;

    while common < from_components.len()
        && common < to_components.len()
        && from_components[common] == to_components[common]
    {
        common += 1;
    }

    if common == 0 {
        return Ok(slash_path(&to));
    }

    let mut relative = PathBuf::new();
    for component in &from_components[common..] {
        use std::path::Component;

        if !matches!(component, Component::CurDir) {
            relative.push("..");
        }
    }

    for component in &to_components[common..] {
        relative.push(component.as_os_str());
    }

    if relative.as_os_str().is_empty() {
        Ok(".".into())
    } else {
        Ok(slash_path(&relative))
    }
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn quoted_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn shell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn powershell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn executable_name(name: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("{name}.exe")
    }

    #[cfg(not(target_os = "windows"))]
    {
        name.to_string()
    }
}

fn package_executable_name(name: &str, target_os: &str) -> String {
    if target_os == "windows" {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn format_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn format_size(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    const KIB: f64 = 1024.0;

    if bytes >= 1024 * 1024 {
        format!("{:.2} MiB", bytes as f64 / MIB)
    } else if bytes >= 1024 {
        format!("{:.2} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn rust_sysroot() -> CliResult<PathBuf> {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .map_err(|error| format!("failed to launch rustc --print sysroot: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "failed to resolve Rust sysroot:\n{}",
            summarize_command_output(&output)
        ));
    }

    let sysroot = String::from_utf8(output.stdout)
        .map_err(|error| format!("rustc --print sysroot returned invalid UTF-8: {error}"))?;
    let sysroot = sysroot.trim();
    if sysroot.is_empty() {
        return Err("rustc --print sysroot returned an empty path".into());
    }

    Ok(PathBuf::from(sysroot))
}

fn check_platform_target(
    workspace: &Path,
    app: &AppProject,
    runner: &RunnerProject,
    sysroot: &Path,
    target: &str,
    use_default_matrix: bool,
) -> CliResult<PlatformCheckOutcome> {
    let spec = platform_target_spec(target);
    if use_default_matrix && !default_target_runs_on_current_host(target) {
        return Ok(PlatformCheckOutcome {
            target: target.to_string(),
            label: spec
                .map(|spec| spec.label.to_string())
                .unwrap_or_else(|| target.to_string()),
            support_status: spec
                .map(|spec| spec.status.to_string())
                .unwrap_or_else(|| "custom target".to_string()),
            result: PlatformCheckResult::NeedsNativeHost(native_host_message(target)),
        });
    }

    let rustlib_dir = sysroot.join("lib").join("rustlib").join(target);

    if !rustlib_dir.exists() {
        return Ok(PlatformCheckOutcome {
            target: target.to_string(),
            label: spec
                .map(|spec| spec.label.to_string())
                .unwrap_or_else(|| target.to_string()),
            support_status: spec
                .map(|spec| spec.status.to_string())
                .unwrap_or_else(|| "custom target".to_string()),
            result: PlatformCheckResult::MissingTarget,
        });
    }

    let output = Command::new("cargo")
        .arg("check")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&runner.manifest_path)
        .arg("--bin")
        .arg(&app.name)
        .arg("--target")
        .arg(target)
        .current_dir(workspace)
        .env("CARGO_TARGET_DIR", &runner.target_dir)
        .output()
        .map_err(|error| format!("failed to launch cargo check for target '{target}': {error}"))?;

    let result = if output.status.success() {
        PlatformCheckResult::Supported
    } else {
        PlatformCheckResult::Failed(indent_block(&summarize_command_output(&output), "  "))
    };

    Ok(PlatformCheckOutcome {
        target: target.to_string(),
        label: spec
            .map(|spec| spec.label.to_string())
            .unwrap_or_else(|| target.to_string()),
        support_status: spec
            .map(|spec| spec.status.to_string())
            .unwrap_or_else(|| "custom target".to_string()),
        result,
    })
}

fn default_target_runs_on_current_host(target: &str) -> bool {
    match env::consts::OS {
        "linux" => target.contains("unknown-linux"),
        "windows" => target.contains("pc-windows"),
        "macos" => target.contains("apple-darwin"),
        _ => false,
    }
}

fn native_host_message(target: &str) -> String {
    if target.contains("pc-windows") {
        "validate this row on Windows with MSVC build tools or a configured cross toolchain"
            .to_string()
    } else if target.contains("apple-darwin") {
        "validate this row on macOS with Xcode command line tools or a configured cross toolchain"
            .to_string()
    } else if target.contains("unknown-linux") {
        "validate this row on Linux with the native GTK/WebKitGTK stack".to_string()
    } else {
        "validate this row on a matching native host or with a configured cross toolchain"
            .to_string()
    }
}

fn platform_target_spec(target: &str) -> Option<&'static PlatformTargetSpec> {
    DEFAULT_PLATFORM_TARGETS
        .iter()
        .find(|spec| spec.triple == target)
}

fn summarize_command_output(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Some(summary) = tail_nonempty_lines(&stderr, 20) {
        return summary;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    tail_nonempty_lines(&stdout, 20).unwrap_or_else(|| {
        format!(
            "command exited with status {} and produced no output",
            output.status
        )
    })
}

fn tail_nonempty_lines(text: &str, max_lines: usize) -> Option<String> {
    let lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return None;
    }

    let start = lines.len().saturating_sub(max_lines);
    Some(lines[start..].join("\n"))
}

fn indent_block(value: &str, prefix: &str) -> String {
    value
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_release_binary(
    workspace: &Path,
    name: &str,
    runner: &RunnerProject,
) -> CliResult<PathBuf> {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&runner.manifest_path)
        .arg("--bin")
        .arg(name)
        .current_dir(workspace)
        .env("CARGO_TARGET_DIR", &runner.target_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("failed to launch cargo build: {error}"))?;

    if !status.success() {
        return Err(format!("cargo build --release failed with status {status}"));
    }

    let source = runner
        .target_dir
        .join("release")
        .join(executable_name(name));
    if !source.exists() {
        return Err(format!(
            "expected release binary was not produced: {}",
            source.display()
        ));
    }

    Ok(source)
}

fn copy_with_permissions(source: &Path, destination: &Path) -> CliResult<()> {
    fs::copy(source, destination).map_err(|error| {
        format!(
            "failed to copy '{}' to '{}': {error}",
            source.display(),
            destination.display()
        )
    })?;

    let permissions = fs::metadata(source)
        .map_err(|error| format!("failed to read '{}': {error}", source.display()))?
        .permissions();
    fs::set_permissions(destination, permissions).map_err(|error| {
        format!(
            "failed to preserve permissions for '{}': {error}",
            destination.display()
        )
    })
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> CliResult<()> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "failed to create directory '{}': {error}",
            destination.display()
        )
    })?;

    for entry in fs::read_dir(source)
        .map_err(|error| format!("failed to read directory '{}': {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read directory entry: {error}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = entry.metadata().map_err(|error| {
            format!(
                "failed to read metadata for '{}': {error}",
                source_path.display()
            )
        })?;

        if metadata.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            copy_with_permissions(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

fn packaged_fs_roots(app: &AppProject) -> Vec<(PathBuf, PathBuf)> {
    app.config
        .fs_roots
        .iter()
        .filter_map(|root| {
            let relative = root
                .strip_prefix("${EXE_DIR}/")
                .map(PathBuf::from)
                .or_else(|| Path::new(root).is_relative().then(|| PathBuf::from(root)))?;

            Some((app.app_dir.join(&relative), relative))
        })
        .collect()
}

fn sync_declared_fs_roots(app: &AppProject, executable_dir: &Path) -> CliResult<()> {
    for (source, relative) in packaged_fs_roots(app) {
        if !source.exists() {
            return Err(format!(
                "declared filesystem root '{}' does not exist at '{}'",
                relative.display(),
                source.display()
            ));
        }

        let destination = executable_dir.join(&relative);
        if destination.exists() {
            fs::remove_dir_all(&destination).map_err(|error| {
                format!(
                    "failed to replace bundled filesystem root '{}': {error}",
                    destination.display()
                )
            })?;
        }

        copy_dir_recursive(&source, &destination)?;
    }

    Ok(())
}

fn build_app_inspection(app: &AppProject) -> CliResult<serde_json::Value> {
    let data_dir = default_app_data_dir(&app.config.app_id)?;
    let database_path = data_dir.join(DATABASE_FILE_NAME);
    let packaged_roots = packaged_fs_roots(app)
        .into_iter()
        .map(|(source, relative)| {
            json!({
                "sourcePath": slash_path(&source),
                "bundledRelativePath": slash_path(&relative)
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "name": app.name,
        "appId": app.config.app_id,
        "title": app.config.title,
        "devUrl": app.config.dev_url,
        "paths": {
            "appDir": slash_path(&app.app_dir),
            "assetDir": slash_path(&app.asset_dir),
            "distDir": slash_path(&app.app_dir.join("dist")),
            "dataDir": slash_path(&data_dir),
            "databasePath": slash_path(&database_path),
            "databaseExists": database_path.exists()
        },
        "security": {
            "model": security_model_label(app.config.security.model),
            "database": app.config.security.database,
            "filesystem": app.config.security.filesystem,
            "shell": app.config.security.shell
        },
        "filesystem": {
            "declaredRoots": app.config.fs_roots,
            "packagedRoots": packaged_roots
        },
        "shell": {
            "commands": app.config.shell_commands.iter().map(|command| {
                json!({
                    "name": command.name,
                    "program": command.program,
                    "args": command.args,
                    "allowedArgs": command.allowed_args,
                    "cwd": command.cwd,
                    "env": command.env.keys().collect::<Vec<_>>(),
                    "clearEnv": command.clear_env,
                    "timeoutMs": command.timeout_ms.unwrap_or(10_000),
                    "maxOutputBytes": command.max_output_bytes.unwrap_or(64 * 1024)
                })
            }).collect::<Vec<_>>()
        },
        "database": inspect_database_assets(app, &data_dir)?,
        "warnings": capability_warnings(app)
    }))
}

fn inspect_database_assets(app: &AppProject, data_dir: &Path) -> CliResult<serde_json::Value> {
    let data_root = app.asset_dir.join("data");
    let schema_path = data_root.join("schema.json");
    let seed_dir = data_root.join("seeds");
    let migration_dir = data_root.join("migrations");

    if !schema_path.exists() {
        let has_seed_files = seed_dir.exists() && !list_child_files(&seed_dir)?.is_empty();
        let has_migrations =
            migration_dir.exists() && !list_child_files(&migration_dir)?.is_empty();
        if has_seed_files || has_migrations {
            return Ok(json!({
                "schemaPath": slash_path(&schema_path),
                "dataDir": slash_path(data_dir),
                "databasePath": slash_path(&data_dir.join(DATABASE_FILE_NAME)),
                "seedFiles": [],
                "migrationFiles": [],
                "diagnostics": {
                    "errors": ["data/schema.json is missing while seed or migration files are present"],
                    "warnings": []
                }
            }));
        }

        return Ok(serde_json::Value::Null);
    }

    let schema_source = fs::read_to_string(&schema_path)
        .map_err(|error| format!("failed to read '{}': {error}", schema_path.display()))?;
    let seed_paths = list_files_with_extension(&seed_dir, "json")?;
    let migration_paths = list_files_with_extension(&migration_dir, "sql")?;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let schema = match DatabaseSchema::from_json(&schema_source) {
        Ok(schema) => Some(schema),
        Err(error) => {
            errors.push(format!("schema.json is invalid: {error}"));
            None
        }
    };

    let seed_files = seed_paths
        .iter()
        .map(|path| {
            let source = fs::read_to_string(path)
                .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
            let relative = slash_path(path.strip_prefix(&app.asset_dir).unwrap_or(path));
            match DatabaseSeedFile::from_json(relative.clone(), &source) {
                Ok(seed) => Ok(json!({
                    "path": relative,
                    "entries": seed.entries.len(),
                    "checksum": seed.checksum
                })),
                Err(error) => {
                    errors.push(format!("seed '{}' is invalid: {error}", relative));
                    Ok(json!({ "path": relative }))
                }
            }
        })
        .collect::<CliResult<Vec<_>>>()?;

    let migration_files = migration_paths
        .iter()
        .map(|path| {
            let source = fs::read_to_string(path)
                .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
            let relative = slash_path(path.strip_prefix(&app.asset_dir).unwrap_or(path));
            match DatabaseMigrationFile::from_sql(relative.clone(), &source) {
                Ok(migration) => Ok(json!({
                    "path": relative,
                    "version": migration.version,
                    "checksum": migration.checksum
                })),
                Err(error) => {
                    errors.push(format!("migration '{}' is invalid: {error}", relative));
                    Ok(json!({ "path": relative }))
                }
            }
        })
        .collect::<CliResult<Vec<_>>>()?;

    let tables = schema
        .as_ref()
        .map(|schema| {
            schema
                .tables
                .iter()
                .map(|table| {
                    let searchable_columns = table
                        .columns
                        .iter()
                        .filter(|column| {
                            matches!(
                                column.kind,
                                rustframe::DatabaseColumnType::Text
                                    | rustframe::DatabaseColumnType::Json
                            )
                        })
                        .map(|column| column.name.clone())
                        .collect::<Vec<_>>();
                    if searchable_columns.is_empty() {
                        warnings.push(format!(
                            "table '{}' has no text/json columns for runtime full-text search",
                            table.name
                        ));
                    }

                    json!({
                        "name": table.name,
                        "columnCount": table.columns.len(),
                        "indexCount": table.indexes.len(),
                        "searchableColumns": searchable_columns
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(schema) = &schema {
        if schema.version > 1 && migration_files.is_empty() {
            warnings.push(format!(
                "schema version {} is greater than 1 but no SQL migrations were found",
                schema.version
            ));
        }
    }

    Ok(json!({
        "schemaPath": slash_path(&schema_path),
        "schemaVersion": schema.as_ref().map(|value| value.version),
        "tableCount": tables.len(),
        "tables": tables,
        "seedFiles": seed_files,
        "migrationFiles": migration_files,
        "dataDir": slash_path(data_dir),
        "databasePath": slash_path(&data_dir.join(DATABASE_FILE_NAME)),
        "databaseExists": data_dir.join(DATABASE_FILE_NAME).exists(),
        "diagnostics": {
            "errors": errors,
            "warnings": warnings
        }
    }))
}

fn capability_warnings(app: &AppProject) -> Vec<String> {
    let mut warnings = Vec::new();

    if app.config.security.model == AppSecurityModel::Networked {
        warnings.push(
            "frontend is declared as networked; local bridges should stay disabled unless the app has a clear trust boundary".into(),
        );
    }

    if app.config.security.filesystem && !app.config.fs_roots.is_empty() {
        warnings.push(format!(
            "filesystem bridge is enabled for {} scoped root(s); keep roots narrow and workflow-specific",
            app.config.fs_roots.len()
        ));
    }

    if app.config.security.shell && !app.config.shell_commands.is_empty() {
        warnings.push(format!(
            "shell bridge is enabled with {} allowlisted command(s); review timeouts, cwd, and max output limits before shipping",
            app.config.shell_commands.len()
        ));
    }

    warnings
}

fn security_model_label(model: AppSecurityModel) -> &'static str {
    match model {
        AppSecurityModel::LocalFirst => "local-first",
        AppSecurityModel::Networked => "networked",
    }
}

fn default_app_data_dir(app_id: &str) -> CliResult<PathBuf> {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .ok_or_else(|| "the current platform does not expose a user data directory".to_string())?;
    Ok(base.join(app_id))
}

fn list_child_files(directory: &Path) -> CliResult<Vec<PathBuf>> {
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut paths = fs::read_dir(directory)
        .map_err(|error| {
            format!(
                "failed to read directory '{}': {error}",
                directory.display()
            )
        })?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("failed to read directory entry: {error}"))
        })
        .collect::<CliResult<Vec<_>>>()?;
    paths.sort();
    Ok(paths)
}

fn list_files_with_extension(directory: &Path, extension: &str) -> CliResult<Vec<PathBuf>> {
    let mut paths = list_child_files(directory)?
        .into_iter()
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn write_binary_file(path: &Path, bytes: &[u8]) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!("failed to create directory '{}': {error}", parent.display())
        })?;
    }

    fs::write(path, bytes).map_err(|error| format!("failed to write '{}': {error}", path.display()))
}

fn build_linux_package(app: &AppProject, source_binary: &Path) -> CliResult<LinuxPackageOutput> {
    let dist_dir = app.app_dir.join("dist").join("linux");
    fs::create_dir_all(&dist_dir).map_err(|error| {
        format!(
            "failed to create Linux dist directory '{}': {error}",
            dist_dir.display()
        )
    })?;

    let bundle_name = format!(
        "{}-{}-linux-{}",
        app.config.app_id,
        app.config.packaging.version,
        env::consts::ARCH
    );
    let bundle_dir = dist_dir.join(&bundle_name);
    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir).map_err(|error| {
            format!(
                "failed to replace existing bundle '{}': {error}",
                bundle_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&bundle_dir).map_err(|error| {
        format!(
            "failed to create bundle directory '{}': {error}",
            bundle_dir.display()
        )
    })?;

    let app_dir = bundle_dir.join(format!("{}.AppDir", app.config.app_id));
    let usr_bin = app_dir.join("usr/bin");
    fs::create_dir_all(&usr_bin).map_err(|error| {
        format!(
            "failed to create bundle directory '{}': {error}",
            usr_bin.display()
        )
    })?;

    let binary_name = package_executable_name(&app.name, "linux");
    let installed_binary = usr_bin.join(&binary_name);
    copy_with_permissions(source_binary, &installed_binary)?;
    sync_declared_fs_roots(app, &usr_bin)?;

    let app_run_path = app_dir.join("AppRun");
    write_text_file(&app_run_path, &render_app_run_script(&binary_name))?;
    make_executable(&app_run_path)?;

    let icon = load_linux_icon(app)?;
    let desktop_entry_name = format!("{}.desktop", app.config.app_id);
    let icon_file_name = format!("{}.{}", app.config.app_id, icon.extension);
    let categories = format_desktop_categories(&app.config.packaging.linux.categories);
    let keywords = format_desktop_keywords(&app.config.packaging.linux.keywords);
    let desktop_entry = render_portable_desktop_entry(
        &app.config.title,
        &app.config.packaging.description,
        &app.config.app_id,
        &categories,
        keywords.as_deref(),
    );
    write_text_file(&app_dir.join(&desktop_entry_name), &desktop_entry)?;
    write_text_file(
        &app_dir
            .join("usr/share/applications")
            .join(&desktop_entry_name),
        &desktop_entry,
    )?;

    let icon_relative_path = match icon.extension.as_str() {
        "svg" => PathBuf::from("usr/share/icons/hicolor/scalable/apps").join(&icon_file_name),
        "png" => PathBuf::from("usr/share/icons/hicolor/256x256/apps").join(&icon_file_name),
        _ => unreachable!(),
    };
    write_binary_file(&app_dir.join(&icon_file_name), &icon.bytes)?;
    write_binary_file(&app_dir.join(&icon_relative_path), &icon.bytes)?;

    let metadata_path = bundle_dir.join("rustframe-package.json");
    let metadata = json!({
        "appId": app.config.app_id,
        "name": app.config.title,
        "version": app.config.packaging.version,
        "description": app.config.packaging.description,
        "publisher": app.config.packaging.publisher,
        "homepage": app.config.packaging.homepage,
        "target": {
            "os": "linux",
            "arch": env::consts::ARCH,
            "format": "appdir-tarball"
        },
        "artifacts": {
            "bundleDir": bundle_dir.file_name().map(|value| value.to_string_lossy().to_string()),
            "appDir": app_dir.file_name().map(|value| value.to_string_lossy().to_string()),
            "archive": format!("{bundle_name}.tar.gz")
        }
    });
    write_text_file(
        &metadata_path,
        &serde_json::to_string_pretty(&metadata)
            .map_err(|error| format!("failed to serialize package metadata: {error}"))?,
    )?;

    let install_script = bundle_dir.join("install.sh");
    write_text_file(
        &install_script,
        &render_linux_install_script(
            &app.config.title,
            &app.config.app_id,
            &app.config.packaging.description,
            &desktop_entry_name,
            &icon_relative_path,
            &icon_file_name,
            &categories,
            keywords.as_deref(),
        ),
    )?;
    make_executable(&install_script)?;

    let uninstall_script = bundle_dir.join("uninstall.sh");
    write_text_file(
        &uninstall_script,
        &render_linux_uninstall_script(&app.config.app_id),
    )?;
    make_executable(&uninstall_script)?;

    write_text_file(
        &bundle_dir.join("README.txt"),
        &render_linux_package_readme(app, &bundle_name, &app_dir),
    )?;

    let archive_path = dist_dir.join(format!("{bundle_name}.tar.gz"));
    if archive_path.exists() {
        fs::remove_file(&archive_path).map_err(|error| {
            format!(
                "failed to replace existing archive '{}': {error}",
                archive_path.display()
            )
        })?;
    }
    write_tarball(&bundle_dir, &archive_path)?;

    Ok(LinuxPackageOutput {
        bundle_dir,
        app_dir,
        archive_path,
    })
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn build_windows_package(
    app: &AppProject,
    source_binary: &Path,
) -> CliResult<WindowsPackageOutput> {
    let dist_dir = app.app_dir.join("dist").join("windows");
    fs::create_dir_all(&dist_dir).map_err(|error| {
        format!(
            "failed to create Windows dist directory '{}': {error}",
            dist_dir.display()
        )
    })?;

    let bundle_name = format!(
        "{}-{}-windows-{}",
        app.config.app_id,
        app.config.packaging.version,
        env::consts::ARCH
    );
    let bundle_dir = dist_dir.join(&bundle_name);
    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir).map_err(|error| {
            format!(
                "failed to replace existing bundle '{}': {error}",
                bundle_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&bundle_dir).map_err(|error| {
        format!(
            "failed to create bundle directory '{}': {error}",
            bundle_dir.display()
        )
    })?;

    let portable_dir = bundle_dir.join(&app.config.app_id);
    fs::create_dir_all(&portable_dir).map_err(|error| {
        format!(
            "failed to create bundle directory '{}': {error}",
            portable_dir.display()
        )
    })?;

    let binary_name = package_executable_name(&app.name, "windows");
    let installed_binary = portable_dir.join(&binary_name);
    copy_with_permissions(source_binary, &installed_binary)?;
    sync_declared_fs_roots(app, &portable_dir)?;

    let icon = load_packaging_icon(
        app,
        app.config.packaging.windows.icon_path.as_deref(),
        app.config.packaging.linux.icon_path.as_deref(),
    )?;
    let icon_file_name = icon
        .as_ref()
        .map(|icon| format!("{}.{}", app.config.app_id, icon.extension));
    if let (Some(icon), Some(icon_file_name)) = (&icon, &icon_file_name) {
        write_binary_file(&portable_dir.join(icon_file_name), &icon.bytes)?;
    }

    let metadata_path = bundle_dir.join("rustframe-package.json");
    let metadata = json!({
        "appId": app.config.app_id,
        "name": app.config.title,
        "version": app.config.packaging.version,
        "description": app.config.packaging.description,
        "publisher": app.config.packaging.publisher,
        "homepage": app.config.packaging.homepage,
        "target": {
            "os": "windows",
            "arch": env::consts::ARCH,
            "format": "portable-zip"
        },
        "artifacts": {
            "bundleDir": bundle_dir.file_name().map(|value| value.to_string_lossy().to_string()),
            "portableDir": portable_dir.file_name().map(|value| value.to_string_lossy().to_string()),
            "archive": format!("{bundle_name}.zip")
        }
    });
    write_text_file(
        &metadata_path,
        &serde_json::to_string_pretty(&metadata)
            .map_err(|error| format!("failed to serialize package metadata: {error}"))?,
    )?;

    let install_script = bundle_dir.join("install.ps1");
    write_text_file(
        &install_script,
        &render_windows_install_script(
            &app.config.title,
            &app.config.app_id,
            &app.config.packaging.description,
            portable_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(&app.config.app_id),
            &binary_name,
            icon_file_name.as_deref(),
        ),
    )?;

    let uninstall_script = bundle_dir.join("uninstall.ps1");
    write_text_file(
        &uninstall_script,
        &render_windows_uninstall_script(&app.config.title, &app.config.app_id),
    )?;

    write_text_file(
        &bundle_dir.join("README.txt"),
        &render_windows_package_readme(app, &bundle_name, &portable_dir),
    )?;

    let archive_path = dist_dir.join(format!("{bundle_name}.zip"));
    if archive_path.exists() {
        fs::remove_file(&archive_path).map_err(|error| {
            format!(
                "failed to replace existing archive '{}': {error}",
                archive_path.display()
            )
        })?;
    }
    write_zip(&bundle_dir, &archive_path)?;

    Ok(WindowsPackageOutput {
        bundle_dir,
        portable_dir,
        archive_path,
    })
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn build_macos_package(app: &AppProject, source_binary: &Path) -> CliResult<MacOsPackageOutput> {
    let dist_dir = app.app_dir.join("dist").join("macos");
    fs::create_dir_all(&dist_dir).map_err(|error| {
        format!(
            "failed to create macOS dist directory '{}': {error}",
            dist_dir.display()
        )
    })?;

    let bundle_name = format!(
        "{}-{}-macos-{}",
        app.config.app_id,
        app.config.packaging.version,
        env::consts::ARCH
    );
    let bundle_dir = dist_dir.join(&bundle_name);
    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir).map_err(|error| {
            format!(
                "failed to replace existing bundle '{}': {error}",
                bundle_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&bundle_dir).map_err(|error| {
        format!(
            "failed to create bundle directory '{}': {error}",
            bundle_dir.display()
        )
    })?;

    let app_bundle_name = format!("{}.app", sanitize_bundle_file_name(&app.config.title));
    let app_bundle = bundle_dir.join(&app_bundle_name);
    let contents_dir = app_bundle.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");
    fs::create_dir_all(&macos_dir).map_err(|error| {
        format!(
            "failed to create bundle directory '{}': {error}",
            macos_dir.display()
        )
    })?;
    fs::create_dir_all(&resources_dir).map_err(|error| {
        format!(
            "failed to create bundle directory '{}': {error}",
            resources_dir.display()
        )
    })?;

    let binary_name = package_executable_name(&app.name, "macos");
    let installed_binary = macos_dir.join(&binary_name);
    copy_with_permissions(source_binary, &installed_binary)?;
    sync_declared_fs_roots(app, &macos_dir)?;

    let icon = load_packaging_icon(
        app,
        app.config.packaging.macos.icon_path.as_deref(),
        app.config.packaging.linux.icon_path.as_deref(),
    )?;
    let mut icon_file_name = None;
    let mut plist_icon_name = None;
    if let Some(icon) = &icon {
        let file_name = format!("{}.{}", app.config.app_id, icon.extension);
        write_binary_file(&resources_dir.join(&file_name), &icon.bytes)?;
        if icon.extension == "icns" {
            plist_icon_name = Some(
                Path::new(&file_name)
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or(&app.config.app_id)
                    .to_string(),
            );
        }
        icon_file_name = Some(file_name);
    }

    write_text_file(
        &contents_dir.join("Info.plist"),
        &render_macos_info_plist(
            &app.config.title,
            &binary_name,
            &app.config.packaging.macos.bundle_identifier,
            &app.config.packaging.version,
            plist_icon_name.as_deref(),
        ),
    )?;

    let metadata_path = bundle_dir.join("rustframe-package.json");
    let metadata = json!({
        "appId": app.config.app_id,
        "name": app.config.title,
        "version": app.config.packaging.version,
        "description": app.config.packaging.description,
        "publisher": app.config.packaging.publisher,
        "homepage": app.config.packaging.homepage,
        "target": {
            "os": "macos",
            "arch": env::consts::ARCH,
            "format": "app-bundle-tarball"
        },
        "artifacts": {
            "bundleDir": bundle_dir.file_name().map(|value| value.to_string_lossy().to_string()),
            "appBundle": app_bundle.file_name().map(|value| value.to_string_lossy().to_string()),
            "archive": format!("{bundle_name}.tar.gz")
        }
    });
    write_text_file(
        &metadata_path,
        &serde_json::to_string_pretty(&metadata)
            .map_err(|error| format!("failed to serialize package metadata: {error}"))?,
    )?;

    let install_script = bundle_dir.join("install.sh");
    write_text_file(
        &install_script,
        &render_macos_install_script(
            app_bundle
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(&app_bundle_name),
        ),
    )?;
    make_executable(&install_script)?;

    let uninstall_script = bundle_dir.join("uninstall.sh");
    write_text_file(
        &uninstall_script,
        &render_macos_uninstall_script(
            app_bundle
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(&app_bundle_name),
        ),
    )?;
    make_executable(&uninstall_script)?;

    write_text_file(
        &bundle_dir.join("README.txt"),
        &render_macos_package_readme(app, &bundle_name, &app_bundle, icon_file_name.as_deref()),
    )?;

    let archive_path = dist_dir.join(format!("{bundle_name}.tar.gz"));
    if archive_path.exists() {
        fs::remove_file(&archive_path).map_err(|error| {
            format!(
                "failed to replace existing archive '{}': {error}",
                archive_path.display()
            )
        })?;
    }
    write_tarball(&bundle_dir, &archive_path)?;

    Ok(MacOsPackageOutput {
        bundle_dir,
        app_bundle,
        archive_path,
    })
}

fn render_app_run_script(binary_name: &str) -> String {
    format!(
        "#!/usr/bin/env bash\nset -euo pipefail\nbundle_dir=\"$(cd \"$(dirname \"${{BASH_SOURCE[0]}}\")\" && pwd)\"\nexec \"$bundle_dir/usr/bin/{binary_name}\" \"$@\"\n"
    )
}

fn render_portable_desktop_entry(
    title: &str,
    description: &str,
    app_id: &str,
    categories: &str,
    keywords: Option<&str>,
) -> String {
    let mut entry = String::new();
    entry.push_str("[Desktop Entry]\n");
    entry.push_str("Type=Application\n");
    entry.push_str(&format!("Name={}\n", sanitize_desktop_entry_value(title)));
    entry.push_str(&format!(
        "Comment={}\n",
        sanitize_desktop_entry_value(description)
    ));
    entry.push_str("Exec=AppRun\n");
    entry.push_str(&format!("Icon={app_id}\n"));
    entry.push_str(&format!("Categories={categories}\n"));
    if let Some(keywords) = keywords {
        entry.push_str(&format!("Keywords={keywords}\n"));
    }
    entry.push_str("Terminal=false\n");
    entry.push_str("StartupNotify=true\n");
    entry
}

fn render_linux_install_script(
    title: &str,
    app_id: &str,
    description: &str,
    desktop_entry_name: &str,
    icon_relative_path: &Path,
    _icon_file_name: &str,
    categories: &str,
    keywords: Option<&str>,
) -> String {
    let keywords_line = keywords
        .map(|value| format!("Keywords={}\n", sanitize_desktop_entry_value(value)))
        .unwrap_or_default();
    format!(
        "#!/usr/bin/env bash\nset -euo pipefail\nbundle_root=\"$(cd \"$(dirname \"${{BASH_SOURCE[0]}}\")\" && pwd)\"\napp_id={app_id}\napp_title={title}\ndescription={description}\ndesktop_entry_name={desktop_entry_name}\nappdir_name={appdir_name}\ninstall_root=\"${{XDG_DATA_HOME:-$HOME/.local/share}}/rustframe/apps/${{app_id}}\"\ndesktop_dir=\"${{XDG_DATA_HOME:-$HOME/.local/share}}/applications\"\nbin_dir=\"$HOME/.local/bin\"\nmkdir -p \"$install_root\" \"$desktop_dir\" \"$bin_dir\"\nrm -rf \"$install_root\"\ncp -R \"$bundle_root/${{appdir_name}}\" \"$install_root/\"\ncat > \"$bin_dir/${{app_id}}\" <<WRAPPER\n#!/usr/bin/env bash\nset -euo pipefail\nexec \"$install_root/${{appdir_name}}/AppRun\" \"$@\"\nWRAPPER\nchmod +x \"$bin_dir/${{app_id}}\"\ncat > \"$desktop_dir/${{desktop_entry_name}}\" <<DESKTOP\n[Desktop Entry]\nType=Application\nName=$app_title\nComment=$description\nExec=$bin_dir/$app_id\nIcon=$install_root/${{appdir_name}}/{icon_relative}\nCategories={categories}\n{keywords_line}Terminal=false\nStartupNotify=true\nDESKTOP\nif command -v update-desktop-database >/dev/null 2>&1; then\n  update-desktop-database \"$desktop_dir\" >/dev/null 2>&1 || true\nfi\nprintf 'Installed %s to %s\\n' \"$app_id\" \"$install_root/${{appdir_name}}\"\n",
        app_id = shell_single_quoted(app_id),
        title = shell_single_quoted(title),
        description = shell_single_quoted(description),
        desktop_entry_name = shell_single_quoted(desktop_entry_name),
        appdir_name = shell_single_quoted(&format!("{app_id}.AppDir")),
        icon_relative = slash_path(icon_relative_path),
        categories = sanitize_desktop_entry_value(categories),
        keywords_line = keywords_line,
    )
}

fn render_linux_uninstall_script(app_id: &str) -> String {
    format!(
        "#!/usr/bin/env bash\nset -euo pipefail\napp_id={app_id}\ninstall_root=\"${{XDG_DATA_HOME:-$HOME/.local/share}}/rustframe/apps/${{app_id}}\"\ndesktop_file=\"${{XDG_DATA_HOME:-$HOME/.local/share}}/applications/${{app_id}}.desktop\"\nwrapper=\"$HOME/.local/bin/${{app_id}}\"\nrm -rf \"$install_root\"\nrm -f \"$desktop_file\" \"$wrapper\"\nif command -v update-desktop-database >/dev/null 2>&1; then\n  update-desktop-database \"${{XDG_DATA_HOME:-$HOME/.local/share}}/applications\" >/dev/null 2>&1 || true\nfi\nprintf 'Removed %s\\n' \"$app_id\"\n",
        app_id = shell_single_quoted(app_id),
    )
}

fn render_linux_package_readme(app: &AppProject, bundle_name: &str, app_dir: &Path) -> String {
    let homepage = app
        .config
        .packaging
        .homepage
        .as_deref()
        .unwrap_or("not set");
    let publisher = app
        .config
        .packaging
        .publisher
        .as_deref()
        .unwrap_or("not set");
    format!(
        "{title}\nVersion: {version}\nBundle: {bundle_name}\nPublisher: {publisher}\nHomepage: {homepage}\n\nThis Linux package contains:\n- a portable AppDir at {app_dir_name}\n- install.sh for per-user installation under ~/.local\n- uninstall.sh to remove that installation\n- rustframe-package.json with release metadata\n\nPortable run:\n  ./{app_dir_name}/AppRun\n\nUser install:\n  ./install.sh\n",
        title = app.config.title,
        version = app.config.packaging.version,
        bundle_name = bundle_name,
        publisher = publisher,
        homepage = homepage,
        app_dir_name = app_dir
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.AppDir", app.config.app_id)),
    )
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn render_windows_install_script(
    title: &str,
    app_id: &str,
    description: &str,
    portable_dir_name: &str,
    binary_name: &str,
    icon_file_name: Option<&str>,
) -> String {
    let icon_line = if let Some(icon_file_name) = icon_file_name {
        if icon_file_name.ends_with(".ico") {
            format!(
                "$shortcut.IconLocation = Join-Path $installRoot {icon}\n",
                icon = powershell_single_quoted(icon_file_name)
            )
        } else {
            "$shortcut.IconLocation = $exePath\n".to_string()
        }
    } else {
        "$shortcut.IconLocation = $exePath\n".to_string()
    };

    format!(
        "$ErrorActionPreference = 'Stop'\n$bundleRoot = Split-Path -Parent $MyInvocation.MyCommand.Path\n$appId = {app_id}\n$appTitle = {title}\n$description = {description}\n$portableDirName = {portable_dir_name}\n$binaryName = {binary_name}\n$installRoot = Join-Path $env:LOCALAPPDATA (Join-Path 'RustFrame\\Apps' $appId)\n$sourceRoot = Join-Path $bundleRoot $portableDirName\n$exePath = Join-Path $installRoot $binaryName\n$startMenuDir = Join-Path $env:APPDATA 'Microsoft\\Windows\\Start Menu\\Programs'\n$desktopDir = [Environment]::GetFolderPath('Desktop')\n$startShortcutPath = Join-Path $startMenuDir ($appTitle + '.lnk')\n$desktopShortcutPath = Join-Path $desktopDir ($appTitle + '.lnk')\nif (Test-Path $installRoot) {{ Remove-Item $installRoot -Recurse -Force }}\nNew-Item -ItemType Directory -Force -Path (Split-Path -Parent $installRoot), $startMenuDir, $desktopDir | Out-Null\nCopy-Item $sourceRoot $installRoot -Recurse -Force\n$wsh = New-Object -ComObject WScript.Shell\nforeach ($shortcutPath in @($startShortcutPath, $desktopShortcutPath)) {{\n  $shortcut = $wsh.CreateShortcut($shortcutPath)\n  $shortcut.TargetPath = $exePath\n  $shortcut.WorkingDirectory = $installRoot\n  $shortcut.Description = $description\n  {icon_line}  $shortcut.Save()\n}}\nWrite-Host \"Installed $appId to $installRoot\"\n",
        app_id = powershell_single_quoted(app_id),
        title = powershell_single_quoted(title),
        description = powershell_single_quoted(description),
        portable_dir_name = powershell_single_quoted(portable_dir_name),
        binary_name = powershell_single_quoted(binary_name),
        icon_line = icon_line,
    )
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn render_windows_uninstall_script(title: &str, app_id: &str) -> String {
    format!(
        "$ErrorActionPreference = 'Stop'\n$appId = {app_id}\n$appTitle = {title}\n$installRoot = Join-Path $env:LOCALAPPDATA (Join-Path 'RustFrame\\Apps' $appId)\n$startShortcutPath = Join-Path (Join-Path $env:APPDATA 'Microsoft\\Windows\\Start Menu\\Programs') ($appTitle + '.lnk')\n$desktopShortcutPath = Join-Path ([Environment]::GetFolderPath('Desktop')) ($appTitle + '.lnk')\nif (Test-Path $installRoot) {{ Remove-Item $installRoot -Recurse -Force }}\nforeach ($path in @($startShortcutPath, $desktopShortcutPath)) {{\n  if (Test-Path $path) {{ Remove-Item $path -Force }}\n}}\nWrite-Host \"Removed $appId\"\n",
        app_id = powershell_single_quoted(app_id),
        title = powershell_single_quoted(title),
    )
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn render_windows_package_readme(
    app: &AppProject,
    bundle_name: &str,
    portable_dir: &Path,
) -> String {
    let homepage = app
        .config
        .packaging
        .homepage
        .as_deref()
        .unwrap_or("not set");
    let publisher = app
        .config
        .packaging
        .publisher
        .as_deref()
        .unwrap_or("not set");
    format!(
        "{title}\nVersion: {version}\nBundle: {bundle_name}\nPublisher: {publisher}\nHomepage: {homepage}\n\nThis Windows package contains:\n- a portable app directory at {portable_dir_name}\n- install.ps1 for per-user installation under %LOCALAPPDATA%\n- uninstall.ps1 to remove that installation\n- rustframe-package.json with release metadata\n- a .zip archive for distribution\n\nPortable run:\n  .\\{portable_dir_name}\\{binary_name}\n\nUser install:\n  powershell -ExecutionPolicy Bypass -File .\\install.ps1\n",
        title = app.config.title,
        version = app.config.packaging.version,
        bundle_name = bundle_name,
        publisher = publisher,
        homepage = homepage,
        portable_dir_name = portable_dir
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| app.config.app_id.clone()),
        binary_name = package_executable_name(&app.name, "windows"),
    )
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn render_macos_info_plist(
    title: &str,
    binary_name: &str,
    bundle_identifier: &str,
    version: &str,
    icon_name: Option<&str>,
) -> String {
    let icon_entry = icon_name
        .map(|icon_name| {
            format!(
                "    <key>CFBundleIconFile</key>\n    <string>{}</string>\n",
                xml_escape(icon_name)
            )
        })
        .unwrap_or_default();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n    <key>CFBundleDevelopmentRegion</key>\n    <string>en</string>\n    <key>CFBundleDisplayName</key>\n    <string>{title}</string>\n    <key>CFBundleExecutable</key>\n    <string>{binary_name}</string>\n    <key>CFBundleIdentifier</key>\n    <string>{bundle_identifier}</string>\n    <key>CFBundleInfoDictionaryVersion</key>\n    <string>6.0</string>\n    <key>CFBundleName</key>\n    <string>{title}</string>\n    <key>CFBundlePackageType</key>\n    <string>APPL</string>\n    <key>CFBundleShortVersionString</key>\n    <string>{version}</string>\n    <key>CFBundleVersion</key>\n    <string>{version}</string>\n{icon_entry}    <key>LSMinimumSystemVersion</key>\n    <string>11.0</string>\n    <key>NSHighResolutionCapable</key>\n    <true/>\n</dict>\n</plist>\n",
        title = xml_escape(title),
        binary_name = xml_escape(binary_name),
        bundle_identifier = xml_escape(bundle_identifier),
        version = xml_escape(version),
        icon_entry = icon_entry,
    )
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn render_macos_install_script(app_bundle_name: &str) -> String {
    format!(
        "#!/usr/bin/env bash\nset -euo pipefail\nbundle_root=\"$(cd \"$(dirname \"${{BASH_SOURCE[0]}}\")\" && pwd)\"\napp_name={app_name}\ninstall_root=\"$HOME/Applications\"\nmkdir -p \"$install_root\"\nrm -rf \"$install_root/$app_name\"\ncp -R \"$bundle_root/$app_name\" \"$install_root/\"\nprintf 'Installed %s to %s\\n' \"$app_name\" \"$install_root/$app_name\"\n",
        app_name = shell_single_quoted(app_bundle_name),
    )
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn render_macos_uninstall_script(app_bundle_name: &str) -> String {
    format!(
        "#!/usr/bin/env bash\nset -euo pipefail\napp_name={app_name}\ninstall_root=\"$HOME/Applications\"\nrm -rf \"$install_root/$app_name\"\nprintf 'Removed %s\\n' \"$app_name\"\n",
        app_name = shell_single_quoted(app_bundle_name),
    )
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn render_macos_package_readme(
    app: &AppProject,
    bundle_name: &str,
    app_bundle: &Path,
    _icon_file_name: Option<&str>,
) -> String {
    let homepage = app
        .config
        .packaging
        .homepage
        .as_deref()
        .unwrap_or("not set");
    let publisher = app
        .config
        .packaging
        .publisher
        .as_deref()
        .unwrap_or("not set");
    format!(
        "{title}\nVersion: {version}\nBundle: {bundle_name}\nPublisher: {publisher}\nHomepage: {homepage}\nBundle Identifier: {bundle_identifier}\n\nThis macOS package contains:\n- an app bundle at {app_bundle_name}\n- install.sh for per-user installation under ~/Applications\n- uninstall.sh to remove that installation\n- rustframe-package.json with release metadata\n\nPortable run:\n  open ./{app_bundle_name}\n\nUser install:\n  ./install.sh\n",
        title = app.config.title,
        version = app.config.packaging.version,
        bundle_name = bundle_name,
        publisher = publisher,
        homepage = homepage,
        bundle_identifier = app.config.packaging.macos.bundle_identifier,
        app_bundle_name = app_bundle
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.app", sanitize_bundle_file_name(&app.config.title))),
    )
}

fn format_desktop_categories(categories: &[String]) -> String {
    format!("{};", categories.join(";"))
}

fn format_desktop_keywords(keywords: &[String]) -> Option<String> {
    if keywords.is_empty() {
        None
    } else {
        Some(format!("{};", keywords.join(";")))
    }
}

fn sanitize_desktop_entry_value(value: &str) -> String {
    value.replace('\n', " ").trim().to_string()
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn sanitize_bundle_file_name(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|character| {
            if matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            ) {
                '-'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .to_string();
    if sanitized.is_empty() {
        sanitized = "RustFrame".to_string();
    }
    sanitized
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

struct LinuxIconBytes {
    bytes: Vec<u8>,
    extension: String,
}

fn load_linux_icon(app: &AppProject) -> CliResult<LinuxIconBytes> {
    if let Some(path) = &app.config.packaging.linux.icon_path {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .ok_or_else(|| {
                format!(
                    "packaging.linux.icon must end with .svg or .png: {}",
                    path.display()
                )
            })?;
        let bytes = fs::read(path)
            .map_err(|error| format!("failed to read icon '{}': {error}", path.display()))?;
        return Ok(LinuxIconBytes { bytes, extension });
    }

    Ok(LinuxIconBytes {
        bytes: render_template(
            TEMPLATE_APP_ICON_SVG,
            &[
                ("{{app_title}}", app.config.title.clone()),
                ("{{app_monogram}}", icon_monogram(&app.config.title)),
            ],
        )
        .into_bytes(),
        extension: "svg".to_string(),
    })
}

#[cfg_attr(
    not(any(test, target_os = "windows", target_os = "macos")),
    allow(dead_code)
)]
struct PackagingIconBytes {
    bytes: Vec<u8>,
    extension: String,
}

#[cfg_attr(
    not(any(test, target_os = "windows", target_os = "macos")),
    allow(dead_code)
)]
fn load_packaging_icon(
    app: &AppProject,
    primary_path: Option<&Path>,
    fallback_path: Option<&Path>,
) -> CliResult<Option<PackagingIconBytes>> {
    if let Some(path) = primary_path.or(fallback_path) {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .ok_or_else(|| format!("icon path must have an extension: {}", path.display()))?;
        let bytes = fs::read(path)
            .map_err(|error| format!("failed to read icon '{}': {error}", path.display()))?;
        return Ok(Some(PackagingIconBytes { bytes, extension }));
    }

    Ok(Some(PackagingIconBytes {
        bytes: render_template(
            TEMPLATE_APP_ICON_SVG,
            &[
                ("{{app_title}}", app.config.title.clone()),
                ("{{app_monogram}}", icon_monogram(&app.config.title)),
            ],
        )
        .into_bytes(),
        extension: "svg".to_string(),
    }))
}

fn write_tarball(source_dir: &Path, archive_path: &Path) -> CliResult<()> {
    let archive_file = fs::File::create(archive_path)
        .map_err(|error| format!("failed to create '{}': {error}", archive_path.display()))?;
    let encoder = GzEncoder::new(archive_file, Compression::default());
    let mut builder = TarBuilder::new(encoder);
    let root_name = source_dir
        .file_name()
        .ok_or_else(|| format!("failed to archive '{}'", source_dir.display()))?;
    builder
        .append_dir_all(root_name, source_dir)
        .map_err(|error| format!("failed to archive '{}': {error}", source_dir.display()))?;
    let encoder = builder
        .into_inner()
        .map_err(|error| format!("failed to finalize '{}': {error}", archive_path.display()))?;
    encoder
        .finish()
        .map_err(|error| format!("failed to finalize '{}': {error}", archive_path.display()))?;
    Ok(())
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn write_zip(source_dir: &Path, archive_path: &Path) -> CliResult<()> {
    let archive_file = fs::File::create(archive_path)
        .map_err(|error| format!("failed to create '{}': {error}", archive_path.display()))?;
    let mut writer = ZipWriter::new(archive_file);
    let root_name = source_dir
        .file_name()
        .ok_or_else(|| format!("failed to archive '{}'", source_dir.display()))?
        .to_string_lossy()
        .to_string();
    add_directory_to_zip(&mut writer, source_dir, source_dir, &root_name)?;
    writer
        .finish()
        .map_err(|error| format!("failed to finalize '{}': {error}", archive_path.display()))?;
    Ok(())
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn add_directory_to_zip(
    writer: &mut ZipWriter<fs::File>,
    root: &Path,
    directory: &Path,
    root_name: &str,
) -> CliResult<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read '{}': {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read '{}': {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("failed to resolve '{}': {error}", path.display()))?;
        let archive_path = if relative.as_os_str().is_empty() {
            root_name.to_string()
        } else {
            format!("{root_name}/{}", slash_path(relative))
        };

        if metadata.is_dir() {
            writer
                .add_directory(
                    format!("{archive_path}/"),
                    zip_file_options(metadata.permissions().readonly()),
                )
                .map_err(|error| format!("failed to archive '{}': {error}", path.display()))?;
            add_directory_to_zip(writer, root, &path, root_name)?;
            continue;
        }

        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
        writer
            .start_file(
                archive_path,
                zip_file_options(metadata.permissions().readonly()),
            )
            .map_err(|error| format!("failed to archive '{}': {error}", path.display()))?;
        use std::io::Write;
        writer
            .write_all(&bytes)
            .map_err(|error| format!("failed to archive '{}': {error}", path.display()))?;
    }

    Ok(())
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn zip_file_options(readonly: bool) -> FileOptions {
    let mut options = FileOptions::default().compression_method(CompressionMethod::Deflated);
    #[cfg(unix)]
    {
        let permissions = if readonly { 0o644 } else { 0o755 };
        options = options.unix_permissions(permissions);
    }
    options
}

#[cfg(unix)]
fn make_executable(path: &Path) -> CliResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to update '{}': {error}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(path: &Path) -> CliResult<()> {
    let _ = path;
    Ok(())
}

fn print_help() {
    println!("RustFrame CLI");
    println!();
    println!("Commands:");
    println!("  rustframe-cli new <name>              Create a frontend-only app in apps/<name>");
    println!(
        "  rustframe-cli dev [name] [dev-url]    Run an app from a hidden generated Rust runner"
    );
    println!(
        "  rustframe-cli export [name]           Build a release binary into apps/<name>/dist/"
    );
    println!(
        "  rustframe-cli platform-check [name]   Validate the app against the Linux/Windows/macOS support matrix"
    );
    println!(
        "  rustframe-cli package [name]          Build a host-native bundle into apps/<name>/dist/<platform>/"
    );
    println!(
        "  rustframe-cli inspect [name]          Show resolved paths, capabilities, schema, seeds, and migrations"
    );
    println!(
        "  rustframe-cli reset-data [name]       Remove the local app data directory so schema and seeds are recreated"
    );
    println!(
        "  rustframe-cli eject [name]            Materialize an app-owned Rust runner in apps/<name>/native/"
    );
    println!();
    println!(
        "Run `dev`, `export`, `platform-check`, and `package` from inside apps/<name>/ to omit the app name."
    );
    println!("Primary app config lives in apps/<name>/rustframe.json:");
    println!("  \"window\": {{ \"title\": \"My App\", \"width\": 1280, \"height\": 820 }}");
    println!("  \"devUrl\": \"http://127.0.0.1:5173\"");
    println!(
        "Use `platform-check --target <triple>` to validate a custom Rust target or a narrowed matrix."
    );
    println!("HTML <title> and rustframe:* meta tags still work as fallback.");
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, path::Path};

    use tempfile::tempdir;

    use super::{
        AppConfig, AppPackagingConfig, AppProject, AppSecurityConfig, AppSecurityModel,
        AppShellCommand, DEFAULT_PLATFORM_TARGETS, LinuxPackagingConfig, MacOsPackagingConfig,
        WindowsPackagingConfig, build_app_inspection, build_linux_package, build_macos_package,
        build_windows_package, collect_embedded_assets, find_workspace_root_from, load_app_project,
        parse_platform_check_args, platform_target_spec, prepare_ejected_runner,
        prepare_generated_runner, read_app_config, relative_path, render_asset_match_arms,
        render_database_chain, render_template, resolve_current_app_name_from,
        resolve_runner_project, sync_declared_fs_roots,
    };

    fn write_workspace_manifest(root: &Path) {
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = []\n# crates/rustframe\n",
        )
        .unwrap();
    }

    fn default_packaging_config(title: &str) -> AppPackagingConfig {
        AppPackagingConfig {
            version: "0.1.0".into(),
            description: title.into(),
            publisher: None,
            homepage: None,
            linux: LinuxPackagingConfig {
                categories: vec!["Utility".into()],
                keywords: Vec::new(),
                icon_path: None,
            },
            windows: WindowsPackagingConfig { icon_path: None },
            macos: MacOsPackagingConfig {
                bundle_identifier: "dev.rustframe.test-app".into(),
                icon_path: None,
            },
        }
    }

    #[test]
    fn reads_app_config_from_html_meta_tags() {
        let temp = tempdir().unwrap();
        let index = temp.path().join("index.html");
        fs::write(
            &index,
            r#"
            <!DOCTYPE html>
            <html>
            <head>
              <title>Orbit Desk</title>
              <meta name="rustframe:width" content="1440">
              <meta name="rustframe:height" content="920">
              <meta name="rustframe:dev-url" content="http://127.0.0.1:5173">
            </head>
            </html>
            "#,
        )
        .unwrap();

        let config = read_app_config("orbit-desk", temp.path(), temp.path()).unwrap();

        assert_eq!(config.app_id, "orbit-desk");
        assert_eq!(config.title, "Orbit Desk");
        assert_eq!(config.width, 1440.0);
        assert_eq!(config.height, 920.0);
        assert_eq!(config.dev_url.as_deref(), Some("http://127.0.0.1:5173"));
        assert!(config.fs_roots.is_empty());
        assert!(config.shell_commands.is_empty());
        assert_eq!(config.packaging.version, "0.1.0");
        assert_eq!(config.packaging.description, "Orbit Desk");
    }

    #[test]
    fn app_config_falls_back_to_defaults_when_meta_is_missing() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<html><head></head></html>").unwrap();

        let config = read_app_config("ember-habits", temp.path(), temp.path()).unwrap();

        assert_eq!(config.app_id, "ember-habits");
        assert_eq!(config.title, "Ember Habits");
        assert_eq!(config.width, 1280.0);
        assert_eq!(config.height, 820.0);
        assert_eq!(config.dev_url, None);
    }

    #[test]
    fn app_config_supports_single_quoted_meta_attributes() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join("index.html"),
            r#"
            <title>Atlas CRM</title>
            <meta name='rustframe:width' content='1380'>
            <meta name='rustframe:height' content='880'>
            "#,
        )
        .unwrap();

        let config = read_app_config("atlas-crm", temp.path(), temp.path()).unwrap();

        assert_eq!(config.title, "Atlas CRM");
        assert_eq!(config.width, 1380.0);
        assert_eq!(config.height, 880.0);
    }

    #[test]
    fn app_config_rejects_invalid_dimensions() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join("index.html"),
            r#"<meta name="rustframe:width" content="zero">"#,
        )
        .unwrap();

        let error = read_app_config("ember-habits", temp.path(), temp.path()).unwrap_err();
        assert!(error.contains("rustframe:width must be a number"));
    }

    #[test]
    fn app_config_reads_manifest_declared_capabilities() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join("index.html"),
            "<title>Manifest Demo</title>",
        )
        .unwrap();
        fs::write(
            temp.path().join("rustframe.json"),
            r#"
            {
              "appId": "manifest_demo",
              "security": {
                "model": "networked",
                "bridge": {
                  "database": true
                }
              },
              "filesystem": {
                "roots": ["fixtures", "${EXE_DIR}/imports"]
              },
              "shell": {
                "commands": [
                  {
                    "name": "listFixtures",
                    "program": "ls",
                    "args": ["-la", "${SOURCE_APP_DIR}/fixtures"],
                    "allowedArgs": ["--json"],
                    "cwd": "${SOURCE_APP_DIR}",
                    "env": {
                      "LC_ALL": "C"
                    },
                    "clearEnv": true,
                    "timeoutMs": 2500,
                    "maxOutputBytes": 8192
                  }
                ]
              }
            }
            "#,
        )
        .unwrap();

        let config = read_app_config("manifest-demo", temp.path(), temp.path()).unwrap();

        assert_eq!(config.app_id, "manifest_demo");
        assert_eq!(config.security.model, AppSecurityModel::Networked);
        assert!(config.security.database);
        assert!(!config.security.filesystem);
        assert!(!config.security.shell);
        assert_eq!(config.fs_roots, vec!["fixtures", "${EXE_DIR}/imports"]);
        assert_eq!(config.shell_commands.len(), 1);
        assert_eq!(config.shell_commands[0].name, "listFixtures");
        assert_eq!(config.shell_commands[0].program, "ls");
        assert_eq!(
            config.shell_commands[0].args,
            vec!["-la", "${SOURCE_APP_DIR}/fixtures"]
        );
        assert_eq!(config.shell_commands[0].allowed_args, vec!["--json"]);
        assert_eq!(
            config.shell_commands[0].cwd.as_deref(),
            Some("${SOURCE_APP_DIR}")
        );
        assert_eq!(
            config.shell_commands[0].env,
            BTreeMap::from([("LC_ALL".to_string(), "C".to_string())])
        );
        assert!(config.shell_commands[0].clear_env);
        assert_eq!(config.shell_commands[0].timeout_ms, Some(2500));
        assert_eq!(config.shell_commands[0].max_output_bytes, Some(8192));
    }

    #[test]
    fn app_config_defaults_to_local_first_security() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join("index.html"),
            "<title>Security Demo</title>",
        )
        .unwrap();

        let config = read_app_config("security-demo", temp.path(), temp.path()).unwrap();

        assert_eq!(config.security.model, AppSecurityModel::LocalFirst);
        assert!(config.security.database);
        assert!(config.security.filesystem);
        assert!(config.security.shell);
    }

    #[test]
    fn manifest_window_values_override_html_metadata() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join("index.html"),
            r#"
            <title>HTML Title</title>
            <meta name="rustframe:width" content="1280">
            <meta name="rustframe:height" content="820">
            "#,
        )
        .unwrap();
        fs::write(
            temp.path().join("rustframe.json"),
            r#"
            {
              "window": {
                "title": "Manifest Title",
                "width": 1440,
                "height": 920
              },
              "devUrl": "http://127.0.0.1:4321"
            }
            "#,
        )
        .unwrap();

        let config = read_app_config("manifest-demo", temp.path(), temp.path()).unwrap();

        assert_eq!(config.title, "Manifest Title");
        assert_eq!(config.width, 1440.0);
        assert_eq!(config.height, 920.0);
        assert_eq!(config.dev_url.as_deref(), Some("http://127.0.0.1:4321"));
    }

    #[test]
    fn manifest_reads_linux_packaging_metadata() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join("index.html"),
            "<title>Packaged App</title>",
        )
        .unwrap();
        fs::write(temp.path().join("assets-icon.svg"), "<svg/>").unwrap();
        fs::write(
            temp.path().join("rustframe.json"),
            r#"
            {
              "packaging": {
                "version": "1.2.3",
                "description": "Ship it on Linux",
                "publisher": "RustFrame Labs",
                "homepage": "https://example.com/app",
                "linux": {
                  "icon": "assets-icon.svg",
                  "categories": ["Office", "Utility"],
                  "keywords": ["crm", "sales"]
                },
                "windows": {
                  "icon": "assets-icon.svg"
                },
                "macos": {
                  "bundleIdentifier": "dev.rustframe.packaged-app",
                  "icon": "assets-icon.svg"
                }
              }
            }
            "#,
        )
        .unwrap();

        let config = read_app_config("packaged-app", temp.path(), temp.path()).unwrap();

        assert_eq!(config.packaging.version, "1.2.3");
        assert_eq!(config.packaging.description, "Ship it on Linux");
        assert_eq!(
            config.packaging.publisher.as_deref(),
            Some("RustFrame Labs")
        );
        assert_eq!(
            config.packaging.homepage.as_deref(),
            Some("https://example.com/app")
        );
        assert_eq!(
            config.packaging.linux.categories,
            vec!["Office".to_string(), "Utility".to_string()]
        );
        assert_eq!(
            config.packaging.linux.keywords,
            vec!["crm".to_string(), "sales".to_string()]
        );
        assert_eq!(
            config.packaging.linux.icon_path,
            Some(temp.path().join("assets-icon.svg"))
        );
        assert_eq!(
            config.packaging.windows.icon_path,
            Some(temp.path().join("assets-icon.svg"))
        );
        assert_eq!(
            config.packaging.macos.bundle_identifier,
            "dev.rustframe.packaged-app"
        );
        assert_eq!(
            config.packaging.macos.icon_path,
            Some(temp.path().join("assets-icon.svg"))
        );
    }

    #[test]
    fn manifest_window_config_works_without_html_meta_tags() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<title>HTML Title</title>").unwrap();
        fs::write(
            temp.path().join("rustframe.json"),
            r#"
            {
              "window": {
                "title": "Manifest Window",
                "width": 1366,
                "height": 900
              }
            }
            "#,
        )
        .unwrap();

        let config = read_app_config("window-demo", temp.path(), temp.path()).unwrap();

        assert_eq!(config.title, "Manifest Window");
        assert_eq!(config.width, 1366.0);
        assert_eq!(config.height, 900.0);
    }

    #[test]
    fn manifest_rejects_duplicate_shell_command_names() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join("index.html"),
            "<title>Manifest Demo</title>",
        )
        .unwrap();
        fs::write(
            temp.path().join("rustframe.json"),
            r#"
            {
              "shell": {
                "commands": [
                  { "name": "sync", "program": "echo" },
                  { "name": "sync", "program": "printf" }
                ]
              }
            }
            "#,
        )
        .unwrap();

        let error = read_app_config("manifest-demo", temp.path(), temp.path()).unwrap_err();
        assert!(error.contains("shell.commands defines 'sync' more than once"));
    }

    #[test]
    fn manifest_rejects_zero_shell_timeout() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join("index.html"),
            "<title>Manifest Demo</title>",
        )
        .unwrap();
        fs::write(
            temp.path().join("rustframe.json"),
            r#"
            {
              "shell": {
                "commands": [
                  {
                    "name": "sync",
                    "program": "echo",
                    "timeoutMs": 0
                  }
                ]
              }
            }
            "#,
        )
        .unwrap();

        let error = read_app_config("manifest-demo", temp.path(), temp.path()).unwrap_err();
        assert!(error.contains("shell.commands['sync'].timeoutMs must be greater than zero"));
    }

    #[test]
    fn manifest_rejects_invalid_shell_env_key() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join("index.html"),
            "<title>Manifest Demo</title>",
        )
        .unwrap();
        fs::write(
            temp.path().join("rustframe.json"),
            r#"
            {
              "shell": {
                "commands": [
                  {
                    "name": "sync",
                    "program": "echo",
                    "env": {
                      "BAD=KEY": "value"
                    }
                  }
                ]
              }
            }
            "#,
        )
        .unwrap();

        let error = read_app_config("manifest-demo", temp.path(), temp.path()).unwrap_err();
        assert!(error.contains("shell.commands['sync'].env defines invalid key 'BAD=KEY'"));
    }

    #[test]
    fn manifest_rejects_invalid_dev_url() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<title>Config Demo</title>").unwrap();
        fs::write(
            temp.path().join("rustframe.json"),
            r#"
            {
              "devUrl": "ftp://127.0.0.1:5173"
            }
            "#,
        )
        .unwrap();

        let error = read_app_config("config-demo", temp.path(), temp.path()).unwrap_err();
        assert!(error.contains("devUrl must start with http:// or https://"));
    }

    #[test]
    fn manifest_rejects_blank_window_title() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<title>Config Demo</title>").unwrap();
        fs::write(
            temp.path().join("rustframe.json"),
            r#"
            {
              "window": {
                "title": "   "
              }
            }
            "#,
        )
        .unwrap();

        let error = read_app_config("config-demo", temp.path(), temp.path()).unwrap_err();
        assert!(error.contains("window.title must not be empty"));
    }

    #[test]
    fn collect_embedded_assets_skips_dist_and_hidden_entries() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<!doctype html>").unwrap();
        fs::write(temp.path().join("styles.css"), "body {}").unwrap();
        fs::create_dir_all(temp.path().join("dist")).unwrap();
        fs::write(temp.path().join("dist/app"), "ignored").unwrap();
        fs::write(temp.path().join(".DS_Store"), "ignored").unwrap();
        fs::create_dir_all(temp.path().join("data")).unwrap();
        fs::write(temp.path().join("data/schema.json"), "{}").unwrap();

        let assets = collect_embedded_assets(temp.path()).unwrap();
        let paths = assets
            .iter()
            .map(|asset| asset.request_path.as_str())
            .collect::<Vec<_>>();

        assert!(paths.contains(&"index.html"));
        assert!(paths.contains(&"styles.css"));
        assert!(paths.contains(&"data/schema.json"));
        assert!(!paths.iter().any(|path| path.starts_with("dist/")));
        assert!(!paths.iter().any(|path| path.starts_with('.')));
    }

    #[test]
    fn collect_embedded_assets_recurses_and_sorts_paths() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<!doctype html>").unwrap();
        fs::create_dir_all(temp.path().join("nested/a")).unwrap();
        fs::create_dir_all(temp.path().join(".hidden")).unwrap();
        fs::write(temp.path().join("nested/a/app.js"), "console.log('ok')").unwrap();
        fs::write(temp.path().join("nested/logo.svg"), "<svg/>").unwrap();
        fs::write(temp.path().join(".hidden/ignored.txt"), "nope").unwrap();

        let assets = collect_embedded_assets(temp.path()).unwrap();
        let paths = assets
            .iter()
            .map(|asset| asset.request_path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec!["index.html", "nested/a/app.js", "nested/logo.svg"]
        );
    }

    #[test]
    fn collect_embedded_assets_requires_index_html() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("styles.css"), "body {}").unwrap();

        let error = collect_embedded_assets(temp.path()).unwrap_err();
        assert!(error.contains("must contain index.html"));
    }

    #[test]
    fn renders_database_chain_only_when_schema_exists() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<!doctype html>").unwrap();
        fs::create_dir_all(temp.path().join("data/seeds")).unwrap();
        fs::write(temp.path().join("data/schema.json"), "{}").unwrap();
        fs::write(temp.path().join("data/seeds/001.json"), "{}").unwrap();
        let assets = collect_embedded_assets(temp.path()).unwrap();

        let chain = render_database_chain(&assets);

        assert!(chain.contains(".embedded_database(\"data/schema.json\""));
        assert!(chain.contains("\"data/seeds/001.json\""));
    }

    #[test]
    fn render_database_chain_includes_sql_migrations_when_present() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<!doctype html>").unwrap();
        fs::create_dir_all(temp.path().join("data/seeds")).unwrap();
        fs::create_dir_all(temp.path().join("data/migrations")).unwrap();
        fs::write(temp.path().join("data/schema.json"), "{}").unwrap();
        fs::write(temp.path().join("data/seeds/001.json"), "{}").unwrap();
        fs::write(
            temp.path().join("data/migrations/002-rename.sql"),
            "ALTER TABLE",
        )
        .unwrap();
        let assets = collect_embedded_assets(temp.path()).unwrap();

        let chain = render_database_chain(&assets);

        assert!(chain.contains(".embedded_database_with_migrations("));
        assert!(chain.contains("\"data/migrations/002-rename.sql\""));
    }

    #[test]
    fn render_database_chain_is_empty_without_schema() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<!doctype html>").unwrap();
        fs::create_dir_all(temp.path().join("data/seeds")).unwrap();
        fs::write(temp.path().join("data/seeds/001.json"), "{}").unwrap();
        let assets = collect_embedded_assets(temp.path()).unwrap();

        assert!(render_database_chain(&assets).is_empty());
    }

    #[test]
    fn build_app_inspection_reports_database_diagnostics() {
        let temp = tempdir().unwrap();
        write_workspace_manifest(temp.path());
        let app_dir = temp.path().join("apps/inspect-demo");
        fs::create_dir_all(app_dir.join("data/migrations")).unwrap();
        fs::create_dir_all(app_dir.join("data/seeds")).unwrap();
        fs::write(app_dir.join("index.html"), "<title>Inspect Demo</title>").unwrap();
        fs::write(app_dir.join("app.js"), "console.log('ok')").unwrap();
        fs::write(app_dir.join("styles.css"), "body {}").unwrap();
        fs::write(
            app_dir.join("data/schema.json"),
            r#"
            {
              "version": 2,
              "tables": [
                {
                  "name": "settings",
                  "columns": [
                    { "name": "enabled", "type": "boolean", "required": true, "default": true }
                  ]
                }
              ]
            }
            "#,
        )
        .unwrap();
        fs::write(
            app_dir.join("data/seeds/001-defaults.json"),
            r#"{"entries":[{"table":"settings","rows":[{"enabled":true}]}]}"#,
        )
        .unwrap();

        let app = load_app_project(temp.path(), "inspect-demo").unwrap();
        let inspection = build_app_inspection(&app).unwrap();

        assert_eq!(inspection["appId"], "inspect-demo");
        assert_eq!(inspection["database"]["schemaVersion"], 2);
        assert_eq!(
            inspection["database"]["seedFiles"][0]["path"],
            "data/seeds/001-defaults.json"
        );
        assert!(
            inspection["database"]["diagnostics"]["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value
                    == "table 'settings' has no text/json columns for runtime full-text search")
        );
        assert!(
            inspection["database"]["diagnostics"]["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value
                    == "schema version 2 is greater than 1 but no SQL migrations were found")
        );
    }

    #[test]
    fn prepare_generated_runner_writes_database_enabled_runner() {
        let workspace = tempdir().unwrap();
        fs::create_dir_all(workspace.path().join("crates/rustframe")).unwrap();
        let app_dir = workspace.path().join("apps/orbit-desk");
        fs::create_dir_all(app_dir.join("data/seeds")).unwrap();
        fs::write(
            app_dir.join("index.html"),
            render_template(
                r#"
                <title>Orbit Desk</title>
                <meta name="rustframe:width" content="1440">
                <meta name="rustframe:height" content="920">
                "#,
                &[],
            ),
        )
        .unwrap();
        fs::write(app_dir.join("app.js"), "console.log('ok')").unwrap();
        fs::write(app_dir.join("styles.css"), "body {}").unwrap();
        fs::write(app_dir.join("data/schema.json"), "{}").unwrap();
        fs::write(app_dir.join("data/seeds/001.json"), "{}").unwrap();

        let app = AppProject {
            name: "orbit-desk".into(),
            app_dir: app_dir.clone(),
            asset_dir: app_dir,
            config: AppConfig {
                app_id: "orbit-desk".into(),
                title: "Orbit Desk".into(),
                width: 1440.0,
                height: 920.0,
                dev_url: None,
                security: AppSecurityConfig::local_first(),
                fs_roots: Vec::new(),
                shell_commands: Vec::new(),
                packaging: default_packaging_config("Orbit Desk"),
            },
        };

        let runner = prepare_generated_runner(workspace.path(), &app).unwrap();
        let main =
            fs::read_to_string(runner.manifest_path.parent().unwrap().join("src/main.rs")).unwrap();

        assert!(main.contains(".app_id(\"orbit-desk\")"));
        assert!(main.contains(".frontend_security(rustframe::FrontendSecurity::local_first())"));
        assert!(
            main.contains(".embedded_database(\"data/schema.json\", &[\"data/seeds/001.json\"])")
        );
    }

    #[test]
    fn prepare_generated_runner_omits_database_chain_when_schema_is_missing() {
        let workspace = tempdir().unwrap();
        fs::create_dir_all(workspace.path().join("crates/rustframe")).unwrap();
        let app_dir = workspace.path().join("apps/atlas-crm");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(
            app_dir.join("index.html"),
            r#"
            <title>Atlas CRM</title>
            <meta name="rustframe:width" content="1280">
            <meta name="rustframe:height" content="820">
            "#,
        )
        .unwrap();
        fs::write(app_dir.join("app.js"), "console.log('ok')").unwrap();
        fs::write(app_dir.join("styles.css"), "body {}").unwrap();

        let app = AppProject {
            name: "atlas-crm".into(),
            app_dir: app_dir.clone(),
            asset_dir: app_dir,
            config: AppConfig {
                app_id: "atlas-crm".into(),
                title: "Atlas CRM".into(),
                width: 1280.0,
                height: 820.0,
                dev_url: None,
                security: AppSecurityConfig::local_first(),
                fs_roots: Vec::new(),
                shell_commands: Vec::new(),
                packaging: default_packaging_config("Atlas CRM"),
            },
        };

        let runner = prepare_generated_runner(workspace.path(), &app).unwrap();
        let main =
            fs::read_to_string(runner.manifest_path.parent().unwrap().join("src/main.rs")).unwrap();

        assert!(main.contains(".app_id(\"atlas-crm\")"));
        assert!(!main.contains(".embedded_database("));
    }

    #[test]
    fn render_asset_match_arms_embeds_absolute_paths() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), "<!doctype html>").unwrap();
        let assets = collect_embedded_assets(temp.path()).unwrap();

        let arms = render_asset_match_arms(&assets);

        assert!(arms.contains("index.html"));
        assert!(arms.contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn resolves_current_app_name_from_nested_directory() {
        let temp = tempdir().unwrap();
        let workspace = temp.path();
        fs::create_dir_all(workspace.join("apps/orbit-desk/frontend/components")).unwrap();

        let current = workspace.join("apps/orbit-desk/frontend/components");
        let name = resolve_current_app_name_from(workspace, &current).unwrap();

        assert_eq!(name, "orbit-desk");
    }

    #[test]
    fn resolve_current_app_name_from_rejects_non_app_directory() {
        let temp = tempdir().unwrap();
        let workspace = temp.path();
        fs::create_dir_all(workspace.join("apps")).unwrap();
        fs::create_dir_all(workspace.join("docs")).unwrap();

        let error = resolve_current_app_name_from(workspace, &workspace.join("docs")).unwrap_err();

        assert!(error.contains("missing app name"));
    }

    #[test]
    fn platform_check_defaults_to_full_support_matrix() {
        let temp = tempdir().unwrap();
        write_workspace_manifest(temp.path());
        let request = parse_platform_check_args(temp.path(), &["orbit-desk".into()]).unwrap();

        assert_eq!(request.name, "orbit-desk");
        assert_eq!(
            request.targets,
            DEFAULT_PLATFORM_TARGETS
                .iter()
                .map(|target| target.triple.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn platform_check_parses_target_flags_and_dedupes_requested_triples() {
        let temp = tempdir().unwrap();
        write_workspace_manifest(temp.path());

        let request = parse_platform_check_args(
            temp.path(),
            &[
                "orbit-desk".into(),
                "--target".into(),
                "x86_64-pc-windows-msvc".into(),
                "--target=x86_64-pc-windows-msvc,aarch64-apple-darwin".into(),
            ],
        )
        .unwrap();

        assert_eq!(request.name, "orbit-desk");
        assert_eq!(
            request.targets,
            vec![
                "x86_64-pc-windows-msvc".to_string(),
                "aarch64-apple-darwin".to_string()
            ]
        );
    }

    #[test]
    fn platform_target_specs_include_windows_and_both_macos_targets() {
        assert_eq!(
            platform_target_spec("x86_64-pc-windows-msvc").map(|spec| spec.label),
            Some("Windows")
        );
        assert_eq!(
            platform_target_spec("x86_64-apple-darwin").map(|spec| spec.label),
            Some("macOS (Intel)")
        );
        assert_eq!(
            platform_target_spec("aarch64-apple-darwin").map(|spec| spec.label),
            Some("macOS (Apple Silicon)")
        );
    }

    #[test]
    fn finds_workspace_root_from_nested_path() {
        let temp = tempdir().unwrap();
        write_workspace_manifest(temp.path());
        fs::create_dir_all(temp.path().join("apps/prism-gallery/assets")).unwrap();

        let root =
            find_workspace_root_from(&temp.path().join("apps/prism-gallery/assets")).unwrap();

        assert_eq!(root, temp.path());
    }

    #[test]
    fn load_app_project_supports_legacy_frontend_subdirectory() {
        let temp = tempdir().unwrap();
        write_workspace_manifest(temp.path());
        let app_dir = temp.path().join("apps/legacy-demo/frontend");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(
            app_dir.join("index.html"),
            r#"<title>Legacy Demo</title><meta name="rustframe:width" content="1024">"#,
        )
        .unwrap();

        let project = load_app_project(temp.path(), "legacy-demo").unwrap();

        assert_eq!(project.name, "legacy-demo");
        assert_eq!(project.asset_dir, app_dir);
        assert_eq!(project.config.app_id, "legacy-demo");
        assert_eq!(project.config.title, "Legacy Demo");
        assert_eq!(project.config.width, 1024.0);
    }

    #[test]
    fn prepare_generated_runner_writes_declared_fs_and_shell_capabilities() {
        let workspace = tempdir().unwrap();
        fs::create_dir_all(workspace.path().join("crates/rustframe")).unwrap();
        let app_dir = workspace.path().join("apps/capability-app");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("index.html"), "<title>Capability App</title>").unwrap();
        fs::write(app_dir.join("app.js"), "console.log('ok')").unwrap();
        fs::write(app_dir.join("styles.css"), "body {}").unwrap();
        let mut security = AppSecurityConfig::networked();
        security.database = true;

        let app = AppProject {
            name: "capability-app".into(),
            app_dir: app_dir.clone(),
            asset_dir: app_dir.clone(),
            config: AppConfig {
                app_id: "capability_app".into(),
                title: "Capability App".into(),
                width: 1280.0,
                height: 820.0,
                dev_url: None,
                security,
                fs_roots: vec!["fixtures".into(), "${EXE_DIR}/imports".into()],
                shell_commands: vec![AppShellCommand {
                    name: "listFixtures".into(),
                    program: "ls".into(),
                    args: vec!["-la".into(), "${SOURCE_APP_DIR}/fixtures".into()],
                    allowed_args: vec!["--json".into(), "${EXE_DIR}/flags.txt".into()],
                    cwd: Some("${SOURCE_APP_DIR}".into()),
                    env: BTreeMap::from([("LC_ALL".into(), "C".into())]),
                    clear_env: true,
                    timeout_ms: Some(2_500),
                    max_output_bytes: Some(8_192),
                }],
                packaging: default_packaging_config("Capability App"),
            },
        };

        let runner = prepare_generated_runner(workspace.path(), &app).unwrap();
        let main =
            fs::read_to_string(runner.manifest_path.parent().unwrap().join("src/main.rs")).unwrap();

        assert!(main.contains("fn resolve_declared_fs_root"));
        assert!(main.contains(".allow_fs_root(resolve_declared_fs_root(\"fixtures\"))"));
        assert!(main.contains(
            ".frontend_security(rustframe::FrontendSecurity::networked().database(true))"
        ));
        assert!(main.contains("${SOURCE_APP_DIR}"));
        assert!(main.contains(".allow_shell_command_configured(\"listFixtures\""));
        assert!(main.contains("resolve_declared_shell_value(\"ls\")"));
        assert!(main.contains("resolve_declared_shell_value(\"${SOURCE_APP_DIR}/fixtures\")"));
        assert!(main.contains(".allow_extra_args(vec![resolve_declared_shell_value(\"--json\")"));
        assert!(main.contains("resolve_declared_shell_value(\"${EXE_DIR}/flags.txt\")"));
        assert!(main.contains(".current_dir(resolve_declared_shell_dir(\"${SOURCE_APP_DIR}\"))"));
        assert!(main.contains(".env(\"LC_ALL\", resolve_declared_shell_value(\"C\"))"));
        assert!(main.contains(".clear_env()"));
        assert!(main.contains(".timeout_ms(2500)"));
        assert!(main.contains(".max_output_bytes(8192)"));
    }

    #[test]
    fn relative_path_renders_portable_path_segments() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("apps/demo/native")).unwrap();
        fs::create_dir_all(root.join("crates/rustframe")).unwrap();

        let relative = relative_path(
            &root.join("apps/demo/native"),
            &root.join("crates/rustframe"),
        )
        .unwrap();

        assert_eq!(relative, "../../../crates/rustframe");
    }

    #[test]
    fn prepare_ejected_runner_writes_portable_native_project() {
        let workspace = tempdir().unwrap();
        fs::create_dir_all(workspace.path().join("crates/rustframe")).unwrap();
        let app_dir = workspace.path().join("apps/ejected-demo");
        fs::create_dir_all(app_dir.join("data/seeds")).unwrap();
        fs::write(app_dir.join("index.html"), "<title>Ejected Demo</title>").unwrap();
        fs::write(app_dir.join("app.js"), "console.log('ok')").unwrap();
        fs::write(app_dir.join("styles.css"), "body {}").unwrap();
        fs::write(app_dir.join("data/schema.json"), "{}").unwrap();
        fs::write(app_dir.join("data/seeds/001.json"), "{}").unwrap();

        let app = AppProject {
            name: "ejected-demo".into(),
            app_dir: app_dir.clone(),
            asset_dir: app_dir.clone(),
            config: AppConfig {
                app_id: "ejected_demo".into(),
                title: "Ejected Demo".into(),
                width: 1280.0,
                height: 820.0,
                dev_url: None,
                security: AppSecurityConfig::local_first(),
                fs_roots: vec!["fixtures".into()],
                shell_commands: vec![AppShellCommand {
                    name: "sync".into(),
                    program: "echo".into(),
                    args: vec!["${SOURCE_APP_DIR}".into()],
                    allowed_args: Vec::new(),
                    cwd: None,
                    env: BTreeMap::new(),
                    clear_env: false,
                    timeout_ms: None,
                    max_output_bytes: None,
                }],
                packaging: default_packaging_config("Ejected Demo"),
            },
        };

        let runner = prepare_ejected_runner(workspace.path(), &app).unwrap();
        let cargo = fs::read_to_string(&runner.manifest_path).unwrap();
        let main =
            fs::read_to_string(runner.manifest_path.parent().unwrap().join("src/main.rs")).unwrap();

        assert!(
            cargo.contains(
                "rust-embed = { version = \"8.11.0\", features = [\"include-exclude\"] }"
            )
        );
        assert!(cargo.contains("../../../crates/rustframe"));
        assert!(main.contains("#[derive(RustEmbed)]"));
        assert!(main.contains("#[folder = \"..\"]"));
        assert!(main.contains("#[exclude = \"native/**\"]"));
        assert!(
            main.contains(".embedded_database(\"data/schema.json\", &[\"data/seeds/001.json\"])")
        );
        assert!(main.contains(".frontend_security(rustframe::FrontendSecurity::local_first())"));
        assert!(main.contains(".allow_fs_root(resolve_declared_fs_root(\"fixtures\"))"));
        assert!(main.contains("PathBuf::from(env!(\"CARGO_MANIFEST_DIR\")).join(\"..\")"));
    }

    #[test]
    fn resolve_runner_project_prefers_ejected_runner_when_present() {
        let workspace = tempdir().unwrap();
        fs::create_dir_all(workspace.path().join("crates/rustframe")).unwrap();
        let app_dir = workspace.path().join("apps/orbit-desk");
        fs::create_dir_all(app_dir.join("native/src")).unwrap();
        fs::write(app_dir.join("index.html"), "<title>Orbit Desk</title>").unwrap();
        fs::write(
            app_dir.join("native/Cargo.toml"),
            "[package]\nname = \"runner\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(app_dir.join("native/src/main.rs"), "fn main() {}").unwrap();

        let app = AppProject {
            name: "orbit-desk".into(),
            app_dir: app_dir.clone(),
            asset_dir: app_dir,
            config: AppConfig {
                app_id: "orbit-desk".into(),
                title: "Orbit Desk".into(),
                width: 1280.0,
                height: 820.0,
                dev_url: None,
                security: AppSecurityConfig::local_first(),
                fs_roots: Vec::new(),
                shell_commands: Vec::new(),
                packaging: default_packaging_config("Orbit Desk"),
            },
        };

        let runner = resolve_runner_project(workspace.path(), &app).unwrap();

        assert_eq!(
            runner.manifest_path,
            workspace.path().join("apps/orbit-desk/native/Cargo.toml")
        );
        assert_eq!(
            runner.target_dir,
            workspace.path().join("target/rustframe/ejected/orbit-desk")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn build_linux_package_writes_bundle_scripts_and_archive() {
        let temp = tempdir().unwrap();
        let app_dir = temp.path().join("apps/package-demo");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("index.html"), "<title>Package Demo</title>").unwrap();
        fs::write(app_dir.join("icon.svg"), "<svg/>").unwrap();
        fs::create_dir_all(app_dir.join("workspace")).unwrap();
        fs::write(app_dir.join("workspace/notes.md"), "# packaged\n").unwrap();
        let binary_dir = temp.path().join("target/release");
        fs::create_dir_all(&binary_dir).unwrap();
        let binary_path = binary_dir.join("package-demo");
        fs::write(&binary_path, "#!/usr/bin/env bash\necho ok\n").unwrap();

        let app = AppProject {
            name: "package-demo".into(),
            app_dir: app_dir.clone(),
            asset_dir: app_dir.clone(),
            config: AppConfig {
                app_id: "package-demo".into(),
                title: "Package Demo".into(),
                width: 1280.0,
                height: 820.0,
                dev_url: None,
                security: AppSecurityConfig::local_first(),
                fs_roots: vec!["workspace".into()],
                shell_commands: Vec::new(),
                packaging: AppPackagingConfig {
                    version: "2.4.0".into(),
                    description: "A Linux packaged app".into(),
                    publisher: Some("RustFrame".into()),
                    homepage: Some("https://example.com/package-demo".into()),
                    linux: LinuxPackagingConfig {
                        categories: vec!["Utility".into()],
                        keywords: vec!["package".into(), "demo".into()],
                        icon_path: Some(app_dir.join("icon.svg")),
                    },
                    windows: WindowsPackagingConfig { icon_path: None },
                    macos: MacOsPackagingConfig {
                        bundle_identifier: "dev.rustframe.package-demo".into(),
                        icon_path: None,
                    },
                },
            },
        };

        let output = build_linux_package(&app, &binary_path).unwrap();
        let install_script = fs::read_to_string(output.bundle_dir.join("install.sh")).unwrap();

        assert!(output.bundle_dir.join("install.sh").exists());
        assert!(output.bundle_dir.join("uninstall.sh").exists());
        assert!(output.bundle_dir.join("README.txt").exists());
        assert!(output.bundle_dir.join("rustframe-package.json").exists());
        assert!(output.app_dir.join("AppRun").exists());
        assert!(output.app_dir.join("usr/bin/package-demo").exists());
        assert!(output.app_dir.join("usr/bin/workspace/notes.md").exists());
        assert!(
            output
                .app_dir
                .join("usr/share/applications/package-demo.desktop")
                .exists()
        );
        assert!(
            output
                .app_dir
                .join("usr/share/icons/hicolor/scalable/apps/package-demo.svg")
                .exists()
        );
        assert!(output.archive_path.exists());
        assert!(install_script.contains("Icon=$install_root/${appdir_name}/usr/share/icons"));
        assert!(install_script.contains("Keywords=package;demo;"));
    }

    #[test]
    fn sync_declared_fs_roots_copies_relative_and_exe_dir_roots() {
        let temp = tempdir().unwrap();
        let app_dir = temp.path().join("apps/research-desk");
        fs::create_dir_all(app_dir.join("workspace/reports")).unwrap();
        fs::create_dir_all(app_dir.join("exports")).unwrap();
        fs::write(app_dir.join("workspace/reports/day-01.md"), "alpha").unwrap();
        fs::write(app_dir.join("exports/layout.json"), "{\"ok\":true}").unwrap();

        let app = AppProject {
            name: "research-desk".into(),
            app_dir: app_dir.clone(),
            asset_dir: app_dir.clone(),
            config: AppConfig {
                app_id: "research-desk".into(),
                title: "Research Desk".into(),
                width: 1280.0,
                height: 820.0,
                dev_url: None,
                security: AppSecurityConfig::local_first(),
                fs_roots: vec!["workspace".into(), "${EXE_DIR}/exports".into()],
                shell_commands: Vec::new(),
                packaging: AppPackagingConfig {
                    version: "0.1.0".into(),
                    description: "Research desk".into(),
                    publisher: None,
                    homepage: None,
                    linux: LinuxPackagingConfig {
                        categories: vec!["Utility".into()],
                        keywords: Vec::new(),
                        icon_path: None,
                    },
                    windows: WindowsPackagingConfig { icon_path: None },
                    macos: MacOsPackagingConfig {
                        bundle_identifier: "dev.rustframe.research-desk".into(),
                        icon_path: None,
                    },
                },
            },
        };

        let executable_dir = temp.path().join("dist");
        sync_declared_fs_roots(&app, &executable_dir).unwrap();

        assert_eq!(
            fs::read_to_string(executable_dir.join("workspace/reports/day-01.md")).unwrap(),
            "alpha"
        );
        assert_eq!(
            fs::read_to_string(executable_dir.join("exports/layout.json")).unwrap(),
            "{\"ok\":true}"
        );
    }

    #[test]
    fn build_windows_package_writes_portable_bundle_and_zip() {
        let temp = tempdir().unwrap();
        let app_dir = temp.path().join("apps/package-demo");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("index.html"), "<title>Package Demo</title>").unwrap();
        fs::write(app_dir.join("icon.ico"), "icon").unwrap();
        let binary_dir = temp.path().join("target/release");
        fs::create_dir_all(&binary_dir).unwrap();
        let binary_path = binary_dir.join("package-demo.exe");
        fs::write(&binary_path, "binary").unwrap();

        let app = AppProject {
            name: "package-demo".into(),
            app_dir: app_dir.clone(),
            asset_dir: app_dir.clone(),
            config: AppConfig {
                app_id: "package-demo".into(),
                title: "Package Demo".into(),
                width: 1280.0,
                height: 820.0,
                dev_url: None,
                security: AppSecurityConfig::local_first(),
                fs_roots: Vec::new(),
                shell_commands: Vec::new(),
                packaging: AppPackagingConfig {
                    version: "2.4.0".into(),
                    description: "A Windows packaged app".into(),
                    publisher: Some("RustFrame".into()),
                    homepage: Some("https://example.com/package-demo".into()),
                    linux: LinuxPackagingConfig {
                        categories: vec!["Utility".into()],
                        keywords: vec!["package".into(), "demo".into()],
                        icon_path: None,
                    },
                    windows: WindowsPackagingConfig {
                        icon_path: Some(app_dir.join("icon.ico")),
                    },
                    macos: MacOsPackagingConfig {
                        bundle_identifier: "dev.rustframe.package-demo".into(),
                        icon_path: None,
                    },
                },
            },
        };

        let output = build_windows_package(&app, &binary_path).unwrap();
        let install_script = fs::read_to_string(output.bundle_dir.join("install.ps1")).unwrap();

        assert!(output.bundle_dir.join("install.ps1").exists());
        assert!(output.bundle_dir.join("uninstall.ps1").exists());
        assert!(output.bundle_dir.join("README.txt").exists());
        assert!(output.bundle_dir.join("rustframe-package.json").exists());
        assert!(output.portable_dir.join("package-demo.exe").exists());
        assert!(output.portable_dir.join("package-demo.ico").exists());
        assert!(output.archive_path.exists());
        assert!(install_script.contains("WScript.Shell"));
        assert!(install_script.contains("package-demo.exe"));
    }

    #[test]
    fn build_macos_package_writes_app_bundle_and_archive() {
        let temp = tempdir().unwrap();
        let app_dir = temp.path().join("apps/package-demo");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("index.html"), "<title>Package Demo</title>").unwrap();
        fs::write(app_dir.join("icon.icns"), "icon").unwrap();
        let binary_dir = temp.path().join("target/release");
        fs::create_dir_all(&binary_dir).unwrap();
        let binary_path = binary_dir.join("package-demo");
        fs::write(&binary_path, "#!/usr/bin/env bash\necho ok\n").unwrap();

        let app = AppProject {
            name: "package-demo".into(),
            app_dir: app_dir.clone(),
            asset_dir: app_dir.clone(),
            config: AppConfig {
                app_id: "package-demo".into(),
                title: "Package Demo".into(),
                width: 1280.0,
                height: 820.0,
                dev_url: None,
                security: AppSecurityConfig::local_first(),
                fs_roots: Vec::new(),
                shell_commands: Vec::new(),
                packaging: AppPackagingConfig {
                    version: "2.4.0".into(),
                    description: "A macOS packaged app".into(),
                    publisher: Some("RustFrame".into()),
                    homepage: Some("https://example.com/package-demo".into()),
                    linux: LinuxPackagingConfig {
                        categories: vec!["Utility".into()],
                        keywords: vec!["package".into(), "demo".into()],
                        icon_path: None,
                    },
                    windows: WindowsPackagingConfig { icon_path: None },
                    macos: MacOsPackagingConfig {
                        bundle_identifier: "dev.rustframe.package-demo".into(),
                        icon_path: Some(app_dir.join("icon.icns")),
                    },
                },
            },
        };

        let output = build_macos_package(&app, &binary_path).unwrap();
        let info_plist = fs::read_to_string(output.app_bundle.join("Contents/Info.plist")).unwrap();

        assert!(output.bundle_dir.join("install.sh").exists());
        assert!(output.bundle_dir.join("uninstall.sh").exists());
        assert!(output.bundle_dir.join("README.txt").exists());
        assert!(output.bundle_dir.join("rustframe-package.json").exists());
        assert!(
            output
                .app_bundle
                .join("Contents/MacOS/package-demo")
                .exists()
        );
        assert!(
            output
                .app_bundle
                .join("Contents/Resources/package-demo.icns")
                .exists()
        );
        assert!(output.archive_path.exists());
        assert!(info_plist.contains("dev.rustframe.package-demo"));
        assert!(info_plist.contains("CFBundleIconFile"));
    }
}
