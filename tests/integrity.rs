//! Checks atomic failures, competing CLI writers, database identity, and project isolation.

mod support;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    process::Command,
    sync::{Arc, Barrier},
};
use support::Harness;

#[test]
fn unrelated_database_is_rejected_without_changing_its_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.sqlite3");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("CREATE TABLE important(value TEXT); INSERT INTO important VALUES('keep');")
        .unwrap();
    drop(connection);
    let before = std::fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args([
            "--db",
            path.to_str().unwrap(),
            "--project",
            "demo",
            "init",
            "--repo",
            directory.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["error"]["code"], "conflict");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!directory.path().join("existing.sqlite3-wal").exists());
}

#[test]
fn missing_database_read_does_not_create_a_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing.db");
    let output = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args([
            "--db",
            path.to_str().unwrap(),
            "--project",
            "demo",
            "context",
            "search",
            "runner",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!path.exists());
}

#[test]
fn source_insert_fault_rolls_back_claims_observations_entities_events_and_fts() {
    let h = Harness::new();
    let connection = Connection::open(&h.database).unwrap();
    connection.execute_batch("CREATE TRIGGER injected_fault BEFORE INSERT ON evidence BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    let path = h.input(&Harness::finding("fault", "runner", "runner fact", None));
    h.error(
        &["observation", "add", "--input", path.to_str().unwrap()],
        "storage_error",
    );
    for table in ["claims", "observations", "entities", "events", "claims_fts"] {
        let count: i64 = connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "partial transaction left {table}");
    }
    assert_eq!(
        connection
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}

#[test]
fn competing_checkpoints_keep_exactly_one_winner_under_repeated_races() {
    let h = Arc::new(Harness::new());
    for iteration in 0..12 {
        let task = format!("race-{iteration}");
        h.task(&task);
        let barrier = Arc::new(Barrier::new(2));
        let handles:Vec<_>=(0..2).map(|writer|{
            let h=Arc::clone(&h); let barrier=Arc::clone(&barrier); let task=task.clone();
            std::thread::spawn(move||{
                let path=h.input(&json!({"expectedVersion":0,"actor":"codex","phase":format!("writer-{writer}"),
                    "progress":[],"decisions":[],"constraints":[],"blockers":[],"nextActions":[]}));
                barrier.wait(); h.run(&["task","checkpoint",&task,"--input",path.to_str().unwrap()])
            })
        }).collect();
        let outputs: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert_eq!(
            outputs
                .iter()
                .filter(|output| output.status.success())
                .count(),
            1
        );
        assert_eq!(h.ok(&["task", "resume", &task])["version"], 1);
        let failure = outputs
            .iter()
            .find(|output| !output.status.success())
            .unwrap();
        let value: Value = serde_json::from_slice(&failure.stdout).unwrap();
        assert_eq!(value["error"]["code"], "conflict");
    }
}

#[test]
fn concurrent_identical_contributions_share_one_durable_receipt() {
    let h = Arc::new(Harness::new());
    let barrier = Arc::new(Barrier::new(4));
    let input = h.input(&Harness::finding("same-key", "runner", "runner fact", None));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let h = Arc::clone(&h);
            let barrier = Arc::clone(&barrier);
            let input = input.clone();
            std::thread::spawn(move || {
                barrier.wait();
                h.ok(&["observation", "add", "--input", input.to_str().unwrap()])
            })
        })
        .collect();
    let receipts: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert!(receipts.iter().all(|receipt| receipt == &receipts[0]));
    let connection = Connection::open(&h.database).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM observations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}

#[test]
fn route_resolution_and_knowledge_writes_keep_a_consistent_snapshot() {
    let h = Arc::new(Harness::new());
    for iteration in 0..12 {
        let entity = format!("module-{iteration}");
        let original = h.add(&Harness::finding(
            &format!("original-{iteration}"),
            &entity,
            "Original evidence",
            None,
        ));
        let prepared = h.ok(&["librarian", "prepare", "conceptual request"]);
        let route = h.input(
            &json!({"requestId":prepared["requestId"],"mapVersion":prepared["mapVersion"],
            "entityIds":[original["entityId"]],"needsScout":false,"uncertainty":[]}),
        );
        let barrier = Arc::new(Barrier::new(2));
        let reader = {
            let h = Arc::clone(&h);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                h.run(&["librarian", "resolve", "--input", route.to_str().unwrap()])
            })
        };
        barrier.wait();
        h.add(&Harness::finding(
            &format!("updated-{iteration}"),
            &entity,
            "New evidence after map creation",
            None,
        ));
        let output = reader.join().unwrap();
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        if output.status.success() {
            assert_eq!(value["data"]["claims"].as_array().unwrap().len(), 1);
            assert_eq!(value["data"]["claims"][0]["claimId"], original["claimId"]);
        } else {
            assert_eq!(value["error"]["code"], "stale_map");
        }
    }
}

#[test]
fn projects_in_one_database_have_separate_entities_and_knowledge() {
    let h = Harness::new();
    h.add(&Harness::finding(
        "one",
        "runner",
        "runner private knowledge",
        None,
    ));
    let output = h.project_run("second", &["init", "--repo", h.root.to_str().unwrap()]);
    assert!(output.status.success());
    let output = h.project_run("second", &["context", "search", "runner"]);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["data"]["claims"], json!([]));
    let mut private = Harness::finding("task", "runner", "task knowledge", Some("private"));
    h.task("private");
    private["entity"]["name"] = json!("Private label");
    h.add(&private);
    assert!(!h.ok(&["journal"]).to_string().contains("private"));
}

#[cfg(unix)]
#[test]
fn evidence_symlink_cannot_escape_the_registered_root() {
    let h = Harness::new();
    let outside = h.directory.path().join("outside.txt");
    std::fs::write(&outside, "outside").unwrap();
    std::os::unix::fs::symlink(&outside, h.root.join("escape.txt")).unwrap();
    let mut input = Harness::finding("escape", "runner", "runner fact", None);
    input["evidence"][0]["path"] = json!("escape.txt");
    let path = h.input(&input);
    h.error(
        &["observation", "add", "--input", path.to_str().unwrap()],
        "invalid_input",
    );
}
