use std::{fs, path::Path};

use serde_json::{Map, Value, json};

use crate::manifest::{LEGACY_SCHEMA_URL, SCHEMA_URL};

pub fn migrate_project(project: &Path, dry_run: bool) -> Result<(), String> {
    let manifest_path = project.join("rustframe.json");
    let source = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read '{}': {error}", manifest_path.display()))?;
    let old: Value = serde_json::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", manifest_path.display()))?;
    if old.get("schemaVersion").and_then(Value::as_u64) == Some(1)
        && old.get("$schema").and_then(Value::as_str) == Some(LEGACY_SCHEMA_URL)
    {
        let mut migrated = old.clone();
        migrated["$schema"] = Value::String(SCHEMA_URL.into());
        println!(
            "Updating retired manifest schema URL in {}",
            manifest_path.display()
        );
        if dry_run {
            println!("Dry run: no files changed");
            return Ok(());
        }
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&migrated).unwrap() + "\n",
        )
        .map_err(|error| format!("failed to update manifest schema URL: {error}"))?;
        println!("Updated manifest schema URL");
        return Ok(());
    }
    if old.get("schemaVersion").and_then(Value::as_u64) == Some(1) {
        println!(
            "{} already uses manifest schema v1",
            manifest_path.display()
        );
        return Ok(());
    }

    let app_id = old.get("appId").and_then(Value::as_str).unwrap_or_else(|| {
        project
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("rustframe-app")
    });
    let window = old.get("window").and_then(Value::as_object);
    let title = window
        .and_then(|value| value.get("title"))
        .and_then(Value::as_str)
        .unwrap_or(app_id);
    let width = window
        .and_then(|value| value.get("width"))
        .and_then(Value::as_f64)
        .unwrap_or(1280.0);
    let height = window
        .and_then(|value| value.get("height"))
        .and_then(Value::as_f64)
        .unwrap_or(820.0);
    let model = old
        .pointer("/security/model")
        .and_then(Value::as_str)
        .unwrap_or("local-first");
    let dev_url = old
        .get("devUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:5173");
    let roots = old
        .pointer("/filesystem/roots")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let shell = migrate_shell(old.get("shell"));
    let packaging = migrate_packaging(app_id, title, old.get("packaging"));
    let permissions = if model == "networked" {
        json!([{ "window": "main", "allow": ["dialog:open", "dialog:save"] }])
    } else {
        json!([{
            "window": "main",
            "allow": [
                "db:read", "db:write", "db:backup", "db:restore",
                "fs:workspace:read", "fs:workspace:write", "fs:workspace:watch",
                "fs:grants:read", "fs:grants:write", "fs:grants:watch",
                "dialog:open", "dialog:save", "window:create"
            ]
        }])
    };

    let migrated = json!({
        "$schema": SCHEMA_URL,
        "schemaVersion": 1,
        "app": {
            "id": app_id,
            "title": title,
            "windows": [{ "id": "main", "title": title, "route": "/", "width": width, "height": height }]
        },
        "frontend": {
            "devCommand": "npm run dev -- --host 127.0.0.1",
            "buildCommand": "npm run build",
            "devUrl": dev_url,
            "distDir": "dist",
            "generatedTypes": "src/rustframe.generated.ts"
        },
        "security": {
            "model": model,
            "csp": "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self' http://127.0.0.1:* ws://127.0.0.1:*; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
            "permissions": permissions
        },
        "database": { "schema": "data/schema.json", "seeds": "data/seeds", "migrations": "data/migrations" },
        "filesystem": { "roots": roots, "persistGrants": true },
        "shell": shell,
        "packaging": packaging
    });

    let calls = scan_bridge_calls(project)?;
    println!("RustFrame pre-v1 migration for {}", project.display());
    println!("  manifest: schema v1");
    println!("  frontend: Vite commands and generated types contract");
    println!("  runner: registry rustframe-runtime dependency when regenerated");
    if !calls.is_empty() {
        println!("  manual bridge review required: {}", calls.join(", "));
    }
    if dry_run {
        println!("Dry run: no files changed");
        return Ok(());
    }

    fs::write(project.join("rustframe.pre-v1.json"), &source)
        .map_err(|error| format!("failed to write manifest backup: {error}"))?;
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&migrated).expect("manifest JSON") + "\n",
    )
    .map_err(|error| format!("failed to write '{}': {error}", manifest_path.display()))?;
    ensure_package_json(project, app_id)?;
    ensure_migration_dirs(project)?;
    migrate_native_dependency(project)?;
    println!(
        "Migrated {} (backup: rustframe.pre-v1.json)",
        project.display()
    );
    Ok(())
}

fn migrate_shell(shell: Option<&Value>) -> Value {
    let mut shell = shell.cloned().unwrap_or_else(|| json!({ "commands": [] }));
    if let Some(commands) = shell.get_mut("commands").and_then(Value::as_array_mut) {
        for command in commands {
            if let Some(object) = command.as_object_mut() {
                if !object.contains_key("id") {
                    if let Some(name) = object.remove("name") {
                        object.insert("id".into(), name);
                    }
                }
            }
        }
    }
    shell
}

fn migrate_packaging(app_id: &str, title: &str, packaging: Option<&Value>) -> Value {
    let old = packaging.and_then(Value::as_object);
    let mut output = Map::new();
    output.insert(
        "version".into(),
        old.and_then(|value| value.get("version"))
            .cloned()
            .unwrap_or_else(|| json!("0.1.0")),
    );
    output.insert(
        "identifier".into(),
        old.and_then(|value| value.get("macos"))
            .and_then(Value::as_object)
            .and_then(|value| value.get("bundleIdentifier"))
            .cloned()
            .unwrap_or_else(|| json!(format!("dev.rustframe.{app_id}"))),
    );
    output.insert(
        "description".into(),
        old.and_then(|value| value.get("description"))
            .cloned()
            .unwrap_or_else(|| json!(title)),
    );
    output.insert(
        "icon".into(),
        old.and_then(|value| value.get("linux"))
            .and_then(Value::as_object)
            .and_then(|value| value.get("icon"))
            .cloned()
            .unwrap_or_else(|| json!("assets/icon.svg")),
    );
    output.insert(
        "linux".into(),
        old.and_then(|value| value.get("linux"))
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    output.insert(
        "windows".into(),
        old.and_then(|value| value.get("windows"))
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    output.insert(
        "macos".into(),
        old.and_then(|value| value.get("macos"))
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    Value::Object(output)
}

fn scan_bridge_calls(project: &Path) -> Result<Vec<String>, String> {
    let mut calls = Vec::new();
    scan_bridge_sources(project, project, &mut calls)?;
    calls.sort();
    calls.dedup();
    Ok(calls)
}

fn scan_bridge_sources(
    root: &Path,
    directory: &Path,
    calls: &mut Vec<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("failed to scan '{}': {error}", directory.display()))?
    {
        let path = entry
            .map_err(|error| format!("failed to scan project: {error}"))?
            .path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if name.starts_with('.') || matches!(name, "node_modules" | "dist" | "target") {
                continue;
            }
            scan_bridge_sources(root, &path, calls)?;
            continue;
        }
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|extension| !matches!(extension, "js" | "ts" | "jsx" | "tsx"))
        {
            continue;
        }
        let source = fs::read_to_string(&path).map_err(|error| {
            format!(
                "failed to read frontend source '{}' while scanning bridge calls: {error}",
                path.strip_prefix(root).unwrap_or(&path).display()
            )
        })?;
        for namespace in ["db", "fs", "shell", "dialog", "window"] {
            if source.contains(&format!("RustFrame.{namespace}.")) {
                calls.push(namespace.to_string());
            }
        }
    }
    Ok(())
}

fn ensure_package_json(project: &Path, app_id: &str) -> Result<(), String> {
    let path = project.join("package.json");
    if path.exists() {
        return Ok(());
    }
    let api_dependency = format!("={}", env!("CARGO_PKG_VERSION"));
    let package = json!({
        "name": app_id,
        "private": true,
        "version": "0.1.0",
        "type": "module",
        "scripts": { "dev": "vite", "build": "vite build" },
        "dependencies": { "rustframe-api": api_dependency, "typescript": "^5.9.0", "vite": "^7.0.0" }
    });
    fs::write(path, serde_json::to_string_pretty(&package).unwrap() + "\n")
        .map_err(|error| format!("failed to create package.json: {error}"))
}

fn ensure_migration_dirs(project: &Path) -> Result<(), String> {
    for path in ["data/seeds", "data/migrations", "src"] {
        fs::create_dir_all(project.join(path))
            .map_err(|error| format!("failed to create {path}: {error}"))?;
    }
    Ok(())
}

fn migrate_native_dependency(project: &Path) -> Result<(), String> {
    let path = project.join("native/Cargo.toml");
    let Ok(source) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let mut changed = Vec::new();
    for line in source.lines() {
        if line.trim_start().starts_with("rustframe =") {
            changed.push(format!(
                "rustframe = {{ package = \"rustframe-runtime\", version = \"={}\" }}",
                env!("CARGO_PKG_VERSION")
            ));
        } else {
            changed.push(line.to_string());
        }
    }
    fs::write(&path, changed.join("\n") + "\n")
        .map_err(|error| format!("failed to update '{}': {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migrate_replaces_the_retired_schema_url() {
        let directory = tempdir().unwrap();
        let manifest = json!({
            "$schema": LEGACY_SCHEMA_URL,
            "schemaVersion": 1
        });
        fs::write(
            directory.path().join("rustframe.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        migrate_project(directory.path(), false).unwrap();

        let migrated: Value = serde_json::from_str(
            &fs::read_to_string(directory.path().join("rustframe.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(migrated["$schema"], SCHEMA_URL);
    }

    #[test]
    fn dry_run_preserves_the_retired_schema_url() {
        let directory = tempdir().unwrap();
        let manifest = json!({
            "$schema": LEGACY_SCHEMA_URL,
            "schemaVersion": 1
        });
        fs::write(
            directory.path().join("rustframe.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        migrate_project(directory.path(), true).unwrap();

        let unchanged: Value = serde_json::from_str(
            &fs::read_to_string(directory.path().join("rustframe.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(unchanged["$schema"], LEGACY_SCHEMA_URL);
    }
}
