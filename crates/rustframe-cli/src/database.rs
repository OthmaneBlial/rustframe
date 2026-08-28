use std::{
    fs,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use rustframe::{
    DatabaseCapability, DatabaseListQuery, DatabaseMigrationFile, DatabaseOpenConfig,
    DatabaseOrder, DatabaseOrderDirection, DatabaseSchema, DatabaseSeedFile,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    AppProject, CliResult, DATABASE_FILE_NAME, command::DatabaseExportFormat, default_app_data_dir,
    list_files_with_extension, load_app_project, print_capability_warnings, slash_path,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportManifest {
    schema_version: u32,
    export_version: u32,
    kind: &'static str,
    app_id: String,
    database_schema_version: u32,
    format: String,
    created_at_unix_ms: u128,
    consistent_snapshot: bool,
    tables: Vec<ExportTableRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportTableRecord {
    name: String,
    file: String,
    rows: u64,
    bytes: u64,
    sha256: String,
}

pub fn reset(project: &Path, name: &str) -> CliResult<()> {
    let app = load_app_project(project, name)?;
    print_capability_warnings(&app);
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
        "The next `rustframe dev` run for {} will recreate the database, migrations, and seed data.",
        app.config.title
    );
    Ok(())
}

pub fn backup(project: &Path, name: &str, destination: Option<&Path>) -> CliResult<()> {
    let app = load_app_project(project, name)?;
    let data_dir = default_app_data_dir(&app.config.app_id)?;
    let source = data_dir.join(DATABASE_FILE_NAME);
    let default_destination = app
        .app_dir
        .join("backups")
        .join(format!("{}-backup.db", app.config.app_id));
    let destination = destination.unwrap_or(&default_destination);
    rustframe::backup_database_file(&source, destination)
        .map_err(|error| format!("database backup failed: {error}"))?;
    println!("Backed up database to {}", destination.display());
    Ok(())
}

pub fn restore(project: &Path, name: &str, source: &Path) -> CliResult<()> {
    let app = load_app_project(project, name)?;
    let data_dir = default_app_data_dir(&app.config.app_id)?;
    let safety_dir = app.app_dir.join("backups");
    fs::create_dir_all(&safety_dir)
        .map_err(|error| format!("failed to create '{}': {error}", safety_dir.display()))?;
    let safety = safety_dir.join(format!("{}-pre-restore.db", app.config.app_id));
    let database = open_project_database(&app, data_dir)?;
    database
        .restore_from(source, &safety)
        .map_err(|error| format!("database restore failed: {error}"))?;
    println!("Restored database from {}", source.display());
    println!("Safety backup: {}", safety.display());
    Ok(())
}

pub fn export(
    project: &Path,
    name: &str,
    destination: Option<&Path>,
    format: DatabaseExportFormat,
    batch_size: u32,
) -> CliResult<()> {
    let app = load_app_project(project, name)?;
    print_capability_warnings(&app);
    let schema_path = app.app_dir.join(&app.config.database.schema);
    let schema_source = fs::read_to_string(&schema_path).map_err(|error| {
        format!(
            "failed to read database schema '{}': {error}",
            schema_path.display()
        )
    })?;
    let schema = DatabaseSchema::from_json(&schema_source).map_err(|error| {
        format!(
            "invalid database schema '{}': {error}",
            schema_path.display()
        )
    })?;
    let timestamp = unix_time_ms()?;
    let destination = destination.map_or_else(
        || {
            app.app_dir
                .join("exports")
                .join(format!("{}-{timestamp}", app.config.app_id))
        },
        |path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                app.app_dir.join(path)
            }
        },
    );
    if destination.exists() {
        return Err(format!(
            "export destination '{}' already exists; choose a new directory",
            destination.display()
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        format!(
            "export destination '{}' has no parent directory",
            destination.display()
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    let destination_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "export destination must end in a valid directory name".to_string())?;
    let staging = parent.join(format!(
        ".{destination_name}.rustframe-staging-{}-{timestamp}",
        process::id()
    ));
    let snapshot_dir = std::env::temp_dir().join(format!(
        "rustframe-export-snapshot-{}-{timestamp}",
        process::id()
    ));

    let result = (|| {
        fs::create_dir_all(&snapshot_dir).map_err(|error| {
            format!(
                "failed to create export snapshot directory '{}': {error}",
                snapshot_dir.display()
            )
        })?;
        let data_dir = default_app_data_dir(&app.config.app_id)?;
        let active = open_project_database(&app, data_dir)?;
        active
            .backup_to(&snapshot_dir.join(DATABASE_FILE_NAME))
            .map_err(|error| format!("failed to create consistent export snapshot: {error}"))?;
        drop(active);

        let snapshot = open_project_database(&app, snapshot_dir.clone())?;
        fs::create_dir_all(staging.join("tables"))
            .map_err(|error| format!("failed to create export staging directory: {error}"))?;
        let mut table_records = Vec::new();
        for table in &schema.tables {
            let columns = table
                .columns
                .iter()
                .map(|column| column.name.clone())
                .collect::<Vec<_>>();
            table_records.push(export_table(
                &snapshot,
                &table.name,
                &columns,
                &staging,
                format,
                batch_size,
            )?);
        }

        let manifest = ExportManifest {
            schema_version: 1,
            export_version: 1,
            kind: "rustframe.portable-data-export",
            app_id: app.config.app_id.clone(),
            database_schema_version: schema.version,
            format: format.as_str().into(),
            created_at_unix_ms: timestamp,
            consistent_snapshot: true,
            tables: table_records,
        };
        let manifest_path = staging.join("export-manifest.json");
        let rendered = serde_json::to_string_pretty(&manifest)
            .map_err(|error| format!("failed to serialize export manifest: {error}"))?;
        fs::write(&manifest_path, format!("{rendered}\n")).map_err(|error| {
            format!(
                "failed to write export manifest '{}': {error}",
                manifest_path.display()
            )
        })?;

        let mut checksums = manifest
            .tables
            .iter()
            .map(|table| format!("{}  {}", table.sha256, table.file))
            .collect::<Vec<_>>();
        checksums.push(format!(
            "{}  export-manifest.json",
            file_sha256(&manifest_path)?
        ));
        checksums.sort();
        fs::write(
            staging.join("SHA256SUMS"),
            format!("{}\n", checksums.join("\n")),
        )
        .map_err(|error| format!("failed to write export checksums: {error}"))?;
        fs::rename(&staging, &destination).map_err(|error| {
            format!(
                "failed to publish export '{}' atomically: {error}",
                destination.display()
            )
        })?;
        Ok::<_, String>(manifest)
    })();

    let _ = fs::remove_dir_all(&snapshot_dir);
    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    let manifest = result?;
    println!(
        "Exported a consistent SQLite snapshot to {}",
        destination.display()
    );
    println!("Format: {}", manifest.format);
    println!(
        "Rows: {} across {} table(s)",
        manifest.tables.iter().map(|table| table.rows).sum::<u64>(),
        manifest.tables.len()
    );
    println!(
        "Manifest: {}",
        destination.join("export-manifest.json").display()
    );
    println!("Checksums: {}", destination.join("SHA256SUMS").display());
    Ok(())
}

fn export_table(
    database: &DatabaseCapability,
    table_name: &str,
    columns: &[String],
    root: &Path,
    format: DatabaseExportFormat,
    batch_size: u32,
) -> CliResult<ExportTableRecord> {
    let extension = format.as_str();
    let relative = format!("tables/{table_name}.{extension}");
    let path = root.join(&relative);
    let file = fs::File::create(&path).map_err(|error| {
        format!(
            "failed to create export table '{}': {error}",
            path.display()
        )
    })?;
    let mut writer = BufWriter::new(file);
    let headers = std::iter::once("id".to_string())
        .chain(columns.iter().cloned())
        .chain(["createdAt".into(), "updatedAt".into()])
        .collect::<Vec<String>>();
    if format == DatabaseExportFormat::Json {
        writer
            .write_all(b"[\n")
            .map_err(|error| format!("failed to start JSON export: {error}"))?;
    } else if format == DatabaseExportFormat::Csv {
        write_csv_record(&mut writer, headers.iter().map(String::as_str))?;
    }

    let mut offset = 0_u32;
    let mut row_count = 0_u64;
    let mut first_json = true;
    loop {
        let rows = database
            .list(&DatabaseListQuery {
                table: table_name.to_string(),
                filters: Vec::new(),
                order_by: vec![DatabaseOrder {
                    field: "id".into(),
                    direction: DatabaseOrderDirection::Asc,
                }],
                limit: Some(batch_size),
                offset: Some(offset),
            })
            .map_err(|error| format!("failed to export table '{table_name}': {error}"))?;
        if rows.is_empty() {
            break;
        }
        for row in &rows {
            match format {
                DatabaseExportFormat::Json => {
                    if !first_json {
                        writer
                            .write_all(b",\n")
                            .map_err(|error| format!("failed to continue JSON export: {error}"))?;
                    }
                    serde_json::to_writer(&mut writer, row)
                        .map_err(|error| format!("failed to serialize JSON row: {error}"))?;
                    first_json = false;
                }
                DatabaseExportFormat::Jsonl => {
                    serde_json::to_writer(&mut writer, row)
                        .map_err(|error| format!("failed to serialize JSONL row: {error}"))?;
                    writer
                        .write_all(b"\n")
                        .map_err(|error| format!("failed to continue JSONL export: {error}"))?;
                }
                DatabaseExportFormat::Csv => {
                    let object = row.as_object().ok_or_else(|| {
                        format!("database row in table '{table_name}' is not an object")
                    })?;
                    write_csv_record(
                        &mut writer,
                        headers.iter().map(|header| csv_value(object.get(header))),
                    )?;
                }
            }
            row_count = row_count.saturating_add(1);
        }
        let page = u32::try_from(rows.len())
            .map_err(|_| "export page is larger than the supported offset".to_string())?;
        offset = offset
            .checked_add(page)
            .ok_or_else(|| "export row offset exceeded the supported range".to_string())?;
        if page < batch_size {
            break;
        }
    }
    if format == DatabaseExportFormat::Json {
        writer
            .write_all(b"\n]\n")
            .map_err(|error| format!("failed to finish JSON export: {error}"))?;
    }
    writer
        .flush()
        .map_err(|error| format!("failed to flush table export '{}': {error}", path.display()))?;
    drop(writer);
    let bytes = fs::metadata(&path)
        .map_err(|error| format!("failed to inspect export '{}': {error}", path.display()))?
        .len();
    Ok(ExportTableRecord {
        name: table_name.to_string(),
        file: relative,
        rows: row_count,
        bytes,
        sha256: file_sha256(&path)?,
    })
}

fn write_csv_record<T, I>(writer: &mut impl Write, values: I) -> CliResult<()>
where
    T: AsRef<str>,
    I: IntoIterator<Item = T>,
{
    let mut first = true;
    for value in values {
        let value = value.as_ref();
        if !first {
            writer
                .write_all(b",")
                .map_err(|error| format!("failed to write CSV delimiter: {error}"))?;
        }
        let escaped = value.replace('"', "\"\"");
        if value
            .chars()
            .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
        {
            write!(writer, "\"{escaped}\"")
                .map_err(|error| format!("failed to write CSV field: {error}"))?;
        } else {
            writer
                .write_all(value.as_bytes())
                .map_err(|error| format!("failed to write CSV field: {error}"))?;
        }
        first = false;
    }
    writer
        .write_all(b"\n")
        .map_err(|error| format!("failed to finish CSV record: {error}"))
}

fn csv_value(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(value @ (Value::Bool(_) | Value::Number(_))) => value.to_string(),
        Some(value @ (Value::Array(_) | Value::Object(_))) => value.to_string(),
    }
}

fn file_sha256(path: &Path) -> CliResult<String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("failed to open '{}': {error}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to hash '{}': {error}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn unix_time_ms() -> CliResult<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))
}

fn open_project_database(app: &AppProject, data_dir: PathBuf) -> CliResult<DatabaseCapability> {
    let schema_path = app.app_dir.join(&app.config.database.schema);
    let schema_source = fs::read_to_string(&schema_path).map_err(|error| {
        format!(
            "failed to read database schema '{}': {error}",
            schema_path.display()
        )
    })?;
    let schema = DatabaseSchema::from_json(&schema_source).map_err(|error| {
        format!(
            "invalid database schema '{}': {error}",
            schema_path.display()
        )
    })?;
    DatabaseCapability::open(DatabaseOpenConfig {
        app_id: app.config.app_id.clone(),
        data_dir: Some(data_dir),
        schema,
        migration_files: read_migration_files(app)?,
        seed_files: read_seed_files(app)?,
    })
    .map_err(|error| format!("failed to open project database: {error}"))
}

fn read_seed_files(app: &AppProject) -> CliResult<Vec<DatabaseSeedFile>> {
    list_files_with_extension(&app.app_dir.join(&app.config.database.seeds), "json")?
        .into_iter()
        .map(|path| {
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
            let relative = slash_path(path.strip_prefix(&app.app_dir).unwrap_or(&path));
            DatabaseSeedFile::from_json(relative, &source)
                .map_err(|error| format!("invalid database seed '{}': {error}", path.display()))
        })
        .collect()
}

fn read_migration_files(app: &AppProject) -> CliResult<Vec<DatabaseMigrationFile>> {
    list_files_with_extension(&app.app_dir.join(&app.config.database.migrations), "sql")?
        .into_iter()
        .map(|path| {
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
            let relative = slash_path(path.strip_prefix(&app.app_dir).unwrap_or(&path));
            DatabaseMigrationFile::from_sql(relative, &source).map_err(|error| {
                format!("invalid database migration '{}': {error}", path.display())
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rustframe::{DatabaseCapability, DatabaseOpenConfig, DatabaseSchema};
    use serde_json::json;
    use tempfile::tempdir;

    use super::{export_table, write_csv_record};
    use crate::command::DatabaseExportFormat;

    #[test]
    fn csv_records_escape_delimiters_quotes_and_newlines() {
        let mut output = Vec::new();
        write_csv_record(
            &mut output,
            ["plain", "comma,value", "a \"quote\"", "two\nlines"],
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "plain,\"comma,value\",\"a \"\"quote\"\"\",\"two\nlines\"\n"
        );
    }

    #[test]
    fn table_exports_page_through_json_jsonl_and_csv() {
        let temp = tempdir().unwrap();
        let schema = DatabaseSchema::from_json(
            r#"{"version":1,"tables":[{"name":"items","columns":[{"name":"title","type":"text","required":true},{"name":"meta","type":"json"}]}]}"#,
        )
        .unwrap();
        let database = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "portable-export-test".into(),
            data_dir: Some(temp.path().join("data")),
            schema,
            migration_files: Vec::new(),
            seed_files: Vec::new(),
        })
        .unwrap();
        database
            .insert("items", json!({"title": "one, quoted", "meta": {"n": 1}}))
            .unwrap();
        database
            .insert("items", json!({"title": "two", "meta": ["local"]}))
            .unwrap();
        let columns = vec!["title".into(), "meta".into()];

        for format in [
            DatabaseExportFormat::Json,
            DatabaseExportFormat::Jsonl,
            DatabaseExportFormat::Csv,
        ] {
            let root = temp.path().join(format.as_str());
            fs::create_dir_all(root.join("tables")).unwrap();
            let record = export_table(&database, "items", &columns, &root, format, 1).unwrap();
            assert_eq!(record.rows, 2);
            assert_eq!(record.sha256.len(), 64);
            let source = fs::read_to_string(root.join(record.file)).unwrap();
            match format {
                DatabaseExportFormat::Json => {
                    assert_eq!(
                        serde_json::from_str::<serde_json::Value>(&source)
                            .unwrap()
                            .as_array()
                            .unwrap()
                            .len(),
                        2
                    );
                }
                DatabaseExportFormat::Jsonl => assert_eq!(source.lines().count(), 2),
                DatabaseExportFormat::Csv => {
                    assert_eq!(source.lines().count(), 3);
                    assert!(source.contains("\"one, quoted\""));
                }
            }
        }
    }

    #[test]
    fn portable_export_reads_rows_after_an_additive_schema_upgrade() {
        let temp = tempdir().unwrap();
        let data_dir = temp.path().join("data");
        let v1 = DatabaseSchema::from_json(
            r#"{"version":1,"tables":[{"name":"items","columns":[{"name":"title","type":"text","required":true}]}]}"#,
        )
        .unwrap();
        let database = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "portable-upgrade-test".into(),
            data_dir: Some(data_dir.clone()),
            schema: v1,
            migration_files: Vec::new(),
            seed_files: Vec::new(),
        })
        .unwrap();
        database
            .insert("items", json!({"title": "kept through upgrade"}))
            .unwrap();
        drop(database);

        let v2 = DatabaseSchema::from_json(
            r#"{"version":2,"tables":[{"name":"items","columns":[{"name":"title","type":"text","required":true},{"name":"status","type":"text","required":true,"default":"queued"}]}]}"#,
        )
        .unwrap();
        let database = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "portable-upgrade-test".into(),
            data_dir: Some(data_dir),
            schema: v2,
            migration_files: Vec::new(),
            seed_files: Vec::new(),
        })
        .unwrap();
        let root = temp.path().join("export");
        fs::create_dir_all(root.join("tables")).unwrap();
        let record = export_table(
            &database,
            "items",
            &["title".into(), "status".into()],
            &root,
            DatabaseExportFormat::Json,
            1,
        )
        .unwrap();
        let rows: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join(record.file)).unwrap()).unwrap();
        assert_eq!(rows[0]["title"], "kept through upgrade");
        assert_eq!(rows[0]["status"], "queued");
    }
}
