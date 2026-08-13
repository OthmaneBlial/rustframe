use std::{
    fs,
    path::{Path, PathBuf},
};

use rustframe::{
    DatabaseCapability, DatabaseMigrationFile, DatabaseOpenConfig, DatabaseSchema, DatabaseSeedFile,
};

use super::{
    AppProject, CliResult, DATABASE_FILE_NAME, default_app_data_dir, list_files_with_extension,
    load_app_project, print_capability_warnings, slash_path,
};

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
