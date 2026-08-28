use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::{
    AppProject, AppSecurityModel, CliResult, capabilities, list_files_with_extension, slash_path,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFirstReport {
    pub schema_version: u32,
    pub kind: &'static str,
    pub app_id: String,
    pub title: String,
    pub conformant: bool,
    pub policy_hash: String,
    pub assets: AssetReport,
    pub network: NetworkReport,
    pub database: DatabaseReport,
    pub filesystem: FilesystemReport,
    pub windows: Vec<WindowReport>,
    pub shell: ShellReport,
    pub packaging: PackagingReport,
    pub findings: Vec<LocalFirstFinding>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetReport {
    pub mode: &'static str,
    pub bundled_root: String,
    pub exists: bool,
    pub file_count: usize,
    pub bytes: u64,
    pub remote_references: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkReport {
    pub model: String,
    pub csp: Option<String>,
    pub restrictive_csp: bool,
    pub production_server_required: bool,
    pub undeclared_remote_dependencies: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseReport {
    pub enabled: bool,
    pub schema: String,
    pub schema_exists: bool,
    pub schema_version: Option<u32>,
    pub migration_count: usize,
    pub backup: bool,
    pub restore: bool,
    pub portable_export_formats: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemReport {
    pub declared_roots: Vec<String>,
    pub persisted_grants: bool,
    pub grant_policy: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowReport {
    pub id: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellReport {
    pub enabled: bool,
    pub command_count: usize,
    pub command_ids: Vec<String>,
    pub arbitrary_frontend_execution: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagingReport {
    pub version: String,
    pub native_host_required: bool,
    pub single_instance: bool,
    pub file_association_count: usize,
    pub associated_extensions: Vec<String>,
    pub signing_policy: &'static str,
    pub update_policy: &'static str,
    pub verification_command: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFirstFinding {
    pub severity: &'static str,
    pub code: &'static str,
    pub message: String,
}

pub fn inspect(app: &AppProject) -> CliResult<LocalFirstReport> {
    let policy = capabilities::from_app(app)?;
    let bundled_root = app.app_dir.join(&app.config.frontend.dist_dir);
    let asset_scan = scan_assets(&app.app_dir, &bundled_root)?;
    let csp = app.config.security.csp.clone();
    let restrictive_csp = csp.as_deref().is_some_and(is_restrictive_csp);
    let schema_path = app.app_dir.join(&app.config.database.schema);
    let schema_version = read_schema_version(&schema_path)?;
    let migration_count =
        list_files_with_extension(&app.app_dir.join(&app.config.database.migrations), "sql")?.len();
    let mut findings = Vec::new();

    if app.config.security.model != AppSecurityModel::LocalFirst {
        findings.push(LocalFirstFinding {
            severity: "error",
            code: "RF-LF-001",
            message:
                "security.model is networked; the packaged workflow is not declared local-first"
                    .into(),
        });
    }
    if !bundled_root.exists() {
        findings.push(LocalFirstFinding {
            severity: "error",
            code: "RF-LF-002",
            message: format!(
                "bundled frontend root '{}' does not exist",
                app.config.frontend.dist_dir
            ),
        });
    }
    if !restrictive_csp {
        findings.push(LocalFirstFinding {
            severity: "error",
            code: "RF-LF-003",
            message:
                "the declared CSP must keep default sources local and disable object embedding"
                    .into(),
        });
    }
    if !asset_scan.remote_references.is_empty() {
        findings.push(LocalFirstFinding {
            severity: "warning",
            code: "RF-LF-004",
            message: format!(
                "bundled frontend text contains {} remote URL reference(s); review whether they are links or runtime dependencies",
                asset_scan.remote_references.len()
            ),
        });
    }
    if !app.config.shell_commands.is_empty() {
        findings.push(LocalFirstFinding {
            severity: "warning",
            code: "RF-LF-005",
            message: format!(
                "{} bounded shell command(s) require explicit release review",
                app.config.shell_commands.len()
            ),
        });
    }
    if app.config.security.persist_grants {
        findings.push(LocalFirstFinding {
            severity: "info",
            code: "RF-LF-006",
            message: "persisted opaque grants are enabled; users need a visible revoke path".into(),
        });
    }
    if !schema_path.exists() {
        findings.push(LocalFirstFinding {
            severity: "error",
            code: "RF-LF-007",
            message: format!(
                "database schema '{}' is missing",
                app.config.database.schema
            ),
        });
    }

    let conformant = !findings.iter().any(|finding| finding.severity == "error");
    Ok(LocalFirstReport {
        schema_version: 1,
        kind: "rustframe.local-first-conformance",
        app_id: app.config.app_id.clone(),
        title: app.config.title.clone(),
        conformant,
        policy_hash: policy.policy_hash,
        assets: AssetReport {
            mode: "bundled",
            bundled_root: slash_path(
                bundled_root
                    .strip_prefix(&app.app_dir)
                    .unwrap_or(&bundled_root),
            ),
            exists: bundled_root.exists(),
            file_count: asset_scan.file_count,
            bytes: asset_scan.bytes,
            remote_references: asset_scan.remote_references.clone(),
        },
        network: NetworkReport {
            model: match app.config.security.model {
                AppSecurityModel::LocalFirst => "local-first".into(),
                AppSecurityModel::Networked => "networked".into(),
            },
            csp,
            restrictive_csp,
            production_server_required: false,
            undeclared_remote_dependencies: asset_scan.remote_references,
        },
        database: DatabaseReport {
            enabled: app.config.security.database,
            schema: app.config.database.schema.clone(),
            schema_exists: schema_path.exists(),
            schema_version,
            migration_count,
            backup: app.config.security.database,
            restore: app.config.security.database,
            portable_export_formats: vec!["json", "jsonl", "csv"],
        },
        filesystem: FilesystemReport {
            declared_roots: app.config.fs_roots.clone(),
            persisted_grants: app.config.security.persist_grants,
            grant_policy: "opaque runtime grants resolved and authorized per request",
        },
        windows: policy
            .windows
            .into_iter()
            .map(|(id, permissions)| WindowReport { id, permissions })
            .collect(),
        shell: ShellReport {
            enabled: app.config.security.shell,
            command_count: app.config.shell_commands.len(),
            command_ids: app
                .config
                .shell_commands
                .iter()
                .map(|command| command.name.clone())
                .collect(),
            arbitrary_frontend_execution: false,
        },
        packaging: PackagingReport {
            version: app.config.packaging.version.clone(),
            native_host_required: true,
            single_instance: true,
            file_association_count: app.config.packaging.file_associations.len(),
            associated_extensions: app
                .config
                .packaging
                .file_associations
                .iter()
                .flat_map(|association| association.extensions.iter().cloned())
                .collect(),
            signing_policy: "observed by the protected native release workflow",
            update_policy: "not declared by RustFrame schema v1",
            verification_command: "rustframe release verify <artifact> --json",
        },
        findings,
    })
}

pub fn render(report: &LocalFirstReport) -> String {
    let mut lines = vec![
        format!("Local-first conformance for {}", report.title),
        format!(
            "  result: {}",
            if report.conformant {
                "conformant"
            } else {
                "not conformant"
            }
        ),
        format!("  policy hash: {}", report.policy_hash),
        format!(
            "  assets: {} bundled files, {} bytes",
            report.assets.file_count, report.assets.bytes
        ),
        format!(
            "  network: {}; restrictive CSP: {}",
            report.network.model, report.network.restrictive_csp
        ),
        format!(
            "  database: schema v{}; {} migration(s); export json/jsonl/csv",
            report
                .database
                .schema_version
                .map_or_else(|| "unknown".into(), |version| version.to_string()),
            report.database.migration_count
        ),
        format!(
            "  filesystem: {} declared roots; persisted grants: {}",
            report.filesystem.declared_roots.len(),
            report.filesystem.persisted_grants
        ),
        format!(
            "  shell: {} bounded command(s); arbitrary execution: false",
            report.shell.command_count
        ),
    ];
    if report.findings.is_empty() {
        lines.push("  findings: none".into());
    } else {
        lines.push("Findings".into());
        lines.extend(report.findings.iter().map(|finding| {
            format!(
                "  {} [{}] {}",
                finding.severity, finding.code, finding.message
            )
        }));
    }
    lines.join("\n")
}

pub fn write_report(path: &Path, report: &LocalFirstReport) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    }
    let rendered = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize local-first report: {error}"))?;
    fs::write(path, format!("{rendered}\n"))
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))
}

struct AssetScan {
    file_count: usize,
    bytes: u64,
    remote_references: Vec<String>,
}

fn scan_assets(app_dir: &Path, root: &Path) -> CliResult<AssetScan> {
    if !root.exists() {
        return Ok(AssetScan {
            file_count: 0,
            bytes: 0,
            remote_references: Vec::new(),
        });
    }
    let mut files = Vec::new();
    collect_asset_files(root, &mut files)?;
    let mut bytes = 0_u64;
    let mut references = BTreeSet::new();
    for path in &files {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("failed to inspect '{}': {error}", path.display()))?;
        bytes = bytes.saturating_add(metadata.len());
        if metadata.len() > 2 * 1024 * 1024 || !is_text_asset(path) {
            continue;
        }
        let source = fs::read_to_string(path).map_err(|error| {
            format!("failed to read bundled asset '{}': {error}", path.display())
        })?;
        references.extend(extract_remote_urls(&source));
    }
    let _ = app_dir;
    Ok(AssetScan {
        file_count: files.len(),
        bytes,
        remote_references: references.into_iter().collect(),
    })
}

fn collect_asset_files(directory: &Path, files: &mut Vec<PathBuf>) -> CliResult<()> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("failed to inspect '{}': {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to inspect bundled asset: {error}"))?;
        let path = entry.path();
        let name = entry.file_name();
        if matches!(name.to_str(), Some("node_modules" | "target" | ".git")) {
            continue;
        }
        let kind = entry
            .file_type()
            .map_err(|error| format!("failed to inspect '{}': {error}", path.display()))?;
        if kind.is_dir() {
            collect_asset_files(&path, files)?;
        } else if kind.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn is_text_asset(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("html" | "htm" | "js" | "mjs" | "css" | "json" | "svg" | "txt")
    )
}

fn extract_remote_urls(source: &str) -> Vec<String> {
    let mut values = BTreeSet::new();
    for marker in ["https://", "http://"] {
        let mut remaining = source;
        while let Some(index) = remaining.find(marker) {
            let candidate = &remaining[index..];
            let end = candidate
                .find(|character: char| {
                    character.is_whitespace()
                        || matches!(character, '"' | '\'' | '<' | '>' | ')' | ']' | '}')
                })
                .unwrap_or(candidate.len());
            let value = candidate[..end].trim_end_matches([';', ',']);
            if !is_loopback_url(value) {
                values.insert(value.to_string());
            }
            remaining = &candidate[marker.len()..];
        }
    }
    values.into_iter().collect()
}

fn is_loopback_url(value: &str) -> bool {
    [
        "http://127.0.0.1",
        "https://127.0.0.1",
        "http://localhost",
        "https://localhost",
    ]
    .iter()
    .any(|prefix| value.starts_with(prefix))
}

fn read_schema_version(path: &Path) -> CliResult<Option<u32>> {
    if !path.exists() {
        return Ok(None);
    }
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read database schema '{}': {error}",
            path.display()
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&source).map_err(|error| {
        format!(
            "failed to parse database schema '{}': {error}",
            path.display()
        )
    })?;
    Ok(value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|version| u32::try_from(version).ok()))
}

fn is_restrictive_csp(csp: &str) -> bool {
    let normalized = csp.to_ascii_lowercase();
    normalized.contains("default-src 'self'") && normalized.contains("object-src 'none'")
}

#[cfg(test)]
mod tests {
    use super::{extract_remote_urls, is_restrictive_csp};

    #[test]
    fn extracts_and_deduplicates_remote_references() {
        assert_eq!(
            extract_remote_urls(
                "fetch('https://api.example.test/v1'); https://api.example.test/v1 http://127.0.0.1:4316"
            ),
            ["https://api.example.test/v1"]
        );
    }

    #[test]
    fn requires_local_defaults_and_disabled_objects() {
        assert!(is_restrictive_csp("default-src 'self'; object-src 'none'"));
        assert!(!is_restrictive_csp("default-src *"));
    }
}
