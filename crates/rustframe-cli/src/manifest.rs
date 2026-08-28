use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path},
};

use serde::Serialize;
use serde_json::Value;

use crate::diagnostics::Diagnostic;

pub const SCHEMA_URL: &str =
    "https://othmaneblial.github.io/rustframe/schemas/v1/rustframe.schema.json";
pub const LEGACY_SCHEMA_URL: &str = "https://rustframe.dev/schemas/v1/rustframe.schema.json";
pub const SCHEMA_SOURCE: &str = include_str!("../schema/rustframe-v1.schema.json");

/// Parses a manifest source without reading from the filesystem.
///
/// This is public so editors and fuzz targets can exercise the same parser as
/// `rustframe validate`.
pub fn parse_manifest_source(source: &str) -> Result<Value, String> {
    serde_json::from_str(source).map_err(|error| error.to_string())
}

const TOP_LEVEL_FIELDS: &[&str] = &[
    "$schema",
    "schemaVersion",
    "app",
    "frontend",
    "security",
    "database",
    "filesystem",
    "shell",
    "packaging",
];

const KNOWN_PERMISSIONS: &[&str] = &[
    "db:read",
    "db:write",
    "db:backup",
    "db:restore",
    "fs:workspace:read",
    "fs:workspace:write",
    "fs:workspace:watch",
    "fs:grants:read",
    "fs:grants:write",
    "fs:grants:watch",
    "shell:index-workspace",
    "dialog:open",
    "dialog:save",
    "window:create",
    "clipboard:read",
    "clipboard:write",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub valid: bool,
    pub schema_version: Option<u64>,
    pub project: String,
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
}

pub fn validate_project(project: &Path) -> Result<ValidationReport, String> {
    let manifest_path = project.join("rustframe.json");
    let source = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read '{}': {error}", manifest_path.display()))?;
    let value = parse_manifest_source(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", manifest_path.display()))?;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    errors.extend(validate_schema_contract(&value)?);

    let Some(object) = value.as_object() else {
        return Err("rustframe.json must contain a JSON object".into());
    };
    let schema_version = object.get("schemaVersion").and_then(Value::as_u64);

    if schema_version != Some(1) {
        errors.push(
            Diagnostic::error(
                "RF1001",
                "manifest schemaVersion must be 1 for the public project contract",
            )
            .with_hint("run `rustframe migrate` for a pre-v1 project"),
        );
    }
    let schema_url = object.get("$schema").and_then(Value::as_str);
    if schema_url != Some(SCHEMA_URL) {
        let mut diagnostic =
            Diagnostic::error("RF1002", format!("manifest $schema must be '{SCHEMA_URL}'"));
        if schema_url == Some(LEGACY_SCHEMA_URL) {
            diagnostic =
                diagnostic.with_hint("run `rustframe migrate` to replace the retired schema URL");
        }
        errors.push(diagnostic);
    }
    for key in object.keys() {
        if !TOP_LEVEL_FIELDS.contains(&key.as_str()) {
            errors.push(Diagnostic::error(
                "RF1003",
                format!("unknown top-level manifest field '{key}'"),
            ));
        }
    }
    for section in [
        "app",
        "frontend",
        "security",
        "database",
        "filesystem",
        "shell",
        "packaging",
    ] {
        if !object.get(section).is_some_and(Value::is_object) {
            errors.push(Diagnostic::error(
                "RF1004",
                format!("manifest section '{section}' must be an object"),
            ));
        }
    }

    validate_app(object.get("app"), &mut errors);
    validate_frontend(project, object.get("frontend"), &mut errors);
    validate_database(project, object.get("database"), &mut errors);
    validate_security(
        object.get("app"),
        object.get("security"),
        object.get("shell"),
        &mut errors,
        &mut warnings,
    );
    validate_packaging(project, object.get("packaging"), &mut errors);
    validate_shell(object.get("shell"), &mut errors);

    let index_path = project.join("index.html");
    match fs::read_to_string(&index_path) {
        Ok(html)
            if !html
                .to_ascii_lowercase()
                .contains("content-security-policy") =>
        {
            errors.push(
                Diagnostic::error(
                    "RF1401",
                    "index.html must declare a restrictive Content Security Policy",
                )
                .with_hint("start with default-src 'self'; object-src 'none'; base-uri 'none'"),
            );
        }
        Err(_) => errors.push(Diagnostic::error(
            "RF1204",
            format!("frontend entry point '{}' is missing", index_path.display()),
        )),
        _ => {}
    }

    Ok(ValidationReport {
        valid: errors.is_empty(),
        schema_version,
        project: project.display().to_string(),
        errors,
        warnings,
    })
}

fn validate_schema_contract(value: &Value) -> Result<Vec<Diagnostic>, String> {
    let schema: Value = serde_json::from_str(SCHEMA_SOURCE)
        .map_err(|error| format!("embedded manifest schema is invalid: {error}"))?;
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| format!("failed to compile embedded manifest schema: {error}"))?;
    Ok(validator
        .iter_errors(value)
        .map(|error| {
            let location = error.instance_path().to_string();
            Diagnostic::error(
                "RF1005",
                format!(
                    "manifest schema violation at {}: {error}",
                    if location.is_empty() { "/" } else { &location }
                ),
            )
        })
        .collect())
}

fn validate_app(value: Option<&Value>, errors: &mut Vec<Diagnostic>) {
    let Some(app) = value.and_then(Value::as_object) else {
        return;
    };
    let id = app.get("id").and_then(Value::as_str).unwrap_or_default();
    if id.is_empty()
        || !id.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        errors.push(Diagnostic::error(
            "RF1101",
            "app.id must contain only lowercase ASCII letters, digits, and hyphens",
        ));
    }

    let mut windows = BTreeSet::new();
    if let Some(values) = app.get("windows").and_then(Value::as_array) {
        for window in values {
            let Some(id) = window.get("id").and_then(Value::as_str) else {
                errors.push(Diagnostic::error(
                    "RF1102",
                    "every app window requires an id",
                ));
                continue;
            };
            if !windows.insert(id.to_string()) {
                errors.push(Diagnostic::error(
                    "RF1103",
                    format!("window id '{id}' is declared more than once"),
                ));
            }
            if let Some(route) = window.get("route").and_then(Value::as_str) {
                let unsafe_route = route.contains("://")
                    || route.starts_with("//")
                    || route.split(['/', '\\']).any(|segment| segment == "..");
                if unsafe_route {
                    errors.push(Diagnostic::error(
                        "RF1104",
                        format!("window '{id}' route must stay inside the bundled frontend"),
                    ));
                }
            }
        }
    }
}

fn validate_frontend(project: &Path, value: Option<&Value>, errors: &mut Vec<Diagnostic>) {
    let Some(frontend) = value.and_then(Value::as_object) else {
        return;
    };
    for field in ["distDir", "generatedTypes"] {
        let Some(path) = frontend.get(field).and_then(Value::as_str) else {
            errors.push(Diagnostic::error(
                "RF1201",
                format!("frontend.{field} is required"),
            ));
            continue;
        };
        if let Err(error) = validate_relative_path(&format!("frontend.{field}"), path) {
            errors.push(Diagnostic::error("RF1202", error));
        }
    }
    for field in ["devCommand", "buildCommand"] {
        if frontend
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            errors.push(Diagnostic::error(
                "RF1203",
                format!("frontend.{field} is required"),
            ));
        }
    }
    if !project.join("package.json").is_file() {
        errors.push(Diagnostic::error("RF1205", "package.json is missing"));
    }
}

fn validate_database(project: &Path, value: Option<&Value>, errors: &mut Vec<Diagnostic>) {
    let Some(database) = value.and_then(Value::as_object) else {
        return;
    };
    let schema = database
        .get("schema")
        .and_then(Value::as_str)
        .unwrap_or("data/schema.json");
    match validate_relative_path("database.schema", schema) {
        Ok(path) if !project.join(&path).is_file() => errors.push(Diagnostic::error(
            "RF1301",
            format!(
                "database schema '{}' does not exist",
                project.join(path).display()
            ),
        )),
        Err(error) => errors.push(Diagnostic::error("RF1302", error)),
        _ => {}
    }
    for field in ["seeds", "migrations"] {
        let Some(path) = database.get(field).and_then(Value::as_str) else {
            continue;
        };
        match validate_relative_path(&format!("database.{field}"), path) {
            Ok(path) if !project.join(&path).is_dir() => errors.push(Diagnostic::error(
                "RF1303",
                format!(
                    "database {field} directory '{}' does not exist",
                    project.join(path).display()
                ),
            )),
            Err(error) => errors.push(Diagnostic::error("RF1302", error)),
            _ => {}
        }
    }
}

fn validate_security(
    app: Option<&Value>,
    value: Option<&Value>,
    shell: Option<&Value>,
    errors: &mut Vec<Diagnostic>,
    warnings: &mut Vec<Diagnostic>,
) {
    let Some(security) = value.and_then(Value::as_object) else {
        return;
    };
    let model = security
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("local-first");
    if !matches!(model, "local-first" | "networked") {
        errors.push(Diagnostic::error(
            "RF1402",
            "security.model must be local-first or networked",
        ));
    }

    let mut declared_windows = BTreeSet::from(["main".to_string()]);
    if let Some(windows) = app
        .and_then(|value| value.get("windows"))
        .and_then(Value::as_array)
    {
        for window in windows {
            if let Some(id) = window.get("id").and_then(Value::as_str) {
                declared_windows.insert(id.to_string());
            }
        }
    }

    let mut permission_ids = BTreeSet::new();
    let permissions = security.get("permissions").and_then(Value::as_array);
    if permissions.is_none_or(Vec::is_empty) {
        errors.push(Diagnostic::error(
            "RF1403",
            "security.permissions must declare at least one window scope",
        ));
    }
    for scope in permissions.into_iter().flatten() {
        let window = scope
            .get("window")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let is_pattern = window.len() > 1
            && window.ends_with('*')
            && !window[..window.len().saturating_sub(1)].contains('*');
        if !is_pattern && !declared_windows.contains(window) {
            errors.push(Diagnostic::error(
                "RF1404",
                format!("permission scope references undeclared window '{window}'"),
            ));
        }
        let mut local = BTreeSet::new();
        for permission in scope
            .get("allow")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(permission) = permission.as_str() else {
                continue;
            };
            if !KNOWN_PERMISSIONS.contains(&permission) && !permission.starts_with("shell:") {
                errors.push(Diagnostic::error(
                    "RF1405",
                    format!("unknown permission '{permission}'"),
                ));
            }
            if let Some(command) = permission.strip_prefix("shell:") {
                let command_exists = shell
                    .and_then(Value::as_object)
                    .and_then(|value| value.get("commands"))
                    .and_then(Value::as_array)
                    .is_some_and(|commands| {
                        commands.iter().any(|declared| {
                            declared
                                .get("id")
                                .or_else(|| declared.get("name"))
                                .and_then(Value::as_str)
                                == Some(command)
                        })
                    });
                if !command_exists {
                    errors.push(Diagnostic::error(
                        "RF1409",
                        format!(
                            "permission '{permission}' has no matching shell command declaration"
                        ),
                    ));
                }
            }
            if !local.insert(permission) {
                errors.push(Diagnostic::error(
                    "RF1406",
                    format!("permission '{permission}' is duplicated for window '{window}'"),
                ));
            }
            permission_ids.insert(permission);
        }
    }
    if model == "networked"
        && permission_ids.iter().any(|permission| {
            permission.starts_with("db:")
                || permission.starts_with("fs:")
                || permission.starts_with("shell:")
        })
    {
        errors.push(Diagnostic::error(
            "RF1407",
            "networked frontends cannot receive database, filesystem, or shell permissions",
        ));
    }
    if let Some(csp) = security.get("csp").and_then(Value::as_str) {
        let directives = csp
            .split(';')
            .filter_map(|directive| {
                let mut parts = directive.split_whitespace();
                parts
                    .next()
                    .map(|name| (name.to_ascii_lowercase(), parts.collect::<Vec<_>>()))
            })
            .collect::<Vec<_>>();
        let directive_has = |name: &str, value: &str| {
            directives
                .iter()
                .find(|(candidate, _)| candidate == name)
                .is_some_and(|(_, values)| values.contains(&value))
        };
        let has_unsafe_script = directives
            .iter()
            .find(|(name, _)| name == "script-src")
            .is_some_and(|(_, values)| {
                values
                    .iter()
                    .any(|value| matches!(*value, "'unsafe-inline'" | "'unsafe-eval'" | "*"))
            });
        if !directives.iter().any(|(name, _)| name == "default-src")
            || !directive_has("object-src", "'none'")
            || !directive_has("base-uri", "'none'")
            || !directive_has("frame-ancestors", "'none'")
            || has_unsafe_script
            || csp.contains('\r')
            || csp.contains('\n')
        {
            errors.push(Diagnostic::error(
                "RF1408",
                "security.csp must define default-src, object-src 'none', base-uri 'none', and frame-ancestors 'none' without unsafe script sources",
            ));
        }
    } else {
        warnings.push(Diagnostic::error(
            "RF1408",
            "security.csp is not explicit; the HTML policy remains authoritative",
        ));
    }
}

fn validate_packaging(project: &Path, value: Option<&Value>, errors: &mut Vec<Diagnostic>) {
    let Some(packaging) = value.and_then(Value::as_object) else {
        return;
    };
    validate_packaging_icon_path(project, "packaging.icon", packaging.get("icon"), errors);
    for platform in ["linux", "windows", "macos"] {
        validate_packaging_icon_path(
            project,
            &format!("packaging.{platform}.icon"),
            packaging.get(platform).and_then(|value| value.get("icon")),
            errors,
        );
    }
}

fn validate_packaging_icon_path(
    project: &Path,
    field: &str,
    value: Option<&Value>,
    errors: &mut Vec<Diagnostic>,
) {
    if let Some(icon) = value.and_then(Value::as_str) {
        match validate_relative_path(field, icon) {
            Ok(path) if !project.join(&path).is_file() => errors.push(Diagnostic::error(
                "RF1501",
                format!(
                    "packaging icon '{}' is missing",
                    project.join(icon).display()
                ),
            )),
            Err(error) => errors.push(Diagnostic::error("RF1502", error)),
            _ => {}
        }
    }
}

fn validate_shell(value: Option<&Value>, errors: &mut Vec<Diagnostic>) {
    let Some(shell) = value.and_then(Value::as_object) else {
        return;
    };
    let mut ids = BTreeSet::new();
    for command in shell
        .get("commands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = command
            .get("id")
            .or_else(|| command.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if id.is_empty() || !ids.insert(id) {
            errors.push(Diagnostic::error(
                "RF1601",
                format!("shell command id '{id}' is empty or duplicated"),
            ));
        }
        let program = command
            .get("program")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if program.is_empty() || Path::new(program).is_absolute() || program.contains(['/', '\\']) {
            errors.push(Diagnostic::error(
                "RF1602",
                format!(
                    "shell command '{id}' must use a program name, not an absolute or relative path"
                ),
            ));
        }
    }
}

pub fn validate_relative_path(field: &str, value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    let normalized = value.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "{field} '{value}' must be a safe project-relative path"
        ));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_reject_parent_and_absolute_components() {
        assert!(validate_relative_path("field", "../secret").is_err());
        assert!(validate_relative_path("field", "..\\secret").is_err());
        assert!(validate_relative_path("field", "/tmp/secret").is_err());
        assert_eq!(
            validate_relative_path("field", "src/generated.ts").unwrap(),
            "src/generated.ts"
        );
    }

    #[test]
    fn security_rejects_global_window_patterns_and_unknown_shell_permissions() {
        let app = json!({ "windows": [{ "id": "main" }] });
        let security = json!({
            "model": "local-first",
            "csp": "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
            "permissions": [{ "window": "*", "allow": ["shell:missing"] }]
        });
        let shell = json!({ "commands": [{ "id": "declared", "program": "echo" }] });
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        validate_security(
            Some(&app),
            Some(&security),
            Some(&shell),
            &mut errors,
            &mut warnings,
        );

        assert!(errors.iter().any(|error| error.code == "RF1404"));
        assert!(errors.iter().any(|error| error.code == "RF1409"));
    }

    #[test]
    fn security_rejects_an_unsafe_or_incomplete_csp() {
        let app = json!({ "windows": [{ "id": "main" }] });
        let security = json!({
            "model": "local-first",
            "csp": "default-src *; script-src 'unsafe-eval'",
            "permissions": [{ "window": "main", "allow": ["db:read"] }]
        });
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        validate_security(
            Some(&app),
            Some(&security),
            None,
            &mut errors,
            &mut warnings,
        );

        assert!(errors.iter().any(|error| error.code == "RF1408"));
    }

    #[test]
    fn schema_contract_rejects_missing_nested_required_fields() {
        let manifest = json!({
            "$schema": SCHEMA_URL,
            "schemaVersion": 1,
            "app": { "id": "test-app", "title": "Test App", "windows": [
                { "id": "main", "title": "Test App", "width": 800, "height": 600 }
            ] },
            "frontend": {
                "devCommand": "npm run dev", "buildCommand": "npm run build",
                "devUrl": "http://127.0.0.1:5173", "distDir": "dist",
                "generatedTypes": "src/rustframe.generated.ts"
            },
            "security": {
                "model": "local-first", "csp": "default-src 'self'",
                "permissions": [{ "window": "main", "allow": ["db:read"] }]
            },
            "database": { "schema": "data/schema.json", "seeds": "data/seeds", "migrations": "data/migrations" },
            "filesystem": { "roots": [] },
            "shell": { "commands": [] },
            "packaging": { "version": "0.1.0", "identifier": "dev.rustframe.test-app", "icon": "assets/icon.svg" }
        });

        let errors = validate_schema_contract(&manifest).unwrap();

        assert!(
            errors
                .iter()
                .any(|error| { error.code == "RF1005" && error.message.contains("persistGrants") })
        );
    }
}
