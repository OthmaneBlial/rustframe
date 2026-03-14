use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

type CliResult<T> = Result<T, String>;

const TEMPLATE_CARGO_TOML: &str = include_str!("../templates/Cargo.toml.tmpl");
const TEMPLATE_MAIN_RS: &str = include_str!("../templates/src_main.rs.tmpl");
const TEMPLATE_MANIFEST: &str = include_str!("../templates/rustframe.toml.tmpl");
const TEMPLATE_GITIGNORE: &str = include_str!("../templates/gitignore.tmpl");
const TEMPLATE_INDEX_HTML: &str = include_str!("../templates/frontend/index.html");
const TEMPLATE_STYLES_CSS: &str = include_str!("../templates/frontend/styles.css");
const TEMPLATE_APP_JS: &str = include_str!("../templates/frontend/app.js");
const TEMPLATE_BRIDGE_JS: &str = include_str!("../templates/frontend/bridge.js");

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
            let name = args.get(1).ok_or_else(|| {
                "missing app name: rustframe-cli dev <name> [dev-url]".to_string()
            })?;
            let dev_url = args.get(2).map(String::as_str);
            command_dev(name, dev_url)
        }
        Some("export") => {
            let name = args
                .get(1)
                .ok_or_else(|| "missing app name: rustframe-cli export <name>".to_string())?;
            command_export(name)
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

    write_text_file(
        &app_dir.join("Cargo.toml"),
        &render_template(TEMPLATE_CARGO_TOML, name, &title),
    )?;
    write_text_file(
        &app_dir.join("src/main.rs"),
        &render_template(TEMPLATE_MAIN_RS, name, &title),
    )?;
    write_text_file(
        &app_dir.join("rustframe.toml"),
        &render_template(TEMPLATE_MANIFEST, name, &title),
    )?;
    write_text_file(&app_dir.join(".gitignore"), TEMPLATE_GITIGNORE)?;
    write_text_file(
        &app_dir.join("frontend/index.html"),
        &render_template(TEMPLATE_INDEX_HTML, name, &title),
    )?;
    write_text_file(&app_dir.join("frontend/styles.css"), TEMPLATE_STYLES_CSS)?;
    write_text_file(
        &app_dir.join("frontend/app.js"),
        &render_template(TEMPLATE_APP_JS, name, &title),
    )?;
    write_text_file(&app_dir.join("frontend/bridge.js"), TEMPLATE_BRIDGE_JS)?;
    ensure_workspace_member(&workspace, &format!("apps/{name}"))?;

    println!("Created RustFrame app: {}", app_dir.display());
    println!("Edit the frontend in {}/frontend/", app_dir.display());
    println!("Run it with: cargo run -p rustframe-cli -- dev {name}");
    Ok(())
}

fn command_dev(name: &str, dev_url: Option<&str>) -> CliResult<()> {
    let workspace = find_workspace_root()?;
    ensure_app_exists(&workspace, name)?;

    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("-p")
        .arg(name)
        .current_dir(&workspace)
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

fn command_export(name: &str) -> CliResult<()> {
    let workspace = find_workspace_root()?;
    let app_dir = ensure_app_exists(&workspace, name)?;

    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("-p")
        .arg(name)
        .current_dir(&workspace)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("failed to launch cargo build: {error}"))?;

    if !status.success() {
        return Err(format!("cargo build --release failed with status {status}"));
    }

    let binary_name = executable_name(name);
    let source = workspace.join("target").join("release").join(&binary_name);
    if !source.exists() {
        return Err(format!(
            "expected release binary was not produced: {}",
            source.display()
        ));
    }

    let dist_dir = app_dir.join("dist");
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

fn ensure_app_exists(workspace: &Path, name: &str) -> CliResult<PathBuf> {
    let app_dir = workspace.join("apps").join(name);
    let manifest = app_dir.join("Cargo.toml");
    if manifest.exists() {
        Ok(app_dir)
    } else {
        Err(format!(
            "app '{name}' does not exist at {}",
            app_dir.display()
        ))
    }
}

fn ensure_workspace_member(workspace: &Path, member: &str) -> CliResult<()> {
    let manifest_path = workspace.join("Cargo.toml");
    let contents = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read '{}': {error}", manifest_path.display()))?;

    let Some(line) = contents
        .lines()
        .find(|line| line.trim_start().starts_with("members = ["))
    else {
        return Err(format!(
            "failed to find workspace members in '{}'",
            manifest_path.display()
        ));
    };

    let mut members = parse_members_line(line)?;
    if members.iter().any(|existing| existing == member) {
        return Ok(());
    }

    members.push(member.to_string());
    members.sort();

    let replacement = format!(
        "members = [{}]",
        members
            .iter()
            .map(|entry| format!("\"{entry}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let updated = contents.replacen(line, &replacement, 1);
    fs::write(&manifest_path, updated)
        .map_err(|error| format!("failed to update '{}': {error}", manifest_path.display()))
}

fn parse_members_line(line: &str) -> CliResult<Vec<String>> {
    let start = line
        .find('[')
        .ok_or_else(|| "workspace members line is missing '['".to_string())?;
    let end = line
        .rfind(']')
        .ok_or_else(|| "workspace members line is missing ']'".to_string())?;

    let members = line[start + 1..end]
        .split(',')
        .filter_map(|entry| {
            let trimmed = entry.trim().trim_matches('"');
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect();

    Ok(members)
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

fn render_template(template: &str, name: &str, title: &str) -> String {
    template
        .replace("{{app_name}}", name)
        .replace("{{app_title}}", title)
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
    println!("  rustframe-cli new <name>            Create a new app in apps/<name>");
    println!(
        "  rustframe-cli dev <name> [dev-url]  Run an app, optionally against a frontend dev server"
    );
    println!("  rustframe-cli export <name>         Build a release binary into apps/<name>/dist/");
}
