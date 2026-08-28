use std::{fs, path::Path};

use serde_json::{Value, json};

use crate::{
    CliResult, build_app_inspection, build_doctor_report, capabilities, collect_host_checks,
    load_app_project, local_first, manifest, slash_path,
};

const MAX_AUDIT_ENTRIES: usize = 200;

pub fn export(project_dir: &Path, name: &str, destination: Option<&Path>) -> CliResult<()> {
    let destination = destination
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                project_dir.join(path)
            }
        })
        .unwrap_or_else(|| {
            project_dir.join("target/rustframe/diagnostics/rustframe-diagnostics.json")
        });

    let validation = result_value(manifest::validate_project(project_dir));
    let project = match load_app_project(project_dir, name) {
        Ok(app) => json!({
            "state": "loaded",
            "inspection": result_value(build_app_inspection(&app)),
            "localFirst": result_value(local_first::inspect(&app)),
            "capabilities": result_value(capabilities::from_app(&app))
        }),
        Err(error) => json!({
            "state": "invalid",
            "error": error
        }),
    };
    let audit = read_audit_tail(&project_dir.join("target/rustframe/logs/audit.jsonl"))?;
    let mut bundle = json!({
        "schemaVersion": 1,
        "kind": "rustframe.diagnostics-bundle",
        "redacted": true,
        "cliVersion": env!("CARGO_PKG_VERSION"),
        "doctor": build_doctor_report(&collect_host_checks()),
        "validation": validation,
        "project": project,
        "logs": {
            "nativeAudit": audit,
            "frontendConsole": {
                "state": "not-captured",
                "detail": "Use rustframe dev --open-devtools for the live frontend console; this bundle never injects page content capture."
            }
        },
        "redaction": {
            "projectDirectory": "<PROJECT_DIR>",
            "homeDirectory": "<HOME>",
            "capabilityUris": "opaque grant and root identifiers removed",
            "documentContentsIncluded": false,
            "environmentValuesIncluded": false
        }
    });
    let patterns = redaction_patterns(project_dir);
    redact_value(&mut bundle, &patterns);

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create diagnostics directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    let contents = serde_json::to_string_pretty(&bundle)
        .map_err(|error| format!("failed to serialize diagnostics bundle: {error}"))?;
    fs::write(&destination, format!("{contents}\n")).map_err(|error| {
        format!(
            "failed to write diagnostics bundle '{}': {error}",
            destination.display()
        )
    })?;
    println!("Redacted diagnostics bundle: {}", destination.display());
    Ok(())
}

fn result_value<T: serde::Serialize>(result: Result<T, String>) -> Value {
    match result {
        Ok(value) => json!({ "state": "ok", "value": value }),
        Err(error) => json!({ "state": "error", "error": error }),
    }
}

fn read_audit_tail(path: &Path) -> CliResult<Value> {
    if !path.is_file() {
        return Ok(json!({
            "state": "not-found",
            "entries": [],
            "truncated": false
        }));
    }
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read native audit log '{}': {error}",
            path.display()
        )
    })?;
    let lines = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let start = lines.len().saturating_sub(MAX_AUDIT_ENTRIES);
    let entries = lines[start..]
        .iter()
        .map(|line| {
            serde_json::from_str::<Value>(line).unwrap_or_else(|_| json!({ "unparsed": line }))
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "state": "captured",
        "entries": entries,
        "truncated": start > 0
    }))
}

fn redaction_patterns(project_dir: &Path) -> Vec<(String, &'static str)> {
    let mut patterns = vec![
        (slash_path(project_dir), "<PROJECT_DIR>"),
        (project_dir.to_string_lossy().to_string(), "<PROJECT_DIR>"),
    ];
    if let Some(home) = dirs::home_dir() {
        patterns.push((slash_path(&home), "<HOME>"));
        patterns.push((home.to_string_lossy().to_string(), "<HOME>"));
    }
    patterns.sort_by(|left, right| right.0.len().cmp(&left.0.len()));
    patterns.dedup_by(|left, right| left.0 == right.0);
    patterns
}

fn redact_value(value: &mut Value, patterns: &[(String, &'static str)]) {
    match value {
        Value::String(source) => {
            for (needle, replacement) in patterns {
                if !needle.is_empty() {
                    *source = source.replace(needle, replacement);
                }
            }
            *source = redact_capability_uris(source);
        }
        Value::Array(values) => {
            for value in values {
                redact_value(value, patterns);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                redact_value(value, patterns);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn redact_capability_uris(source: &str) -> String {
    let mut redacted = source.to_string();
    for scheme in ["grant://", "root://"] {
        let mut cursor = 0;
        while let Some(relative) = redacted[cursor..].find(scheme) {
            let start = cursor + relative;
            let token_start = start + scheme.len();
            let token_len = redacted[token_start..]
                .char_indices()
                .take_while(|(_, character)| {
                    !character.is_whitespace()
                        && !matches!(character, ',' | ';' | ')' | ']' | '}' | '"' | '\'')
                })
                .last()
                .map_or(0, |(index, character)| index + character.len_utf8());
            if token_len == 0 {
                cursor = token_start;
                continue;
            }
            redacted.replace_range(token_start..token_start + token_len, "<redacted>");
            cursor = token_start + "<redacted>".len();
        }
    }
    redacted
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{redact_capability_uris, redact_value};

    #[test]
    fn redacts_paths_and_opaque_capability_identifiers_recursively() {
        let mut value = json!({
            "path": "/Users/person/project/private/note.md",
            "grant": "read grant://very-secret-id now",
            "nested": ["root://workspace-secret/doc.txt"]
        });
        redact_value(
            &mut value,
            &[("/Users/person/project".into(), "<PROJECT_DIR>")],
        );
        assert_eq!(value["path"], "<PROJECT_DIR>/private/note.md");
        assert_eq!(value["grant"], "read grant://<redacted> now");
        assert_eq!(value["nested"][0], "root://<redacted>");
        assert_eq!(
            redact_capability_uris("no capability URI"),
            "no capability URI"
        );
    }
}
