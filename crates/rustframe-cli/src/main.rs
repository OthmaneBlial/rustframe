use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, write::GzEncoder};
use serde::Deserialize;
use serde_json::json;
use tar::Builder as TarBuilder;

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
}

#[derive(Debug, Clone)]
struct LinuxPackagingConfig {
    categories: Vec<String>,
    keywords: Vec<String>,
    icon_path: Option<PathBuf>,
}

#[derive(Debug)]
struct LinuxPackageOutput {
    bundle_dir: PathBuf,
    app_dir: PathBuf,
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

    let size = fs::metadata(&destination)
        .map_err(|error| format!("failed to stat '{}': {error}", destination.display()))?
        .len();

    println!("Exported {}", destination.display());
    println!("Size: {}", format_size(size));
    Ok(())
}

fn command_package(workspace: &Path, name: &str) -> CliResult<()> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = workspace;
        let _ = name;
        return Err("`package` currently supports Linux hosts only".into());
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

fn parse_eject_args(workspace: &Path, args: &[String]) -> CliResult<String> {
    match args {
        [] => resolve_current_app_name(workspace),
        [name, ..] => Ok(name.clone()),
    }
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
    let packaging = read_packaging_config(app_dir, &title, manifest.packaging)?;

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
    title: &str,
    manifest: Option<ManifestPackaging>,
) -> CliResult<AppPackagingConfig> {
    let manifest = manifest.unwrap_or_default();
    let linux = manifest.linux.unwrap_or_default();
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

    validate_packaging_metadata(&version, &description, &categories, &keywords)?;
    if let Some(path) = &icon_path {
        validate_packaging_icon(path)?;
    }

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

fn validate_packaging_icon(path: &Path) -> CliResult<()> {
    if !path.exists() {
        return Err(format!(
            "packaging.linux.icon points to a missing file: {}",
            path.display()
        ));
    }

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

    if !matches!(extension.as_str(), "svg" | "png") {
        return Err(format!(
            "packaging.linux.icon must end with .svg or .png: {}",
            path.display()
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

    let binary_name = executable_name(&app.name);
    let installed_binary = usr_bin.join(&binary_name);
    copy_with_permissions(source_binary, &installed_binary)?;

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

#[cfg(target_os = "linux")]
fn make_executable(path: &Path) -> CliResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to update '{}': {error}", path.display()))
}

#[cfg(not(target_os = "linux"))]
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
        "  rustframe-cli package [name]          Build a Linux bundle and tarball into apps/<name>/dist/linux/"
    );
    println!(
        "  rustframe-cli eject [name]            Materialize an app-owned Rust runner in apps/<name>/native/"
    );
    println!();
    println!("Run `dev`, `export`, and `package` from inside apps/<name>/ to omit the app name.");
    println!("Primary app config lives in apps/<name>/rustframe.json:");
    println!("  \"window\": {{ \"title\": \"My App\", \"width\": 1280, \"height\": 820 }}");
    println!("  \"devUrl\": \"http://127.0.0.1:5173\"");
    println!("HTML <title> and rustframe:* meta tags still work as fallback.");
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, path::Path};

    use tempfile::tempdir;

    use super::{
        AppConfig, AppPackagingConfig, AppProject, AppSecurityConfig, AppSecurityModel,
        AppShellCommand, LinuxPackagingConfig, build_linux_package, collect_embedded_assets,
        find_workspace_root_from, load_app_project, prepare_ejected_runner,
        prepare_generated_runner, read_app_config, relative_path, render_asset_match_arms,
        render_database_chain, render_template, resolve_current_app_name_from,
        resolve_runner_project,
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
                    description: "A Linux packaged app".into(),
                    publisher: Some("RustFrame".into()),
                    homepage: Some("https://example.com/package-demo".into()),
                    linux: LinuxPackagingConfig {
                        categories: vec!["Utility".into()],
                        keywords: vec!["package".into(), "demo".into()],
                        icon_path: Some(app_dir.join("icon.svg")),
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
}
