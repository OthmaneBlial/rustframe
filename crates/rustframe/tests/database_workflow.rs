use std::fs;

use serde_json::json;
use tempfile::tempdir;

use rustframe::{
    DatabaseBatchOperation, DatabaseCapability, DatabaseFilter, DatabaseFilterOp,
    DatabaseListQuery, DatabaseMigrationFile, DatabaseOpenConfig, DatabaseOrder,
    DatabaseOrderDirection, DatabaseSchema, DatabaseSeedFile,
};

fn schema() -> DatabaseSchema {
    DatabaseSchema::from_json(
        r#"
        {
          "version": 1,
          "tables": [
            {
              "name": "tasks",
              "columns": [
                { "name": "title", "type": "text", "required": true },
                { "name": "done", "type": "boolean", "default": false },
                { "name": "priority", "type": "text", "default": "high" }
              ]
            }
          ]
        }
        "#,
    )
    .unwrap()
}

#[test]
fn persists_rows_across_database_reopen() {
    let temp = tempdir().unwrap();
    let data_dir = temp.path().join("data");

    let first = DatabaseCapability::open(DatabaseOpenConfig {
        app_id: "orbit_desk".into(),
        data_dir: Some(data_dir.clone()),
        schema: schema(),
        migration_files: Vec::new(),
        seed_files: Vec::new(),
    })
    .unwrap();
    let inserted = first
        .insert(
            "tasks",
            json!({ "title": "Persist me", "priority": "critical" }),
        )
        .unwrap();
    let inserted_id = inserted["id"].as_i64().unwrap();
    drop(first);

    let second = DatabaseCapability::open(DatabaseOpenConfig {
        app_id: "orbit_desk".into(),
        data_dir: Some(data_dir),
        schema: schema(),
        migration_files: Vec::new(),
        seed_files: Vec::new(),
    })
    .unwrap();

    let fetched = second.get("tasks", inserted_id).unwrap().unwrap();
    assert_eq!(fetched["title"], "Persist me");
    assert_eq!(fetched["priority"], "critical");
}

#[test]
fn seeds_and_query_api_work_through_public_types() {
    let temp = tempdir().unwrap();
    let seed = DatabaseSeedFile::from_json(
        "data/seeds/001-defaults.json",
        r#"
        {
          "entries": [
            {
              "table": "tasks",
              "rows": [
                { "title": "A", "priority": "high" },
                { "title": "B", "priority": "low", "done": true }
              ]
            }
          ]
        }
        "#,
    )
    .unwrap();

    let database = DatabaseCapability::open(DatabaseOpenConfig {
        app_id: "orbit_desk".into(),
        data_dir: Some(temp.path().join("data")),
        schema: schema(),
        migration_files: Vec::new(),
        seed_files: vec![seed],
    })
    .unwrap();

    let rows = database
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
            limit: Some(5),
            offset: None,
        })
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["title"], "A");
}

#[test]
fn sql_migrations_work_through_public_types() {
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
        .insert("tasks", json!({ "title": "Ship migration" }))
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
        "ALTER TABLE tasks RENAME COLUMN title TO name;",
    )
    .unwrap();

    let database = DatabaseCapability::open(DatabaseOpenConfig {
        app_id: "orbit_desk".into(),
        data_dir: Some(data_dir),
        schema: v2,
        migration_files: vec![migration],
        seed_files: Vec::new(),
    })
    .unwrap();

    let row = database
        .list(&DatabaseListQuery {
            table: "tasks".into(),
            ..Default::default()
        })
        .unwrap()
        .remove(0);

    assert_eq!(row["name"], "Ship migration");
}

#[test]
fn batches_commit_in_order_and_roll_back_every_operation_on_failure() {
    let temp = tempdir().unwrap();
    let database = DatabaseCapability::open(DatabaseOpenConfig {
        app_id: "batch_desk".into(),
        data_dir: Some(temp.path().join("data")),
        schema: schema(),
        migration_files: Vec::new(),
        seed_files: Vec::new(),
    })
    .unwrap();

    let results = database
        .batch(&[
            DatabaseBatchOperation::Insert {
                table: "tasks".into(),
                record: json!({ "title": "First" }),
            },
            DatabaseBatchOperation::Insert {
                table: "tasks".into(),
                record: json!({ "title": "Second", "done": true }),
            },
        ])
        .unwrap();
    assert_eq!(results[0]["title"], "First");
    assert_eq!(results[1]["title"], "Second");

    let before = database
        .count(&DatabaseListQuery {
            table: "tasks".into(),
            ..Default::default()
        })
        .unwrap();
    let error = database
        .batch(&[
            DatabaseBatchOperation::Insert {
                table: "tasks".into(),
                record: json!({ "title": "Must roll back" }),
            },
            DatabaseBatchOperation::Insert {
                table: "tasks".into(),
                record: json!({ "done": false }),
            },
        ])
        .unwrap_err();
    assert!(error.to_string().contains("missing required field 'title'"));
    assert_eq!(
        database
            .count(&DatabaseListQuery {
                table: "tasks".into(),
                ..Default::default()
            })
            .unwrap(),
        before
    );
}

#[test]
fn concurrent_connections_preserve_all_committed_mutations() {
    let temp = tempdir().unwrap();
    let data_dir = temp.path().join("data");
    DatabaseCapability::open(DatabaseOpenConfig {
        app_id: "concurrent_desk".into(),
        data_dir: Some(data_dir.clone()),
        schema: schema(),
        migration_files: Vec::new(),
        seed_files: Vec::new(),
    })
    .unwrap();

    let workers = (0..4)
        .map(|worker| {
            let data_dir = data_dir.clone();
            std::thread::spawn(move || {
                let database = DatabaseCapability::open(DatabaseOpenConfig {
                    app_id: "concurrent_desk".into(),
                    data_dir: Some(data_dir),
                    schema: schema(),
                    migration_files: Vec::new(),
                    seed_files: Vec::new(),
                })
                .unwrap();
                for index in 0..25 {
                    database
                        .insert(
                            "tasks",
                            json!({ "title": format!("worker-{worker}-{index}") }),
                        )
                        .unwrap();
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }

    let database = DatabaseCapability::open(DatabaseOpenConfig {
        app_id: "concurrent_desk".into(),
        data_dir: Some(data_dir),
        schema: schema(),
        migration_files: Vec::new(),
        seed_files: Vec::new(),
    })
    .unwrap();
    assert_eq!(
        database
            .count(&DatabaseListQuery {
                table: "tasks".into(),
                ..Default::default()
            })
            .unwrap(),
        100
    );
}

#[test]
fn backup_and_restore_validate_identity_and_preserve_a_safety_snapshot() {
    let temp = tempdir().unwrap();
    let database = DatabaseCapability::open(DatabaseOpenConfig {
        app_id: "backup_desk".into(),
        data_dir: Some(temp.path().join("data")),
        schema: schema(),
        migration_files: Vec::new(),
        seed_files: Vec::new(),
    })
    .unwrap();
    database
        .insert("tasks", json!({ "title": "In backup" }))
        .unwrap();
    let backup = temp.path().join("snapshots/backup.db");
    database.backup_to(&backup).unwrap();
    database
        .insert("tasks", json!({ "title": "After backup" }))
        .unwrap();

    let safety = temp.path().join("snapshots/safety.db");
    database.restore_from(&backup, &safety).unwrap();
    assert!(safety.is_file());
    let rows = database
        .list(&DatabaseListQuery {
            table: "tasks".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["title"], "In backup");

    let foreign = DatabaseCapability::open(DatabaseOpenConfig {
        app_id: "foreign_desk".into(),
        data_dir: Some(temp.path().join("foreign")),
        schema: schema(),
        migration_files: Vec::new(),
        seed_files: Vec::new(),
    })
    .unwrap();
    let foreign_backup = temp.path().join("foreign.db");
    foreign.backup_to(&foreign_backup).unwrap();
    assert!(
        database
            .restore_from(&foreign_backup, &safety)
            .unwrap_err()
            .to_string()
            .contains("foreign_desk")
    );

    let corrupt = temp.path().join("corrupt.db");
    fs::write(&corrupt, b"not a sqlite database").unwrap();
    assert!(database.restore_from(&corrupt, &safety).is_err());
    let rows = database
        .list(&DatabaseListQuery {
            table: "tasks".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["title"], "In backup");
}
