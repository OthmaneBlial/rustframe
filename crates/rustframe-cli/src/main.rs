use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

type CliResult<T> = Result<T, String>;

const TEMPLATE_RUNNER_CARGO_TOML: &str =
    include_str!("../templates/generated-runner/Cargo.toml.tmpl");
const TEMPLATE_RUNNER_MAIN_RS: &str = include_str!("../templates/generated-runner/main.rs.tmpl");
const TEMPLATE_DATA_SCHEMA: &str = include_str!("../templates/data/schema.json");
const TEMPLATE_DATA_SEED: &str = include_str!("../templates/data/seeds/001-welcome.json");
const TEMPLATE_INDEX_HTML: &str = include_str!("../templates/frontend/index.html");
const TEMPLATE_STYLES_CSS: &str = include_str!("../templates/frontend/styles.css");
const TEMPLATE_APP_JS: &str = include_str!("../templates/frontend/app.js");
const TEMPLATE_BRIDGE_JS: &str = include_str!("../templates/frontend/bridge.js");

#[derive(Debug)]
struct AppProject {
    name: String,
    app_dir: PathBuf,
    asset_dir: PathBuf,
    config: AppConfig,
}

#[derive(Debug)]
struct AppConfig {
    title: String,
    width: f64,
    height: f64,
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
    println!("Run it with: cargo run -p rustframe-cli -- dev {name}");
    Ok(())
}

fn command_dev(workspace: &Path, name: &str, dev_url: Option<String>) -> CliResult<()> {
    let app = load_app_project(workspace, name)?;
    let runner = prepare_runner(workspace, &app)?;

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
    let runner = prepare_runner(workspace, &app)?;

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

fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn resolve_current_app_name(workspace: &Path) -> CliResult<String> {
    let current_dir = env::current_dir()
        .and_then(fs::canonicalize)
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
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

    let config = read_app_config(name, &asset_dir)?;

    Ok(AppProject {
        name: name.to_string(),
        app_dir,
        asset_dir,
        config,
    })
}

fn read_app_config(name: &str, asset_dir: &Path) -> CliResult<AppConfig> {
    let index_path = asset_dir.join("index.html");
    let html = fs::read_to_string(&index_path)
        .map_err(|error| format!("failed to read '{}': {error}", index_path.display()))?;

    let title = extract_title(&html).unwrap_or_else(|| humanize_name(name));
    let width = extract_meta_content(&html, "rustframe:width")
        .map(|value| parse_dimension("rustframe:width", &value))
        .transpose()?
        .unwrap_or(1280.0);
    let height = extract_meta_content(&html, "rustframe:height")
        .map(|value| parse_dimension("rustframe:height", &value))
        .transpose()?
        .unwrap_or(820.0);
    let dev_url = extract_meta_content(&html, "rustframe:dev-url");

    Ok(AppConfig {
        title,
        width,
        height,
        dev_url,
    })
}

fn prepare_runner(workspace: &Path, app: &AppProject) -> CliResult<RunnerProject> {
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
    let app_id_chain = format!("\n        .app_id({})", quoted_literal(&app.name));
    let database_chain = render_database_chain(&assets);

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
            ("{{window_title}}", quoted_literal(&app.config.title)),
            ("{{window_width}}", format_float(app.config.width)),
            ("{{window_height}}", format_float(app.config.height)),
            ("{{app_id_chain}}", app_id_chain),
            ("{{dev_url_chain}}", dev_url_chain),
            ("{{database_chain}}", database_chain),
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

    if parsed > 0.0 {
        Ok(parsed)
    } else {
        Err(format!("{field} must be greater than zero"))
    }
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
    println!();
    println!("Run `dev` and `export` from inside apps/<name>/ to omit the app name.");
    println!("Window title and size are read from index.html:");
    println!("  <title>My App</title>");
    println!("  <meta name=\"rustframe:width\" content=\"1280\">");
    println!("  <meta name=\"rustframe:height\" content=\"820\">");
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        AppConfig, AppProject, collect_embedded_assets, prepare_runner, read_app_config,
        render_asset_match_arms, render_database_chain, render_template,
    };

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

        let config = read_app_config("orbit-desk", temp.path()).unwrap();

        assert_eq!(config.title, "Orbit Desk");
        assert_eq!(config.width, 1440.0);
        assert_eq!(config.height, 920.0);
        assert_eq!(config.dev_url.as_deref(), Some("http://127.0.0.1:5173"));
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
    fn prepare_runner_writes_database_enabled_runner() {
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
                title: "Orbit Desk".into(),
                width: 1440.0,
                height: 920.0,
                dev_url: None,
            },
        };

        let runner = prepare_runner(workspace.path(), &app).unwrap();
        let main =
            fs::read_to_string(runner.manifest_path.parent().unwrap().join("src/main.rs")).unwrap();

        assert!(main.contains(".app_id(\"orbit-desk\")"));
        assert!(
            main.contains(".embedded_database(\"data/schema.json\", &[\"data/seeds/001.json\"])")
        );
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
}
