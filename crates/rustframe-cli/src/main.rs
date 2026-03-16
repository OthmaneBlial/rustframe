use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::Deserialize;

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
const TEMPLATE_BRIDGE_JS: &str = include_str!("../templates/frontend/bridge.js");
const TEMPLATE_MANIFEST_JSON: &str = include_str!("../templates/frontend/rustframe.json");

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
    fs_roots: Vec<String>,
    shell_commands: Vec<AppShellCommand>,
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
    filesystem: Option<ManifestFilesystem>,
    #[serde(default)]
    shell: Option<ManifestShell>,
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
    write_text_file(&app_dir.join("bridge.js"), TEMPLATE_BRIDGE_JS)?;
    write_text_file(
        &app_dir.join("rustframe.json"),
        &render_template(
            TEMPLATE_MANIFEST_JSON,
            &[("{{app_name}}", name.to_string())],
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
    println!("  {}/bridge.js", app_dir.display());
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

    let binary_name = executable_name(name);
    let source = runner.target_dir.join("release").join(&binary_name);
    if !source.exists() {
        return Err(format!(
            "expected release binary was not produced: {}",
            source.display()
        ));
    }

    let dist_dir = app.app_dir.join("dist");
    fs::create_dir_all(&dist_dir).map_err(|error| {
        format!(
            "failed to create dist directory '{}': {error}",
            dist_dir.display()
        )
    })?;

    let destination = dist_dir.join(&binary_name);
    fs::copy(&source, &destination).map_err(|error| {
        format!(
            "failed to copy '{}' to '{}': {error}",
            source.display(),
            destination.display()
        )
    })?;

    let permissions = fs::metadata(&source)
        .map_err(|error| format!("failed to read '{}': {error}", source.display()))?
        .permissions();
    fs::set_permissions(&destination, permissions).map_err(|error| {
        format!(
            "failed to preserve permissions for '{}': {error}",
            destination.display()
        )
    })?;

    let size = fs::metadata(&destination)
        .map_err(|error| format!("failed to stat '{}': {error}", destination.display()))?
        .len();

    println!("Exported {}", destination.display());
    println!("Size: {}", format_size(size));
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
    let window = manifest.window.as_ref();

    let title = window
        .and_then(|window| window.title.clone())
        .or_else(|| extract_title(&html))
        .unwrap_or_else(|| humanize_name(name));
    let width = if let Some(value) = window.and_then(|window| window.width) {
        validate_dimension("window.width", value)?
    } else {
        extract_meta_content(&html, "rustframe:width")
            .map(|value| parse_dimension("rustframe:width", &value))
            .transpose()?
            .unwrap_or(1280.0)
    };
    let height = if let Some(value) = window.and_then(|window| window.height) {
        validate_dimension("window.height", value)?
    } else {
        extract_meta_content(&html, "rustframe:height")
            .map(|value| parse_dimension("rustframe:height", &value))
            .transpose()?
            .unwrap_or(820.0)
    };
    let dev_url = manifest
        .dev_url
        .or_else(|| extract_meta_content(&html, "rustframe:dev-url"));
    let app_id = manifest.app_id.unwrap_or_else(|| name.to_string());
    validate_app_id(&app_id)?;

    let fs_roots = manifest.filesystem.unwrap_or_default().roots;
    validate_fs_roots(&fs_roots)?;
    let shell_commands = manifest
        .shell
        .unwrap_or_default()
        .commands
        .into_iter()
        .map(|command| AppShellCommand {
            name: command.name,
            program: command.program,
            args: command.args,
        })
        .collect::<Vec<_>>();
    validate_shell_commands(&shell_commands)?;

    Ok(AppConfig {
        app_id,
        title,
        width,
        height,
        dev_url,
        fs_roots,
        shell_commands,
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

    format!(
        "\n        .embedded_database({}, &[{}])",
        quoted_literal("data/schema.json"),
        seed_paths.join(", ")
    )
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

            format!(
                "\n        .allow_shell_command({}, resolve_declared_shell_value({}), {args})",
                quoted_literal(&command.name),
                quoted_literal(&command.program),
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
        "  rustframe-cli eject [name]            Materialize an app-owned Rust runner in apps/<name>/native/"
    );
    println!();
    println!("Run `dev` and `export` from inside apps/<name>/ to omit the app name.");
    println!("Window title and size are read from index.html:");
    println!("  <title>My App</title>");
    println!("  <meta name=\"rustframe:width\" content=\"1280\">");
    println!("  <meta name=\"rustframe:height\" content=\"820\">");
    println!("Optional native capabilities live in apps/<name>/rustframe.json.");
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::tempdir;

    use super::{
        AppConfig, AppProject, AppShellCommand, collect_embedded_assets, find_workspace_root_from,
        load_app_project, prepare_ejected_runner, prepare_generated_runner, read_app_config,
        relative_path, render_asset_match_arms, render_database_chain, render_template,
        resolve_current_app_name_from, resolve_runner_project,
    };

    fn write_workspace_manifest(root: &Path) {
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = []\n# crates/rustframe\n",
        )
        .unwrap();
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
              "filesystem": {
                "roots": ["fixtures", "${EXE_DIR}/imports"]
              },
              "shell": {
                "commands": [
                  {
                    "name": "listFixtures",
                    "program": "ls",
                    "args": ["-la", "${SOURCE_APP_DIR}/fixtures"]
                  }
                ]
              }
            }
            "#,
        )
        .unwrap();

        let config = read_app_config("manifest-demo", temp.path(), temp.path()).unwrap();

        assert_eq!(config.app_id, "manifest_demo");
        assert_eq!(config.fs_roots, vec!["fixtures", "${EXE_DIR}/imports"]);
        assert_eq!(config.shell_commands.len(), 1);
        assert_eq!(config.shell_commands[0].name, "listFixtures");
        assert_eq!(config.shell_commands[0].program, "ls");
        assert_eq!(
            config.shell_commands[0].args,
            vec!["-la", "${SOURCE_APP_DIR}/fixtures"]
        );
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
        fs::write(app_dir.join("bridge.js"), "window.RustFrame = {}").unwrap();
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
                fs_roots: Vec::new(),
                shell_commands: Vec::new(),
            },
        };

        let runner = prepare_generated_runner(workspace.path(), &app).unwrap();
        let main =
            fs::read_to_string(runner.manifest_path.parent().unwrap().join("src/main.rs")).unwrap();

        assert!(main.contains(".app_id(\"orbit-desk\")"));
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
        fs::write(app_dir.join("bridge.js"), "window.RustFrame = {}").unwrap();
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
                fs_roots: Vec::new(),
                shell_commands: Vec::new(),
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
        fs::write(app_dir.join("bridge.js"), "window.RustFrame = {}").unwrap();
        fs::write(app_dir.join("styles.css"), "body {}").unwrap();

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
                fs_roots: vec!["fixtures".into(), "${EXE_DIR}/imports".into()],
                shell_commands: vec![AppShellCommand {
                    name: "listFixtures".into(),
                    program: "ls".into(),
                    args: vec!["-la".into(), "${SOURCE_APP_DIR}/fixtures".into()],
                }],
            },
        };

        let runner = prepare_generated_runner(workspace.path(), &app).unwrap();
        let main =
            fs::read_to_string(runner.manifest_path.parent().unwrap().join("src/main.rs")).unwrap();

        assert!(main.contains("fn resolve_declared_fs_root"));
        assert!(main.contains(".allow_fs_root(resolve_declared_fs_root(\"fixtures\"))"));
        assert!(main.contains("${SOURCE_APP_DIR}"));
        assert!(main.contains(".allow_shell_command(\"listFixtures\""));
        assert!(main.contains("resolve_declared_shell_value(\"ls\")"));
        assert!(main.contains("resolve_declared_shell_value(\"${SOURCE_APP_DIR}/fixtures\")"));
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
        fs::write(app_dir.join("bridge.js"), "window.RustFrame = {}").unwrap();
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
                fs_roots: vec!["fixtures".into()],
                shell_commands: vec![AppShellCommand {
                    name: "sync".into(),
                    program: "echo".into(),
                    args: vec!["${SOURCE_APP_DIR}".into()],
                }],
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
                fs_roots: Vec::new(),
                shell_commands: Vec::new(),
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
}
