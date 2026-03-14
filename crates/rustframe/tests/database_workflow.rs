use serde_json::json;
use tempfile::tempdir;

use rustframe::{
    DatabaseCapability, DatabaseFilter, DatabaseFilterOp, DatabaseListQuery, DatabaseOpenConfig,
    DatabaseOrder, DatabaseOrderDirection, DatabaseSchema, DatabaseSeedFile,
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
