use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{
    Connection, OptionalExtension, params,
    types::{Value as SqlValue, ValueRef},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{Result, RuntimeError};

const META_TABLE: &str = "__rustframe_meta";
const APPLIED_SEEDS_TABLE: &str = "__rustframe_applied_seeds";
const APPLIED_MIGRATIONS_TABLE: &str = "__rustframe_applied_migrations";
const SCHEMA_VERSION_KEY: &str = "schema_version";
const SCHEMA_CHECKSUM_KEY: &str = "schema_checksum";
const DATABASE_FILE_NAME: &str = "app.db";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DatabaseSchema {
    pub version: u32,
    pub tables: Vec<DatabaseTable>,
}

impl DatabaseSchema {
    pub fn from_json(source: &str) -> Result<Self> {
        let schema: Self = serde_json::from_str(source)?;
        schema.validate()?;
        Ok(schema)
    }

    fn validate(&self) -> Result<()> {
        if self.version == 0 {
            return Err(RuntimeError::InvalidConfiguration(
                "database schema version must be greater than zero".into(),
            ));
        }

        if self.tables.is_empty() {
            return Err(RuntimeError::InvalidConfiguration(
                "database schema must define at least one table".into(),
            ));
        }

        let mut seen_tables = BTreeSet::new();
        for table in &self.tables {
            validate_identifier(&table.name, "table name")?;
            if !seen_tables.insert(table.name.as_str()) {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "database schema defines table '{}' more than once",
                    table.name
                )));
            }

            table.validate()?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DatabaseTable {
    pub name: String,
    pub columns: Vec<DatabaseColumn>,
    #[serde(default)]
    pub indexes: Vec<DatabaseIndex>,
}

impl DatabaseTable {
    fn validate(&self) -> Result<()> {
        if self.columns.is_empty() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "table '{}' must define at least one column",
                self.name
            )));
        }

        let mut seen_columns = BTreeSet::new();
        for column in &self.columns {
            validate_identifier(&column.name, "column name")?;
            validate_reserved_column_name(&column.name)?;
            if !seen_columns.insert(column.name.as_str()) {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "table '{}' defines column '{}' more than once",
                    self.name, column.name
                )));
            }

            column.validate(&self.name)?;
        }

        let known_columns = seen_columns;
        for index in &self.indexes {
            index.validate(&self.name, &known_columns)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DatabaseColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: DatabaseColumnType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub default: Option<Value>,
}

impl DatabaseColumn {
    fn validate(&self, table_name: &str) -> Result<()> {
        if let Some(default) = &self.default {
            validate_value_for_type(default, &self.kind, false).map_err(|error| {
                RuntimeError::InvalidConfiguration(format!(
                    "invalid default for '{}.{}': {}",
                    table_name, self.name, error
                ))
            })?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseColumnType {
    Text,
    Integer,
    Real,
    Boolean,
    Json,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DatabaseIndex {
    pub name: Option<String>,
    pub columns: Vec<String>,
    #[serde(default)]
    pub unique: bool,
}

impl DatabaseIndex {
    fn validate(&self, table_name: &str, known_columns: &BTreeSet<&str>) -> Result<()> {
        if self.columns.is_empty() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "table '{}' defines an index with no columns",
                table_name
            )));
        }

        if let Some(name) = &self.name {
            validate_identifier(name, "index name")?;
        }

        for column in &self.columns {
            if !known_columns.contains(column.as_str()) {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "index on table '{}' references unknown column '{}'",
                    table_name, column
                )));
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseSeedFile {
    pub path: String,
    pub checksum: String,
    pub entries: Vec<DatabaseSeedEntry>,
}

impl DatabaseSeedFile {
    pub fn from_json(path: impl Into<String>, source: &str) -> Result<Self> {
        let path = path.into();
        let manifest: DatabaseSeedManifest = serde_json::from_str(source)?;
        manifest.validate(&path)?;

        Ok(Self {
            checksum: hex_sha256(source.as_bytes()),
            entries: manifest.entries,
            path,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
struct DatabaseSeedManifest {
    entries: Vec<DatabaseSeedEntry>,
}

impl DatabaseSeedManifest {
    fn validate(&self, path: &str) -> Result<()> {
        if self.entries.is_empty() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "seed file '{}' must define at least one entry",
                path
            )));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseSeedEntry {
    pub table: String,
    pub rows: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct DatabaseMigrationFile {
    pub path: String,
    pub version: u32,
    pub checksum: String,
    pub sql: String,
}

impl DatabaseMigrationFile {
    pub fn from_sql(path: impl Into<String>, source: &str) -> Result<Self> {
        let path = path.into();
        let version = migration_version_from_path(&path)?;
        if source.trim().is_empty() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "migration file '{}' must not be empty",
                path
            )));
        }

        Ok(Self {
            path,
            version,
            checksum: hex_sha256(source.as_bytes()),
            sql: source.to_string(),
        })
    }
}

fn migration_version_from_path(path: &str) -> Result<u32> {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            RuntimeError::InvalidConfiguration(format!(
                "migration path '{}' must include a UTF-8 file name",
                path
            ))
        })?;

    let digits = file_name
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();

    if digits.is_empty() {
        return Err(RuntimeError::InvalidConfiguration(format!(
            "migration file '{}' must start with a numeric version prefix",
            path
        )));
    }

    let version = digits.parse::<u32>().map_err(|error| {
        RuntimeError::InvalidConfiguration(format!(
            "migration file '{}' has an invalid version prefix: {}",
            path, error
        ))
    })?;

    if version == 0 {
        return Err(RuntimeError::InvalidConfiguration(format!(
            "migration file '{}' must start with a version greater than zero",
            path
        )));
    }

    Ok(version)
}

#[derive(Clone, Debug)]
pub struct DatabaseOpenConfig {
    pub app_id: String,
    pub data_dir: Option<PathBuf>,
    pub schema: DatabaseSchema,
    pub migration_files: Vec<DatabaseMigrationFile>,
    pub seed_files: Vec<DatabaseSeedFile>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub app_id: String,
    pub data_dir: String,
    pub database_path: String,
    pub schema_version: u32,
    pub tables: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseListQuery {
    pub table: String,
    #[serde(default)]
    pub filters: Vec<DatabaseFilter>,
    #[serde(default)]
    pub order_by: Vec<DatabaseOrder>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseFilter {
    pub field: String,
    #[serde(default)]
    pub op: DatabaseFilterOp,
    pub value: Value,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseFilterOp {
    #[default]
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Like,
    In,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseOrder {
    pub field: String,
    #[serde(default)]
    pub direction: DatabaseOrderDirection,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseOrderDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug)]
pub struct DatabaseCapability {
    info: DatabaseInfo,
    tables: BTreeMap<String, TablePlan>,
    connection: RefCell<Connection>,
}

impl DatabaseCapability {
    pub fn open(config: DatabaseOpenConfig) -> Result<Self> {
        validate_app_id(&config.app_id)?;
        config.schema.validate()?;

        let data_dir = match config.data_dir {
            Some(path) => path,
            None => default_app_data_dir(&config.app_id).map_err(|error| {
                RuntimeError::InvalidConfiguration(format!(
                    "could not resolve app data directory for '{}': {error}",
                    config.app_id
                ))
            })?,
        };
        fs::create_dir_all(&data_dir)?;

        let database_path = data_dir.join(DATABASE_FILE_NAME);
        let connection = Connection::open(&database_path)?;
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;

        let tables = build_table_plans(&config.schema)?;

        initialize_meta_tables(&connection)?;
        apply_schema(
            &connection,
            &config.schema,
            &tables,
            &config.migration_files,
        )?;
        apply_seed_files(&connection, &tables, &config.seed_files)?;

        let info = DatabaseInfo {
            app_id: config.app_id,
            data_dir: data_dir.to_string_lossy().into_owned(),
            database_path: database_path.to_string_lossy().into_owned(),
            schema_version: config.schema.version,
            tables: tables.keys().cloned().collect(),
        };

        Ok(Self {
            info,
            tables,
            connection: RefCell::new(connection),
        })
    }

    pub fn info(&self) -> &DatabaseInfo {
        &self.info
    }

    pub fn get(&self, table: &str, id: i64) -> Result<Option<Value>> {
        let table = self.table(table)?;
        let select = select_columns_sql(table);
        let sql = format!(
            "SELECT {select} FROM {} WHERE {} = ?1 LIMIT 1",
            quote_identifier(&table.name),
            quote_identifier("id")
        );

        let connection = self.connection.borrow();
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query(params![id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };

        decode_row(row, table).map(Some)
    }

    pub fn list(&self, query: &DatabaseListQuery) -> Result<Vec<Value>> {
        let table = self.table(&query.table)?;
        let mut values = Vec::new();
        let where_sql = build_where_clause(table, &query.filters, &mut values)?;
        let order_sql = build_order_clause(table, &query.order_by)?;
        let limit_sql = build_limit_clause(query.limit, query.offset, &mut values)?;
        let select = select_columns_sql(table);
        let sql = format!(
            "SELECT {select} FROM {}{where_sql}{order_sql}{limit_sql}",
            quote_identifier(&table.name)
        );

        let connection = self.connection.borrow();
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query(rusqlite::params_from_iter(values))?;
        let mut records = Vec::new();

        while let Some(row) = rows.next()? {
            records.push(decode_row(row, table)?);
        }

        Ok(records)
    }

    pub fn count(&self, query: &DatabaseListQuery) -> Result<u64> {
        let table = self.table(&query.table)?;
        let mut values = Vec::new();
        let where_sql = build_where_clause(table, &query.filters, &mut values)?;
        let sql = format!(
            "SELECT COUNT(*) FROM {}{where_sql}",
            quote_identifier(&table.name)
        );

        let connection = self.connection.borrow();
        connection
            .query_row(&sql, rusqlite::params_from_iter(values), |row| {
                row.get::<_, u64>(0)
            })
            .map_err(Into::into)
    }

    pub fn insert(&self, table: &str, record: Value) -> Result<Value> {
        let table = self.table(table)?;
        let timestamp = now_timestamp()?;
        let record = record_object(record, "record")?;
        let mut values = Vec::new();
        let mut columns = Vec::new();

        columns.push("created_at".to_string());
        values.push(SqlValue::Text(timestamp.clone()));
        columns.push("updated_at".to_string());
        values.push(SqlValue::Text(timestamp.clone()));

        for column in &table.columns {
            columns.push(column.name.clone());
            values.push(insert_value_for_column(column, &record)?);
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_identifier(&table.name),
            columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", "),
            (0..values.len())
                .map(sql_placeholder)
                .collect::<Vec<_>>()
                .join(", ")
        );

        let connection = self.connection.borrow_mut();
        connection.execute(&sql, rusqlite::params_from_iter(values))?;
        let id = connection.last_insert_rowid();
        drop(connection);

        self.get(&table.name, id)?.ok_or_else(|| {
            RuntimeError::RecordNotFound(format!("inserted row '{}' was not found", id))
        })
    }

    pub fn update(&self, table: &str, id: i64, patch: Value) -> Result<Value> {
        let table = self.table(table)?;
        let patch = record_object(patch, "patch")?;
        if patch.is_empty() {
            return Err(RuntimeError::InvalidParameter(
                "patch must contain at least one field".into(),
            ));
        }

        let mut assignments = Vec::new();
        let mut values = Vec::new();

        for column in &table.columns {
            if let Some(value) = patch.get(&column.name) {
                assignments.push(format!(
                    "{} = {}",
                    quote_identifier(&column.name),
                    sql_placeholder(values.len())
                ));
                values.push(update_value_for_column(column, value)?);
            }
        }

        let reserved = ["id", "createdAt", "updatedAt", "created_at", "updated_at"];
        for key in patch.keys() {
            if reserved.contains(&key.as_str()) {
                return Err(RuntimeError::InvalidParameter(format!(
                    "field '{}' is managed by RustFrame and cannot be updated directly",
                    key
                )));
            }

            if !table.columns_by_name.contains_key(key) {
                return Err(RuntimeError::InvalidParameter(format!(
                    "table '{}' has no column named '{}'",
                    table.name, key
                )));
            }
        }

        if assignments.is_empty() {
            return Err(RuntimeError::InvalidParameter(
                "patch must contain at least one updatable field".into(),
            ));
        }

        assignments.push(format!(
            "{} = {}",
            quote_identifier("updated_at"),
            sql_placeholder(values.len())
        ));
        values.push(SqlValue::Text(now_timestamp()?));
        values.push(SqlValue::Integer(id));

        let sql = format!(
            "UPDATE {} SET {} WHERE {} = {}",
            quote_identifier(&table.name),
            assignments.join(", "),
            quote_identifier("id"),
            sql_placeholder(values.len() - 1)
        );

        let connection = self.connection.borrow_mut();
        let changed = connection.execute(&sql, rusqlite::params_from_iter(values))?;
        drop(connection);

        if changed == 0 {
            return Err(RuntimeError::RecordNotFound(format!(
                "table '{}' has no record with id {}",
                table.name, id
            )));
        }

        self.get(&table.name, id)?.ok_or_else(|| {
            RuntimeError::RecordNotFound(format!(
                "table '{}' has no record with id {}",
                table.name, id
            ))
        })
    }

    pub fn delete(&self, table: &str, id: i64) -> Result<bool> {
        let table = self.table(table)?;
        let sql = format!(
            "DELETE FROM {} WHERE {} = ?1",
            quote_identifier(&table.name),
            quote_identifier("id")
        );

        let connection = self.connection.borrow_mut();
        let changed = connection.execute(&sql, params![id])?;
        Ok(changed > 0)
    }

    fn table(&self, table: &str) -> Result<&TablePlan> {
        self.tables.get(table).ok_or_else(|| {
            RuntimeError::InvalidParameter(format!(
                "database schema has no table named '{}'",
                table
            ))
        })
    }
}

#[derive(Debug)]
struct TablePlan {
    name: String,
    columns: Vec<ColumnPlan>,
    columns_by_name: BTreeMap<String, usize>,
}

#[derive(Debug)]
struct ColumnPlan {
    name: String,
    kind: DatabaseColumnType,
    required: bool,
    default: Option<Value>,
}

fn build_table_plans(schema: &DatabaseSchema) -> Result<BTreeMap<String, TablePlan>> {
    let mut tables = BTreeMap::new();

    for table in &schema.tables {
        let mut columns_by_name = BTreeMap::new();
        let mut columns = Vec::new();
        for (index, column) in table.columns.iter().enumerate() {
            columns_by_name.insert(column.name.clone(), index);
            columns.push(ColumnPlan {
                name: column.name.clone(),
                kind: column.kind.clone(),
                required: column.required,
                default: column.default.clone(),
            });
        }

        tables.insert(
            table.name.clone(),
            TablePlan {
                name: table.name.clone(),
                columns,
                columns_by_name,
            },
        );
    }

    Ok(tables)
}

fn initialize_meta_tables(connection: &Connection) -> Result<()> {
    connection.execute_batch(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {meta_table} (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS {seed_table} (
            path TEXT PRIMARY KEY,
            checksum TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS {migration_table} (
            version INTEGER PRIMARY KEY,
            path TEXT NOT NULL,
            checksum TEXT NOT NULL
        );
        "#,
        meta_table = quote_identifier(META_TABLE),
        seed_table = quote_identifier(APPLIED_SEEDS_TABLE),
        migration_table = quote_identifier(APPLIED_MIGRATIONS_TABLE),
    ))?;

    Ok(())
}

fn apply_schema(
    connection: &Connection,
    schema: &DatabaseSchema,
    tables: &BTreeMap<String, TablePlan>,
    migration_files: &[DatabaseMigrationFile],
) -> Result<()> {
    let schema_checksum = hex_sha256(&serde_json::to_vec(schema)?);
    let migrations_by_version = validate_migration_files(schema.version, migration_files)?;
    let stored_version = meta_value(connection, SCHEMA_VERSION_KEY)?
        .map(|value| value.parse::<u32>())
        .transpose()
        .map_err(|error| {
            RuntimeError::InvalidConfiguration(format!("invalid stored schema version: {error}"))
        })?;

    let stored_checksum = meta_value(connection, SCHEMA_CHECKSUM_KEY)?;
    validate_applied_migration_checksums(connection, &migrations_by_version)?;

    match stored_version {
        None => {
            for table in &schema.tables {
                create_table(connection, table)?;
                ensure_indexes(connection, table)?;
            }
        }
        Some(version) => {
            if version > schema.version {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "database on disk is at schema version {}, but embedded schema version is {}",
                    version, schema.version
                )));
            }

            if version < schema.version {
                apply_migration_files(connection, &migrations_by_version, version, schema.version)?;
            }

            for table in &schema.tables {
                if !table_exists(connection, &table.name)? {
                    create_table(connection, table)?;
                } else {
                    reconcile_table(
                        connection,
                        table,
                        tables.get(&table.name).expect("validated table"),
                    )?;
                }

                ensure_indexes(connection, table)?;
            }

            if version == schema.version
                && stored_checksum.as_deref() == Some(schema_checksum.as_str())
            {
                return Ok(());
            }
        }
    }

    set_meta_value(connection, SCHEMA_VERSION_KEY, schema.version.to_string())?;
    set_meta_value(connection, SCHEMA_CHECKSUM_KEY, schema_checksum)?;
    Ok(())
}

fn apply_seed_files(
    connection: &Connection,
    tables: &BTreeMap<String, TablePlan>,
    seed_files: &[DatabaseSeedFile],
) -> Result<()> {
    for seed in seed_files {
        let existing = connection
            .query_row(
                &format!(
                    "SELECT checksum FROM {} WHERE path = ?1",
                    quote_identifier(APPLIED_SEEDS_TABLE)
                ),
                params![seed.path],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(checksum) = existing {
            if checksum == seed.checksum {
                continue;
            }

            return Err(RuntimeError::InvalidConfiguration(format!(
                "seed file '{}' changed after it had already been applied; create a new versioned seed file or move the data change into data/migrations/*.sql",
                seed.path
            )));
        }

        let transaction = connection.unchecked_transaction()?;
        for entry in &seed.entries {
            let table = tables.get(&entry.table).ok_or_else(|| {
                RuntimeError::InvalidConfiguration(format!(
                    "seed file '{}' references unknown table '{}'",
                    seed.path, entry.table
                ))
            })?;

            for row in &entry.rows {
                insert_with_connection(&transaction, table, row.clone())?;
            }
        }

        transaction.execute(
            &format!(
                "INSERT INTO {} (path, checksum) VALUES (?1, ?2)",
                quote_identifier(APPLIED_SEEDS_TABLE)
            ),
            params![seed.path, seed.checksum],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

fn validate_migration_files<'a>(
    schema_version: u32,
    migration_files: &'a [DatabaseMigrationFile],
) -> Result<BTreeMap<u32, &'a DatabaseMigrationFile>> {
    let mut migrations = BTreeMap::new();

    for migration in migration_files {
        if migration.version > schema_version {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "migration '{}' targets version {}, but schema version is {}",
                migration.path, migration.version, schema_version
            )));
        }

        if migrations.insert(migration.version, migration).is_some() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "migration version {} is defined more than once",
                migration.version
            )));
        }
    }

    Ok(migrations)
}

fn validate_applied_migration_checksums(
    connection: &Connection,
    migrations_by_version: &BTreeMap<u32, &DatabaseMigrationFile>,
) -> Result<()> {
    let mut statement = connection.prepare(&format!(
        "SELECT version, path, checksum FROM {} ORDER BY version",
        quote_identifier(APPLIED_MIGRATIONS_TABLE)
    ))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, u32>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for row in rows {
        let (version, path, checksum) = row?;
        let Some(current) = migrations_by_version.get(&version) else {
            continue;
        };

        if current.checksum != checksum {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "migration file '{}' changed after version {} had already been applied (stored as '{}')",
                current.path, version, path
            )));
        }
    }

    Ok(())
}

fn apply_migration_files(
    connection: &Connection,
    migrations_by_version: &BTreeMap<u32, &DatabaseMigrationFile>,
    from_version: u32,
    to_version: u32,
) -> Result<()> {
    for version in (from_version + 1)..=to_version {
        let Some(migration) = migrations_by_version.get(&version) else {
            continue;
        };

        let existing = connection
            .query_row(
                &format!(
                    "SELECT checksum FROM {} WHERE version = ?1",
                    quote_identifier(APPLIED_MIGRATIONS_TABLE)
                ),
                params![version],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(checksum) = existing {
            if checksum == migration.checksum {
                continue;
            }

            return Err(RuntimeError::InvalidConfiguration(format!(
                "migration file '{}' changed after version {} had already been applied",
                migration.path, version
            )));
        }

        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(&migration.sql)?;
        transaction.execute(
            &format!(
                "INSERT INTO {} (version, path, checksum) VALUES (?1, ?2, ?3)",
                quote_identifier(APPLIED_MIGRATIONS_TABLE)
            ),
            params![version, migration.path, migration.checksum],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

fn create_table(connection: &Connection, table: &DatabaseTable) -> Result<()> {
    let mut definitions = vec![
        format!(
            "{} INTEGER PRIMARY KEY AUTOINCREMENT",
            quote_identifier("id")
        ),
        format!("{} TEXT NOT NULL", quote_identifier("created_at")),
        format!("{} TEXT NOT NULL", quote_identifier("updated_at")),
    ];

    for column in &table.columns {
        definitions.push(column_definition(column));
    }

    let sql = format!(
        "CREATE TABLE IF NOT EXISTS {} ({})",
        quote_identifier(&table.name),
        definitions.join(", ")
    );
    connection.execute_batch(&sql)?;
    Ok(())
}

fn reconcile_table(connection: &Connection, table: &DatabaseTable, plan: &TablePlan) -> Result<()> {
    let existing_columns = existing_columns(connection, &table.name)?;

    for column in &plan.columns {
        if existing_columns.contains(column.name.as_str()) {
            continue;
        }

        if column.required && column.default.is_none() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "table '{}' cannot add required column '{}' without a default value",
                table.name, column.name
            )));
        }

        let sql = format!(
            "ALTER TABLE {} ADD COLUMN {}",
            quote_identifier(&table.name),
            column_definition_from_plan(column)
        );
        connection.execute_batch(&sql)?;
    }

    Ok(())
}

fn ensure_indexes(connection: &Connection, table: &DatabaseTable) -> Result<()> {
    for (position, index) in table.indexes.iter().enumerate() {
        let name = index
            .name
            .clone()
            .unwrap_or_else(|| format!("idx_{}_{}", table.name, position + 1));
        let sql = format!(
            "CREATE {} INDEX IF NOT EXISTS {} ON {} ({})",
            if index.unique { "UNIQUE" } else { "" },
            quote_identifier(&name),
            quote_identifier(&table.name),
            index
                .columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .replace("CREATE  INDEX", "CREATE INDEX");

        connection.execute_batch(&sql)?;
    }

    Ok(())
}

fn column_definition(column: &DatabaseColumn) -> String {
    column_definition_parts(
        &column.name,
        &column.kind,
        column.required,
        column.unique,
        column.default.as_ref(),
    )
}

fn column_definition_from_plan(column: &ColumnPlan) -> String {
    column_definition_parts(
        &column.name,
        &column.kind,
        column.required,
        false,
        column.default.as_ref(),
    )
}

fn column_definition_parts(
    name: &str,
    kind: &DatabaseColumnType,
    required: bool,
    unique: bool,
    default: Option<&Value>,
) -> String {
    let mut parts = vec![quote_identifier(name), sqlite_type(kind).into()];
    if required {
        parts.push("NOT NULL".into());
    }
    if unique {
        parts.push("UNIQUE".into());
    }
    if let Some(default) = default {
        parts.push(format!("DEFAULT {}", sql_default_literal(default, kind)));
    }
    parts.join(" ")
}

fn insert_with_connection(connection: &Connection, table: &TablePlan, record: Value) -> Result<()> {
    let timestamp = now_timestamp()?;
    let record = record_object(record, "record")?;
    let mut columns = vec!["created_at".to_string(), "updated_at".to_string()];
    let mut values = vec![SqlValue::Text(timestamp.clone()), SqlValue::Text(timestamp)];

    for column in &table.columns {
        columns.push(column.name.clone());
        values.push(insert_value_for_column(column, &record)?);
    }

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_identifier(&table.name),
        columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", "),
        (0..values.len())
            .map(sql_placeholder)
            .collect::<Vec<_>>()
            .join(", ")
    );
    connection.execute(&sql, rusqlite::params_from_iter(values))?;
    Ok(())
}

fn build_where_clause(
    table: &TablePlan,
    filters: &[DatabaseFilter],
    values: &mut Vec<SqlValue>,
) -> Result<String> {
    if filters.is_empty() {
        return Ok(String::new());
    }

    let mut clauses = Vec::new();
    for filter in filters {
        let column = filter_column_sql(table, &filter.field)?;
        match filter.op {
            DatabaseFilterOp::Eq => {
                clauses.push(format!("{column} = {}", sql_placeholder(values.len())));
                values.push(filter_value(table, &filter.field, &filter.value)?);
            }
            DatabaseFilterOp::Ne => {
                clauses.push(format!("{column} != {}", sql_placeholder(values.len())));
                values.push(filter_value(table, &filter.field, &filter.value)?);
            }
            DatabaseFilterOp::Lt => {
                clauses.push(format!("{column} < {}", sql_placeholder(values.len())));
                values.push(filter_value(table, &filter.field, &filter.value)?);
            }
            DatabaseFilterOp::Lte => {
                clauses.push(format!("{column} <= {}", sql_placeholder(values.len())));
                values.push(filter_value(table, &filter.field, &filter.value)?);
            }
            DatabaseFilterOp::Gt => {
                clauses.push(format!("{column} > {}", sql_placeholder(values.len())));
                values.push(filter_value(table, &filter.field, &filter.value)?);
            }
            DatabaseFilterOp::Gte => {
                clauses.push(format!("{column} >= {}", sql_placeholder(values.len())));
                values.push(filter_value(table, &filter.field, &filter.value)?);
            }
            DatabaseFilterOp::Like => {
                let value = filter
                    .value
                    .as_str()
                    .ok_or_else(|| {
                        RuntimeError::InvalidParameter(format!(
                            "filter '{}' with op 'like' requires a string value",
                            filter.field
                        ))
                    })?
                    .to_string();
                clauses.push(format!("{column} LIKE {}", sql_placeholder(values.len())));
                values.push(SqlValue::Text(value));
            }
            DatabaseFilterOp::In => {
                let items = filter.value.as_array().ok_or_else(|| {
                    RuntimeError::InvalidParameter(format!(
                        "filter '{}' with op 'in' requires an array value",
                        filter.field
                    ))
                })?;

                if items.is_empty() {
                    return Err(RuntimeError::InvalidParameter(format!(
                        "filter '{}' with op 'in' requires a non-empty array",
                        filter.field
                    )));
                }

                let mut placeholders = Vec::new();
                for item in items {
                    placeholders.push(sql_placeholder(values.len()));
                    values.push(filter_value(table, &filter.field, item)?);
                }
                clauses.push(format!("{column} IN ({})", placeholders.join(", ")));
            }
        }
    }

    Ok(format!(" WHERE {}", clauses.join(" AND ")))
}

fn build_order_clause(table: &TablePlan, order_by: &[DatabaseOrder]) -> Result<String> {
    if order_by.is_empty() {
        return Ok(format!(" ORDER BY {} DESC", quote_identifier("updated_at")));
    }

    let mut segments = Vec::new();
    for order in order_by {
        let column = filter_column_sql(table, &order.field)?;
        let direction = match order.direction {
            DatabaseOrderDirection::Asc => "ASC",
            DatabaseOrderDirection::Desc => "DESC",
        };
        segments.push(format!("{column} {direction}"));
    }

    Ok(format!(" ORDER BY {}", segments.join(", ")))
}

fn build_limit_clause(
    limit: Option<u32>,
    offset: Option<u32>,
    values: &mut Vec<SqlValue>,
) -> Result<String> {
    let Some(limit) = limit else {
        if offset.is_some() {
            return Err(RuntimeError::InvalidParameter(
                "offset requires limit to also be set".into(),
            ));
        }
        return Ok(String::new());
    };

    values.push(SqlValue::Integer(i64::from(limit)));
    let mut clause = format!(" LIMIT {}", sql_placeholder(values.len() - 1));
    if let Some(offset) = offset {
        values.push(SqlValue::Integer(i64::from(offset)));
        clause.push_str(&format!(" OFFSET {}", sql_placeholder(values.len() - 1)));
    }

    Ok(clause)
}

fn filter_value(table: &TablePlan, field: &str, value: &Value) -> Result<SqlValue> {
    if field == "id" {
        return sql_value_from_json(value, &DatabaseColumnType::Integer, true);
    }

    if field == "createdAt" || field == "updatedAt" {
        return sql_value_from_json(value, &DatabaseColumnType::Text, true);
    }

    let column = table
        .columns_by_name
        .get(field)
        .and_then(|index| table.columns.get(*index))
        .ok_or_else(|| {
            RuntimeError::InvalidParameter(format!(
                "table '{}' has no column named '{}'",
                table.name, field
            ))
        })?;

    sql_value_from_json(value, &column.kind, !column.required)
}

fn filter_column_sql(table: &TablePlan, field: &str) -> Result<String> {
    match field {
        "id" => Ok(quote_identifier("id")),
        "createdAt" => Ok(quote_identifier("created_at")),
        "updatedAt" => Ok(quote_identifier("updated_at")),
        _ => {
            if table.columns_by_name.contains_key(field) {
                Ok(quote_identifier(field))
            } else {
                Err(RuntimeError::InvalidParameter(format!(
                    "table '{}' has no column named '{}'",
                    table.name, field
                )))
            }
        }
    }
}

fn decode_row(row: &rusqlite::Row<'_>, table: &TablePlan) -> Result<Value> {
    let mut object = Map::new();
    object.insert("id".into(), json!(row.get::<_, i64>(0)?));
    object.insert("createdAt".into(), Value::String(row.get::<_, String>(1)?));
    object.insert("updatedAt".into(), Value::String(row.get::<_, String>(2)?));

    for (index, column) in table.columns.iter().enumerate() {
        let value_index = index + 3;
        let value = value_ref_to_json(row.get_ref(value_index)?, &column.kind)?;
        object.insert(column.name.clone(), value);
    }

    Ok(Value::Object(object))
}

fn value_ref_to_json(value: ValueRef<'_>, kind: &DatabaseColumnType) -> Result<Value> {
    match kind {
        DatabaseColumnType::Text => match value {
            ValueRef::Null => Ok(Value::Null),
            ValueRef::Text(bytes) => Ok(Value::String(String::from_utf8_lossy(bytes).into_owned())),
            _ => Err(RuntimeError::InvalidConfiguration(
                "database returned a non-text value for a text column".into(),
            )),
        },
        DatabaseColumnType::Integer => match value {
            ValueRef::Null => Ok(Value::Null),
            ValueRef::Integer(number) => Ok(json!(number)),
            _ => Err(RuntimeError::InvalidConfiguration(
                "database returned a non-integer value for an integer column".into(),
            )),
        },
        DatabaseColumnType::Real => match value {
            ValueRef::Null => Ok(Value::Null),
            ValueRef::Real(number) => Ok(json!(number)),
            ValueRef::Integer(number) => Ok(json!(number)),
            _ => Err(RuntimeError::InvalidConfiguration(
                "database returned a non-real value for a real column".into(),
            )),
        },
        DatabaseColumnType::Boolean => match value {
            ValueRef::Null => Ok(Value::Null),
            ValueRef::Integer(number) => Ok(Value::Bool(number != 0)),
            _ => Err(RuntimeError::InvalidConfiguration(
                "database returned a non-boolean value for a boolean column".into(),
            )),
        },
        DatabaseColumnType::Json => match value {
            ValueRef::Null => Ok(Value::Null),
            ValueRef::Text(bytes) => serde_json::from_slice(bytes).map_err(Into::into),
            _ => Err(RuntimeError::InvalidConfiguration(
                "database returned a non-json value for a json column".into(),
            )),
        },
    }
}

fn record_object(value: Value, label: &str) -> Result<Map<String, Value>> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidParameter(format!("{label} must be a JSON object")))
}

fn insert_value_for_column(column: &ColumnPlan, record: &Map<String, Value>) -> Result<SqlValue> {
    match record.get(&column.name) {
        Some(value) => sql_value_from_json(value, &column.kind, !column.required),
        None => match &column.default {
            Some(default) => sql_value_from_json(default, &column.kind, !column.required),
            None if column.required => Err(RuntimeError::InvalidParameter(format!(
                "missing required field '{}'",
                column.name
            ))),
            None => Ok(SqlValue::Null),
        },
    }
}

fn update_value_for_column(column: &ColumnPlan, value: &Value) -> Result<SqlValue> {
    sql_value_from_json(value, &column.kind, !column.required)
}

fn sql_value_from_json(
    value: &Value,
    kind: &DatabaseColumnType,
    nullable: bool,
) -> Result<SqlValue> {
    validate_value_for_type(value, kind, nullable)?;

    match kind {
        DatabaseColumnType::Text => {
            if value.is_null() {
                Ok(SqlValue::Null)
            } else {
                Ok(SqlValue::Text(value.as_str().unwrap().to_string()))
            }
        }
        DatabaseColumnType::Integer => {
            if value.is_null() {
                Ok(SqlValue::Null)
            } else if let Some(number) = value.as_i64() {
                Ok(SqlValue::Integer(number))
            } else {
                Ok(SqlValue::Integer(
                    i64::try_from(value.as_u64().unwrap()).map_err(|_| {
                        RuntimeError::InvalidParameter("integer value is out of range".into())
                    })?,
                ))
            }
        }
        DatabaseColumnType::Real => {
            if value.is_null() {
                Ok(SqlValue::Null)
            } else {
                Ok(SqlValue::Real(value.as_f64().unwrap()))
            }
        }
        DatabaseColumnType::Boolean => {
            if value.is_null() {
                Ok(SqlValue::Null)
            } else {
                Ok(SqlValue::Integer(if value.as_bool().unwrap() {
                    1
                } else {
                    0
                }))
            }
        }
        DatabaseColumnType::Json => {
            if value.is_null() {
                Ok(SqlValue::Null)
            } else {
                Ok(SqlValue::Text(serde_json::to_string(value)?))
            }
        }
    }
}

fn validate_value_for_type(value: &Value, kind: &DatabaseColumnType, nullable: bool) -> Result<()> {
    if value.is_null() {
        if nullable {
            return Ok(());
        }

        return Err(RuntimeError::InvalidParameter(
            "null is not allowed for this value".into(),
        ));
    }

    let valid = match kind {
        DatabaseColumnType::Text => value.is_string(),
        DatabaseColumnType::Integer => value.is_i64() || value.is_u64(),
        DatabaseColumnType::Real => value.is_f64() || value.is_i64() || value.is_u64(),
        DatabaseColumnType::Boolean => value.is_boolean(),
        DatabaseColumnType::Json => true,
    };

    if valid {
        Ok(())
    } else {
        Err(RuntimeError::InvalidParameter(format!(
            "value '{}' does not match the expected {:?} type",
            value, kind
        )))
    }
}

fn default_app_data_dir(app_id: &str) -> std::result::Result<PathBuf, &'static str> {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .ok_or("the current platform does not expose a user data directory")?;
    Ok(base.join(app_id))
}

fn meta_value(connection: &Connection, key: &str) -> Result<Option<String>> {
    connection
        .query_row(
            &format!(
                "SELECT value FROM {} WHERE key = ?1",
                quote_identifier(META_TABLE)
            ),
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
}

fn set_meta_value(connection: &Connection, key: &str, value: String) -> Result<()> {
    connection.execute(
        &format!(
            "INSERT INTO {} (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            quote_identifier(META_TABLE)
        ),
        params![key, value],
    )?;
    Ok(())
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            params![table_name],
            |_| Ok(true),
        )
        .optional()
        .map(|value| value.unwrap_or(false))
        .map_err(Into::into)
}

fn existing_columns(connection: &Connection, table_name: &str) -> Result<BTreeSet<String>> {
    let pragma = format!("PRAGMA table_info({})", quote_identifier(table_name));
    let mut statement = connection.prepare(&pragma)?;
    let mut rows = statement.query([])?;
    let mut columns = BTreeSet::new();

    while let Some(row) = rows.next()? {
        columns.insert(row.get::<_, String>(1)?);
    }

    Ok(columns)
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return Err(RuntimeError::InvalidConfiguration(format!(
            "{label} must not be empty"
        )));
    };

    if !matches!(first, 'a'..='z' | 'A'..='Z' | '_') {
        return Err(RuntimeError::InvalidConfiguration(format!(
            "{label} '{}' must start with a letter or underscore",
            value
        )));
    }

    if !characters.all(|character| character.is_ascii_alphanumeric() || character == '_') {
        return Err(RuntimeError::InvalidConfiguration(format!(
            "{label} '{}' may only contain letters, digits, and underscores",
            value
        )));
    }

    Ok(())
}

fn validate_app_id(value: &str) -> Result<()> {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return Err(RuntimeError::InvalidConfiguration(
            "app id must not be empty".into(),
        ));
    };

    if !matches!(first, 'a'..='z' | 'A'..='Z' | '_') {
        return Err(RuntimeError::InvalidConfiguration(format!(
            "app id '{}' must start with a letter or underscore",
            value
        )));
    }

    if !characters
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(RuntimeError::InvalidConfiguration(format!(
            "app id '{}' may only contain letters, digits, underscores, and hyphens",
            value
        )));
    }

    Ok(())
}

fn validate_reserved_column_name(value: &str) -> Result<()> {
    if matches!(
        value,
        "id" | "createdAt" | "updatedAt" | "created_at" | "updated_at"
    ) {
        return Err(RuntimeError::InvalidConfiguration(format!(
            "column name '{}' is reserved by RustFrame",
            value
        )));
    }

    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn sqlite_type(kind: &DatabaseColumnType) -> &'static str {
    match kind {
        DatabaseColumnType::Text => "TEXT",
        DatabaseColumnType::Integer => "INTEGER",
        DatabaseColumnType::Real => "REAL",
        DatabaseColumnType::Boolean => "INTEGER",
        DatabaseColumnType::Json => "TEXT",
    }
}

fn sql_default_literal(value: &Value, kind: &DatabaseColumnType) -> String {
    match kind {
        DatabaseColumnType::Text => format!("'{}'", value.as_str().unwrap().replace('\'', "''")),
        DatabaseColumnType::Integer => value.to_string(),
        DatabaseColumnType::Real => value.to_string(),
        DatabaseColumnType::Boolean => {
            if value.as_bool().unwrap() {
                "1".into()
            } else {
                "0".into()
            }
        }
        DatabaseColumnType::Json => {
            format!(
                "'{}'",
                serde_json::to_string(value)
                    .expect("valid json")
                    .replace('\'', "''")
            )
        }
    }
}

fn select_columns_sql(table: &TablePlan) -> String {
    let mut columns = vec![
        quote_identifier("id"),
        quote_identifier("created_at"),
        quote_identifier("updated_at"),
    ];
    columns.extend(
        table
            .columns
            .iter()
            .map(|column| quote_identifier(&column.name)),
    );
    columns.join(", ")
}

fn sql_placeholder(index: usize) -> String {
    format!("?{}", index + 1)
}

fn now_timestamp() -> Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

fn hex_sha256(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut output = String::with_capacity(hash.len() * 2);
    for byte in hash {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        APPLIED_MIGRATIONS_TABLE, APPLIED_SEEDS_TABLE, DatabaseCapability, DatabaseColumnType,
        DatabaseFilter, DatabaseFilterOp, DatabaseListQuery, DatabaseMigrationFile,
        DatabaseOpenConfig, DatabaseOrder, DatabaseOrderDirection, DatabaseSchema,
        DatabaseSeedFile, META_TABLE,
    };
    use rusqlite::Connection;
    use serde_json::json;
    use tempfile::tempdir;

    fn sample_schema() -> DatabaseSchema {
        DatabaseSchema::from_json(
            r#"
            {
              "version": 1,
              "tables": [
                {
                  "name": "tasks",
                  "columns": [
                    { "name": "title", "type": "text", "required": true },
                    { "name": "priority", "type": "text", "default": "high" },
                    { "name": "done", "type": "boolean", "default": false },
                    { "name": "due", "type": "text" },
                    { "name": "metadata", "type": "json" }
                  ],
                  "indexes": [
                    { "columns": ["done", "priority"] }
                  ]
                },
                {
                  "name": "settings",
                  "columns": [
                    { "name": "key", "type": "text", "required": true, "unique": true },
                    { "name": "value", "type": "json", "required": true }
                  ]
                }
              ]
            }
            "#,
        )
        .unwrap()
    }

    fn open_database(schema: DatabaseSchema, seeds: Vec<DatabaseSeedFile>) -> DatabaseCapability {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        std::mem::forget(temp);
        DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(root.join("data")),
            schema,
            migration_files: Vec::new(),
            seed_files: seeds,
        })
        .unwrap()
    }

    #[test]
    fn rejects_duplicate_tables() {
        let error = DatabaseSchema::from_json(
            r#"
            {
              "version": 1,
              "tables": [
                { "name": "tasks", "columns": [{ "name": "title", "type": "text" }] },
                { "name": "tasks", "columns": [{ "name": "name", "type": "text" }] }
              ]
            }
            "#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("more than once"));
    }

    #[test]
    fn rejects_reserved_column_names() {
        let error = DatabaseSchema::from_json(
            r#"
            {
              "version": 1,
              "tables": [
                { "name": "tasks", "columns": [{ "name": "id", "type": "integer" }] }
              ]
            }
            "#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("reserved"));
    }

    #[test]
    fn stores_database_in_configured_directory() {
        let capability = open_database(sample_schema(), Vec::new());
        assert!(capability.info().database_path.ends_with("app.db"));
        assert!(capability.info().data_dir.contains("data"));
    }

    #[test]
    fn accepts_hyphenated_app_ids() {
        let temp = tempdir().unwrap();
        let capability = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "prism-gallery".into(),
            data_dir: Some(temp.path().join("data")),
            schema: sample_schema(),
            migration_files: Vec::new(),
            seed_files: Vec::new(),
        })
        .unwrap();

        assert_eq!(capability.info().app_id, "prism-gallery");
    }

    #[test]
    fn inserts_and_reads_records_with_defaults() {
        let capability = open_database(sample_schema(), Vec::new());

        let inserted = capability
            .insert(
                "tasks",
                json!({
                    "title": "Ship launch",
                    "metadata": { "lane": "release" }
                }),
            )
            .unwrap();

        assert_eq!(inserted["priority"], "high");
        assert_eq!(inserted["done"], false);
        assert_eq!(inserted["metadata"]["lane"], "release");

        let fetched = capability
            .get("tasks", inserted["id"].as_i64().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(fetched["title"], "Ship launch");
    }

    #[test]
    fn updates_records_and_refreshes_updated_at() {
        let capability = open_database(sample_schema(), Vec::new());
        let inserted = capability
            .insert("tasks", json!({ "title": "Sync docs" }))
            .unwrap();

        let updated = capability
            .update(
                "tasks",
                inserted["id"].as_i64().unwrap(),
                json!({ "done": true, "priority": "critical" }),
            )
            .unwrap();

        assert_eq!(updated["done"], true);
        assert_eq!(updated["priority"], "critical");
    }

    #[test]
    fn deletes_records() {
        let capability = open_database(sample_schema(), Vec::new());
        let inserted = capability
            .insert("tasks", json!({ "title": "Drop stale branch" }))
            .unwrap();

        assert!(
            capability
                .delete("tasks", inserted["id"].as_i64().unwrap())
                .unwrap()
        );
        assert!(
            capability
                .get("tasks", inserted["id"].as_i64().unwrap())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn lists_with_filters_sort_and_limit() {
        let capability = open_database(sample_schema(), Vec::new());
        capability
            .insert("tasks", json!({ "title": "B", "priority": "medium" }))
            .unwrap();
        capability
            .insert("tasks", json!({ "title": "A", "priority": "critical" }))
            .unwrap();
        capability
            .insert(
                "tasks",
                json!({ "title": "C", "priority": "low", "done": true }),
            )
            .unwrap();

        let results = capability
            .list(&DatabaseListQuery {
                table: "tasks".into(),
                filters: vec![DatabaseFilter {
                    field: "done".into(),
                    op: DatabaseFilterOp::Eq,
                    value: json!(false),
                }],
                order_by: vec![DatabaseOrder {
                    field: "title".into(),
                    direction: DatabaseOrderDirection::Asc,
                }],
                limit: Some(2),
                offset: None,
            })
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["title"], "A");
        assert_eq!(results[1]["title"], "B");
    }

    #[test]
    fn counts_rows() {
        let capability = open_database(sample_schema(), Vec::new());
        capability
            .insert("tasks", json!({ "title": "One", "done": true }))
            .unwrap();
        capability
            .insert("tasks", json!({ "title": "Two", "done": false }))
            .unwrap();

        let count = capability
            .count(&DatabaseListQuery {
                table: "tasks".into(),
                filters: vec![DatabaseFilter {
                    field: "done".into(),
                    op: DatabaseFilterOp::Eq,
                    value: json!(true),
                }],
                order_by: Vec::new(),
                limit: None,
                offset: None,
            })
            .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn supports_like_and_in_filters() {
        let capability = open_database(sample_schema(), Vec::new());
        capability
            .insert("tasks", json!({ "title": "Alpha", "priority": "high" }))
            .unwrap();
        capability
            .insert("tasks", json!({ "title": "Beta", "priority": "medium" }))
            .unwrap();
        capability
            .insert("tasks", json!({ "title": "Halo", "priority": "critical" }))
            .unwrap();

        let rows = capability
            .list(&DatabaseListQuery {
                table: "tasks".into(),
                filters: vec![
                    DatabaseFilter {
                        field: "title".into(),
                        op: DatabaseFilterOp::Like,
                        value: json!("%a%"),
                    },
                    DatabaseFilter {
                        field: "priority".into(),
                        op: DatabaseFilterOp::In,
                        value: json!(["high", "critical"]),
                    },
                ],
                order_by: vec![DatabaseOrder {
                    field: "title".into(),
                    direction: DatabaseOrderDirection::Asc,
                }],
                limit: None,
                offset: None,
            })
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["title"], "Alpha");
        assert_eq!(rows[1]["title"], "Halo");
    }

    #[test]
    fn applies_seed_files_once() {
        let seed = DatabaseSeedFile::from_json(
            "data/seeds/001-defaults.json",
            r#"
            {
              "entries": [
                {
                  "table": "settings",
                  "rows": [
                    { "key": "theme", "value": { "mode": "night" } }
                  ]
                }
              ]
            }
            "#,
        )
        .unwrap();

        let temp = tempdir().unwrap();
        let data_dir = temp.path().join("data");

        let first = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir.clone()),
            schema: sample_schema(),
            migration_files: Vec::new(),
            seed_files: vec![seed.clone()],
        })
        .unwrap();
        assert_eq!(
            first
                .list(&DatabaseListQuery {
                    table: "settings".into(),
                    filters: Vec::new(),
                    order_by: Vec::new(),
                    limit: None,
                    offset: None,
                })
                .unwrap()
                .len(),
            1
        );
        drop(first);

        let second = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir),
            schema: sample_schema(),
            migration_files: Vec::new(),
            seed_files: vec![seed],
        })
        .unwrap();
        assert_eq!(
            second
                .list(&DatabaseListQuery {
                    table: "settings".into(),
                    filters: Vec::new(),
                    order_by: Vec::new(),
                    limit: None,
                    offset: None,
                })
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn rejects_changed_seed_files_after_apply() {
        let temp = tempdir().unwrap();
        let data_dir = temp.path().join("data");

        let first_seed = DatabaseSeedFile::from_json(
            "data/seeds/001-defaults.json",
            r#"{ "entries": [{ "table": "settings", "rows": [{ "key": "theme", "value": "night" }] }] }"#,
        )
        .unwrap();
        DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir.clone()),
            schema: sample_schema(),
            migration_files: Vec::new(),
            seed_files: vec![first_seed],
        })
        .unwrap();

        let changed_seed = DatabaseSeedFile::from_json(
            "data/seeds/001-defaults.json",
            r#"{ "entries": [{ "table": "settings", "rows": [{ "key": "theme", "value": "day" }] }] }"#,
        )
        .unwrap();
        let error = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir),
            schema: sample_schema(),
            migration_files: Vec::new(),
            seed_files: vec![changed_seed],
        })
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("changed after it had already been applied")
        );
    }

    #[test]
    fn rejects_seed_file_without_entries() {
        let error =
            DatabaseSeedFile::from_json("data/seeds/001-empty.json", r#"{ "entries": [] }"#)
                .unwrap_err();

        assert!(error.to_string().contains("must define at least one entry"));
    }

    #[test]
    fn supports_additive_schema_upgrade() {
        let temp = tempdir().unwrap();
        let data_dir = temp.path().join("data");

        let v1 = DatabaseSchema::from_json(
            r#"
            {
              "version": 1,
              "tables": [
                { "name": "tasks", "columns": [{ "name": "title", "type": "text", "required": true }] }
              ]
            }
            "#,
        )
        .unwrap();

        let first = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir.clone()),
            schema: v1,
            migration_files: Vec::new(),
            seed_files: Vec::new(),
        })
        .unwrap();
        first
            .insert("tasks", json!({ "title": "Keep history" }))
            .unwrap();
        drop(first);

        let v2 = DatabaseSchema::from_json(
            r#"
            {
              "version": 2,
              "tables": [
                {
                  "name": "tasks",
                  "columns": [
                    { "name": "title", "type": "text", "required": true },
                    { "name": "priority", "type": "text", "default": "high" }
                  ]
                }
              ]
            }
            "#,
        )
        .unwrap();

        let upgraded = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir),
            schema: v2,
            migration_files: Vec::new(),
            seed_files: Vec::new(),
        })
        .unwrap();

        let rows = upgraded
            .list(&DatabaseListQuery {
                table: "tasks".into(),
                filters: Vec::new(),
                order_by: Vec::new(),
                limit: None,
                offset: None,
            })
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["priority"], "high");
    }

    #[test]
    fn applies_explicit_sql_migrations_for_non_additive_changes() {
        let temp = tempdir().unwrap();
        let data_dir = temp.path().join("data");

        let v1 = DatabaseSchema::from_json(
            r#"
            {
              "version": 1,
              "tables": [
                { "name": "tasks", "columns": [{ "name": "title", "type": "text", "required": true }] }
              ]
            }
            "#,
        )
        .unwrap();

        let first = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir.clone()),
            schema: v1,
            migration_files: Vec::new(),
            seed_files: Vec::new(),
        })
        .unwrap();
        first
            .insert("tasks", json!({ "title": "Rename this field" }))
            .unwrap();
        drop(first);

        let v2 = DatabaseSchema::from_json(
            r#"
            {
              "version": 2,
              "tables": [
                { "name": "tasks", "columns": [{ "name": "name", "type": "text", "required": true }] }
              ]
            }
            "#,
        )
        .unwrap();
        let migration = DatabaseMigrationFile::from_sql(
            "data/migrations/002-rename-title.sql",
            r#"
            ALTER TABLE tasks RENAME COLUMN title TO name;
            "#,
        )
        .unwrap();

        let upgraded = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir),
            schema: v2,
            migration_files: vec![migration],
            seed_files: Vec::new(),
        })
        .unwrap();

        let rows = upgraded
            .list(&DatabaseListQuery {
                table: "tasks".into(),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "Rename this field");
    }

    #[test]
    fn rejects_changed_migration_files_after_apply() {
        let temp = tempdir().unwrap();
        let data_dir = temp.path().join("data");

        let v1 = DatabaseSchema::from_json(
            r#"
            {
              "version": 1,
              "tables": [
                { "name": "tasks", "columns": [{ "name": "title", "type": "text", "required": true }] }
              ]
            }
            "#,
        )
        .unwrap();
        DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir.clone()),
            schema: v1,
            migration_files: Vec::new(),
            seed_files: Vec::new(),
        })
        .unwrap();

        let v2 = DatabaseSchema::from_json(
            r#"
            {
              "version": 2,
              "tables": [
                { "name": "tasks", "columns": [{ "name": "name", "type": "text", "required": true }] }
              ]
            }
            "#,
        )
        .unwrap();
        let first_migration = DatabaseMigrationFile::from_sql(
            "data/migrations/002-rename-title.sql",
            "ALTER TABLE tasks RENAME COLUMN title TO name;",
        )
        .unwrap();
        DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir.clone()),
            schema: v2.clone(),
            migration_files: vec![first_migration],
            seed_files: Vec::new(),
        })
        .unwrap();

        let changed_migration = DatabaseMigrationFile::from_sql(
            "data/migrations/002-rename-title.sql",
            "ALTER TABLE tasks RENAME COLUMN title TO display_name;",
        )
        .unwrap();
        let error = DatabaseCapability::open(DatabaseOpenConfig {
            app_id: "orbit_desk".into(),
            data_dir: Some(data_dir),
            schema: v2,
            migration_files: vec![changed_migration],
            seed_files: Vec::new(),
        })
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("changed after version 2 had already been applied")
        );
    }

    #[test]
    fn rejects_offset_without_limit() {
        let capability = open_database(sample_schema(), Vec::new());
        let error = capability
            .list(&DatabaseListQuery {
                table: "tasks".into(),
                filters: Vec::new(),
                order_by: Vec::new(),
                limit: None,
                offset: Some(10),
            })
            .unwrap_err();

        assert!(error.to_string().contains("offset requires limit"));
    }

    #[test]
    fn rejects_missing_required_fields_on_insert() {
        let capability = open_database(sample_schema(), Vec::new());
        let error = capability
            .insert("settings", json!({ "value": "night" }))
            .unwrap_err();

        assert!(error.to_string().contains("missing required field 'key'"));
    }

    #[test]
    fn enforces_unique_constraints() {
        let capability = open_database(sample_schema(), Vec::new());
        capability
            .insert("settings", json!({ "key": "theme", "value": "night" }))
            .unwrap();

        let error = capability
            .insert("settings", json!({ "key": "theme", "value": "day" }))
            .unwrap_err();

        assert!(error.to_string().contains("UNIQUE constraint failed"));
    }

    #[test]
    fn creates_meta_tables() {
        let capability = open_database(sample_schema(), Vec::new());
        let connection = Connection::open(capability.info().database_path.as_str()).unwrap();
        let tables: Vec<String> = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();

        assert!(tables.iter().any(|name| name == META_TABLE));
        assert!(tables.iter().any(|name| name == APPLIED_SEEDS_TABLE));
        assert!(tables.iter().any(|name| name == APPLIED_MIGRATIONS_TABLE));
    }

    #[test]
    fn boolean_columns_round_trip_as_bool() {
        let capability = open_database(sample_schema(), Vec::new());
        let inserted = capability
            .insert("tasks", json!({ "title": "Bool", "done": true }))
            .unwrap();

        assert_eq!(inserted["done"], true);
    }

    #[test]
    fn json_columns_round_trip_as_json() {
        let capability = open_database(sample_schema(), Vec::new());
        let inserted = capability
            .insert(
                "tasks",
                json!({
                    "title": "Structured",
                    "metadata": {
                        "labels": ["ship", "urgent"],
                        "estimate": 3
                    }
                }),
            )
            .unwrap();

        assert_eq!(inserted["metadata"]["estimate"], 3);
        assert_eq!(inserted["metadata"]["labels"][1], "urgent");
    }

    #[test]
    fn exposes_column_types() {
        let schema = sample_schema();
        assert!(matches!(
            schema.tables[0].columns[0].kind,
            DatabaseColumnType::Text
        ));
    }

    #[test]
    fn rejects_zero_schema_version() {
        let error = DatabaseSchema::from_json(
            r#"
            {
              "version": 0,
              "tables": [
                { "name": "tasks", "columns": [{ "name": "title", "type": "text" }] }
              ]
            }
            "#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("greater than zero"));
    }
}
