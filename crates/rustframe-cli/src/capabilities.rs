use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{AppProject, CliResult};

pub const POLICY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityPolicy {
    pub schema_version: u32,
    pub kind: String,
    pub app_id: String,
    pub security_model: String,
    pub csp: Option<String>,
    pub persist_grants: bool,
    pub filesystem_roots: Vec<String>,
    pub windows: BTreeMap<String, Vec<String>>,
    pub shell_commands: Vec<ShellPolicy>,
    pub policy_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellPolicy {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub allowed_args: Vec<String>,
    pub cwd: Option<String>,
    pub environment_keys: Vec<String>,
    pub clear_env: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDiff {
    pub schema_version: u32,
    pub old_policy_hash: String,
    pub new_policy_hash: String,
    pub changed: bool,
    pub expanded: bool,
    pub additions: Vec<String>,
    pub removals: Vec<String>,
    pub changes: Vec<String>,
    pub expansions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyHashInput<'a> {
    schema_version: u32,
    kind: &'a str,
    app_id: &'a str,
    security_model: &'a str,
    csp: &'a Option<String>,
    persist_grants: bool,
    filesystem_roots: &'a [String],
    windows: &'a BTreeMap<String, Vec<String>>,
    shell_commands: &'a [ShellPolicy],
}

pub fn from_app(app: &AppProject) -> CliResult<CapabilityPolicy> {
    let path = app.app_dir.join("rustframe.json");
    let source = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read capability input '{}': {error}",
            path.display()
        )
    })?;
    let value: Value = serde_json::from_str(&source).map_err(|error| {
        format!(
            "failed to parse capability input '{}': {error}",
            path.display()
        )
    })?;
    from_manifest_value(&value)
}

pub fn from_file(path: &Path) -> CliResult<CapabilityPolicy> {
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read capability input '{}': {error}",
            path.display()
        )
    })?;
    let value: Value = serde_json::from_str(&source).map_err(|error| {
        format!(
            "failed to parse capability input '{}': {error}",
            path.display()
        )
    })?;
    if value.get("kind").and_then(Value::as_str) == Some("rustframe.capability-policy") {
        let policy: CapabilityPolicy = serde_json::from_value(value)
            .map_err(|error| format!("invalid capability policy '{}': {error}", path.display()))?;
        return finalize(policy);
    }
    from_manifest_value(&value).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn from_manifest_value(value: &Value) -> CliResult<CapabilityPolicy> {
    let root = value
        .as_object()
        .ok_or_else(|| "manifest must contain a JSON object".to_string())?;
    let app = root.get("app").and_then(Value::as_object);
    let app_id = app
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .or_else(|| root.get("appId").and_then(Value::as_str))
        .ok_or_else(|| "manifest app.id is missing".to_string())?;
    let security = root.get("security").and_then(Value::as_object);
    let filesystem = root.get("filesystem").and_then(Value::as_object);
    let mut windows = BTreeMap::new();

    if let Some(app) = app {
        if let Some(window) = app.get("window").and_then(Value::as_object) {
            let id = window.get("id").and_then(Value::as_str).unwrap_or("main");
            windows.entry(id.to_string()).or_insert_with(Vec::new);
        }
        if let Some(values) = app.get("windows").and_then(Value::as_array) {
            for window in values.iter().filter_map(Value::as_object) {
                if let Some(id) = window.get("id").and_then(Value::as_str) {
                    windows.entry(id.to_string()).or_insert_with(Vec::new);
                }
            }
        }
    }
    if let Some(values) = security
        .and_then(|value| value.get("permissions"))
        .and_then(Value::as_array)
    {
        for entry in values.iter().filter_map(Value::as_object) {
            let Some(window) = entry.get("window").and_then(Value::as_str) else {
                continue;
            };
            let permissions = entry
                .get("allow")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            windows.insert(window.to_string(), permissions);
        }
    }
    if windows.is_empty() {
        windows.insert("main".into(), Vec::new());
    }
    normalize_windows(&mut windows);

    let mut filesystem_roots = filesystem
        .and_then(|value| value.get("roots"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    filesystem_roots.sort();
    filesystem_roots.dedup();

    let mut shell_commands = root
        .get("shell")
        .and_then(Value::as_object)
        .and_then(|value| value.get("commands"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(|command| ShellPolicy {
            id: command
                .get("id")
                .or_else(|| command.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("unnamed")
                .to_string(),
            program: command
                .get("program")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            args: string_array(command.get("args")),
            allowed_args: string_array(command.get("allowedArgs")),
            cwd: command
                .get("cwd")
                .and_then(Value::as_str)
                .map(str::to_string),
            environment_keys: command
                .get("env")
                .and_then(Value::as_object)
                .map(|values| values.keys().cloned().collect())
                .unwrap_or_default(),
            clear_env: command
                .get("clearEnv")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
        .collect::<Vec<_>>();
    shell_commands.sort();

    finalize(CapabilityPolicy {
        schema_version: POLICY_SCHEMA_VERSION,
        kind: "rustframe.capability-policy".into(),
        app_id: app_id.to_string(),
        security_model: security
            .and_then(|value| value.get("model"))
            .and_then(Value::as_str)
            .unwrap_or("local-first")
            .to_string(),
        csp: security
            .and_then(|value| value.get("csp"))
            .and_then(Value::as_str)
            .map(str::to_string),
        persist_grants: filesystem
            .and_then(|value| value.get("persistGrants"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        filesystem_roots,
        windows,
        shell_commands,
        policy_hash: String::new(),
    })
}

pub fn diff(old: &CapabilityPolicy, new: &CapabilityPolicy) -> CapabilityDiff {
    let mut additions = Vec::new();
    let mut removals = Vec::new();
    let mut changes = Vec::new();
    let mut expansions = Vec::new();

    if old.app_id != new.app_id {
        let change = format!("app id changed from '{}' to '{}'", old.app_id, new.app_id);
        changes.push(change.clone());
        expansions.push(change);
    }
    if old.security_model != new.security_model {
        let change = format!(
            "security model changed from '{}' to '{}'",
            old.security_model, new.security_model
        );
        if old.security_model == "local-first" && new.security_model != "local-first" {
            expansions.push(change.clone());
        }
        changes.push(change);
    }
    if old.csp != new.csp {
        let change = "content security policy changed and requires review".to_string();
        changes.push(change.clone());
        expansions.push(change);
    }
    if !old.persist_grants && new.persist_grants {
        expansions.push("persisted filesystem grants were enabled".into());
        changes.push("persisted filesystem grants changed from disabled to enabled".into());
    } else if old.persist_grants && !new.persist_grants {
        changes.push("persisted filesystem grants changed from enabled to disabled".into());
    }

    compare_sets(
        "filesystem root",
        &old.filesystem_roots,
        &new.filesystem_roots,
        &mut additions,
        &mut removals,
        &mut expansions,
    );

    let window_ids = old
        .windows
        .keys()
        .chain(new.windows.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for window in window_ids {
        if !old.windows.contains_key(&window) {
            additions.push(format!("window: {window}"));
        }
        if !new.windows.contains_key(&window) {
            removals.push(format!("window: {window}"));
        }
        compare_sets(
            &format!("permission for window '{window}'"),
            old.windows.get(&window).map(Vec::as_slice).unwrap_or(&[]),
            new.windows.get(&window).map(Vec::as_slice).unwrap_or(&[]),
            &mut additions,
            &mut removals,
            &mut expansions,
        );
    }

    let old_shell = old
        .shell_commands
        .iter()
        .map(shell_signature)
        .collect::<Vec<_>>();
    let new_shell = new
        .shell_commands
        .iter()
        .map(shell_signature)
        .collect::<Vec<_>>();
    compare_sets(
        "shell command",
        &old_shell,
        &new_shell,
        &mut additions,
        &mut removals,
        &mut expansions,
    );

    additions.sort();
    removals.sort();
    changes.sort();
    expansions.sort();
    expansions.dedup();
    CapabilityDiff {
        schema_version: POLICY_SCHEMA_VERSION,
        old_policy_hash: old.policy_hash.clone(),
        new_policy_hash: new.policy_hash.clone(),
        changed: !additions.is_empty() || !removals.is_empty() || !changes.is_empty(),
        expanded: !expansions.is_empty(),
        additions,
        removals,
        changes,
        expansions,
    }
}

pub fn render_explanation(policy: &CapabilityPolicy) -> String {
    let mut lines = vec![
        format!("Capability policy for {}", policy.app_id),
        format!("  model: {}", policy.security_model),
        format!("  policy hash: {}", policy.policy_hash),
        format!(
            "  persisted grants: {}",
            if policy.persist_grants {
                "allowed"
            } else {
                "disabled"
            }
        ),
    ];
    if policy.filesystem_roots.is_empty() {
        lines.push("  filesystem roots: none (user grants may still be requested)".into());
    } else {
        lines.push(format!(
            "  filesystem roots: {}",
            policy.filesystem_roots.join(", ")
        ));
    }
    lines.push("".into());
    for (window, permissions) in &policy.windows {
        lines.push(format!("Window '{window}'"));
        if permissions.is_empty() {
            lines.push("  no explicit bridge permissions".into());
        } else {
            lines.extend(
                permissions
                    .iter()
                    .map(|permission| format!("  allow {permission}")),
            );
        }
    }
    if !policy.shell_commands.is_empty() {
        lines.push("".into());
        lines.push("Bounded shell commands".into());
        lines.extend(policy.shell_commands.iter().map(|command| {
            format!(
                "  {} -> {} {}",
                command.id,
                command.program,
                command.args.join(" ")
            )
        }));
    }
    lines.join("\n")
}

pub fn render_diff(report: &CapabilityDiff) -> String {
    let mut lines = vec![format!(
        "Capability policy: {}",
        if !report.changed {
            "unchanged"
        } else if report.expanded {
            "expanded"
        } else {
            "changed without privilege expansion"
        }
    )];
    for value in &report.additions {
        lines.push(format!("  + {value}"));
    }
    for value in &report.removals {
        lines.push(format!("  - {value}"));
    }
    for value in &report.changes {
        lines.push(format!("  ~ {value}"));
    }
    if report.expanded {
        lines.push("Privilege expansion requiring explicit review:".into());
        lines.extend(report.expansions.iter().map(|value| format!("  ! {value}")));
    }
    lines.join("\n")
}

pub fn write_policy(path: &Path, policy: &CapabilityPolicy) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    }
    let rendered = serde_json::to_string_pretty(policy)
        .map_err(|error| format!("failed to serialize capability policy: {error}"))?;
    fs::write(path, format!("{rendered}\n"))
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))
}

pub fn resolve_from_project(project: &Path, value: &Path) -> PathBuf {
    if value.is_absolute() {
        value.to_path_buf()
    } else {
        project.join(value)
    }
}

fn finalize(mut policy: CapabilityPolicy) -> CliResult<CapabilityPolicy> {
    if policy.schema_version != POLICY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported capability policy schema {}",
            policy.schema_version
        ));
    }
    policy.kind = "rustframe.capability-policy".into();
    policy.filesystem_roots.sort();
    policy.filesystem_roots.dedup();
    normalize_windows(&mut policy.windows);
    policy.shell_commands.sort();
    let input = PolicyHashInput {
        schema_version: policy.schema_version,
        kind: &policy.kind,
        app_id: &policy.app_id,
        security_model: &policy.security_model,
        csp: &policy.csp,
        persist_grants: policy.persist_grants,
        filesystem_roots: &policy.filesystem_roots,
        windows: &policy.windows,
        shell_commands: &policy.shell_commands,
    };
    let encoded = serde_json::to_vec(&input)
        .map_err(|error| format!("failed to hash capability policy: {error}"))?;
    policy.policy_hash = format!("sha256:{:x}", Sha256::digest(encoded));
    Ok(policy)
}

fn normalize_windows(windows: &mut BTreeMap<String, Vec<String>>) {
    for permissions in windows.values_mut() {
        permissions.sort();
        permissions.dedup();
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn compare_sets(
    label: &str,
    old: &[String],
    new: &[String],
    additions: &mut Vec<String>,
    removals: &mut Vec<String>,
    expansions: &mut Vec<String>,
) {
    let old = old.iter().cloned().collect::<BTreeSet<_>>();
    let new = new.iter().cloned().collect::<BTreeSet<_>>();
    for value in new.difference(&old) {
        let entry = format!("{label}: {value}");
        additions.push(entry.clone());
        expansions.push(entry);
    }
    for value in old.difference(&new) {
        removals.push(format!("{label}: {value}"));
    }
}

fn shell_signature(command: &ShellPolicy) -> String {
    serde_json::to_string(command).unwrap_or_else(|_| command.id.clone())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{diff, from_manifest_value};

    #[test]
    fn normalizes_manifest_policy_and_hashes_it_deterministically() {
        let manifest = json!({
            "app": { "id": "desk", "windows": [{"id": "reader"}, {"id": "main"}] },
            "security": {
                "model": "local-first",
                "permissions": [
                    {"window": "main", "allow": ["db:write", "db:read", "db:read"]},
                    {"window": "reader", "allow": ["db:read"]}
                ]
            },
            "filesystem": { "persistGrants": false, "roots": [] },
            "shell": { "commands": [] }
        });
        let first = from_manifest_value(&manifest).unwrap();
        let second = from_manifest_value(&manifest).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.windows["main"], ["db:read", "db:write"]);
        assert!(first.policy_hash.starts_with("sha256:"));
    }

    #[test]
    fn identifies_only_new_privilege_as_expansion() {
        let old = from_manifest_value(&json!({
            "app": { "id": "desk", "windows": [{"id": "main"}] },
            "security": { "model": "local-first", "permissions": [{"window": "main", "allow": ["db:read"]}] },
            "filesystem": { "persistGrants": false, "roots": [] },
            "shell": { "commands": [] }
        })).unwrap();
        let expanded = from_manifest_value(&json!({
            "app": { "id": "desk", "windows": [{"id": "main"}] },
            "security": { "model": "local-first", "permissions": [{"window": "main", "allow": ["db:read", "db:write"]}] },
            "filesystem": { "persistGrants": true, "roots": [] },
            "shell": { "commands": [] }
        })).unwrap();
        let report = diff(&old, &expanded);
        assert!(report.expanded);
        assert_eq!(report.expansions.len(), 2);
    }
}
