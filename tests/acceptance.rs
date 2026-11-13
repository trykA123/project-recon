//! Exercises persistent memory, scope safety, Librarian routing, and bounded protocol output.

mod support;
use serde_json::{Value, json};
use support::Harness;

#[test]
fn task_resumes_and_rejects_stale_checkpoint_without_losing_progress() {
    let h = Harness::new();
    assert_eq!(h.task("migration")["version"], 0);
    let checkpoint = h.input(&json!({"expectedVersion":0,"actor":"codex","phase":"investigation",
        "progress":["mapped runner"],"decisions":[],"constraints":[],"blockers":[],"nextActions":["check worker isolation"]}));
    h.ok(&[
        "task",
        "checkpoint",
        "migration",
        "--input",
        checkpoint.to_str().unwrap(),
    ]);
    h.error(
        &[
            "task",
            "checkpoint",
            "migration",
            "--input",
            checkpoint.to_str().unwrap(),
        ],
        "conflict",
    );
    let resumed = h.ok(&["task", "resume", "migration"]);
    assert_eq!(resumed["version"], 1);
    assert_eq!(
        resumed["checkpoint"]["nextActions"][0],
        "check worker isolation"
    );
}

#[test]
fn contributions_deduplicate_claims_but_preserve_authors_and_retry_before_reading_source() {
    let h = Harness::new();
    let input = Harness::finding("one", "runner", "Worker initializes the runner", None);
    let first = h.add(&input);
    let mut second = input.clone();
    second["idempotencyKey"] = json!("two");
    second["actor"] = json!("claude");
    let receipt = h.add(&second);
    assert_eq!(first["claimId"], receipt["claimId"]);
    assert_ne!(first["observationId"], receipt["observationId"]);
    std::fs::remove_file(h.root.join("source.txt")).unwrap();
    assert_eq!(h.add(&input), first);
    let mut conflict = input;
    conflict["text"] = json!("changed assertion");
    let path = h.input(&conflict);
    h.error(
        &["observation", "add", "--input", path.to_str().unwrap()],
        "conflict",
    );
}

#[test]
fn evidence_mismatch_rolls_back_every_record_and_outside_paths_are_rejected() {
    let h = Harness::new();
    let mut input = Harness::finding("wrong-hash", "runner", "Something true", None);
    input["evidence"][0]["sourceHash"] = json!("0".repeat(64));
    let path = h.input(&input);
    h.error(
        &["observation", "add", "--input", path.to_str().unwrap()],
        "source_changed",
    );
    assert!(
        h.ok(&["context", "search", "runner"])["claims"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    input["evidence"][0] = json!({"path":"../outside.txt"});
    let path = h.input(&input);
    h.error(
        &["observation", "add", "--input", path.to_str().unwrap()],
        "invalid_input",
    );
}

#[test]
fn task_and_project_scopes_hide_private_claims_aliases_and_contradiction_ids() {
    let h = Harness::new();
    h.task("private");
    h.task("other");
    let general = h.add(&Harness::finding(
        "general",
        "runner",
        "Runner initializes worker",
        None,
    ));
    let mut input = Harness::finding(
        "private",
        "runner",
        "Runner never initializes worker",
        Some("private"),
    );
    input["entity"]["aliases"] = json!(["secret-alias"]);
    input["contradicts"] = general["claimId"].clone();
    let private = h.add(&input);
    let public = h.ok(&["context", "search", "runner"]);
    assert_eq!(public["claims"].as_array().unwrap().len(), 1);
    assert!(
        !public
            .to_string()
            .contains(private["claimId"].as_str().unwrap())
    );
    let map = h.ok(&["librarian", "prepare", "runner"]);
    assert!(!map.to_string().contains("secret-alias"));
    let scoped = h.ok(&["context", "search", "runner", "--task", "private"]);
    assert_eq!(scoped["claims"].as_array().unwrap().len(), 2);
    assert_eq!(scoped["needsScout"], true);
    let output = h.project_run("different", &["context", "search", "runner"]);
    assert!(!output.status.success());
}

#[test]
fn librarian_routes_only_allowed_ids_and_rejects_stale_maps() {
    let h = Harness::new();
    let finding = h.add(&Harness::finding(
        "one",
        "runner",
        "Runner starts a worker process",
        None,
    ));
    let prepared = h.ok(&["librarian", "prepare", "Who launches the execution unit?"]);
    assert!(
        !prepared
            .to_string()
            .contains("Runner starts a worker process")
    );
    let route = json!({"requestId":prepared["requestId"],"mapVersion":prepared["mapVersion"],
        "entityIds":[finding["entityId"]],"needsScout":false,"uncertainty":[]});
    let path = h.input(&route);
    let result = h.ok(&["librarian", "resolve", "--input", path.to_str().unwrap()]);
    assert_eq!(result["claims"][0]["claimId"], finding["claimId"]);
    let mut invalid = route.clone();
    invalid["entityIds"] = json!(["invented"]);
    let bad = h.input(&invalid);
    h.error(
        &["librarian", "resolve", "--input", bad.to_str().unwrap()],
        "invalid_route",
    );
    h.add(&Harness::finding(
        "two",
        "worker",
        "Worker processes tasks",
        None,
    ));
    h.error(
        &["librarian", "resolve", "--input", path.to_str().unwrap()],
        "stale_map",
    );
}

#[test]
fn context_obeys_serialized_budget_and_treats_fts_operators_as_data() {
    let h = Harness::new();
    for index in 0..5 {
        h.add(&Harness::finding(
            &format!("finding-{index}"),
            "runner",
            &format!("runner {index} {}", "detail ".repeat(100)),
            None,
        ));
    }
    let output = h.run(&["context", "search", "runner", "--budget", "128"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stdout.len() <= 128 * 4 + 1);
    let data: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(data["data"]["truncated"], true);
    h.ok(&["context", "search", "\" OR * NEAR(foo) NOT bar"]);
}

#[test]
fn source_verification_detects_changes_without_promoting_claims() {
    let h = Harness::new();
    let claim = h.add(&Harness::finding(
        "one",
        "runner",
        "Runner starts worker",
        None,
    ));
    let id = claim["claimId"].as_str().unwrap();
    assert_eq!(
        h.ok(&["evidence", "verify", id])["evidence"][0]["freshness"],
        "current"
    );
    std::fs::write(h.root.join("source.txt"), "changed implementation").unwrap();
    assert_eq!(
        h.ok(&["evidence", "verify", id])["evidence"][0]["freshness"],
        "changed"
    );
    assert_eq!(
        h.ok(&["context", "search", "runner"])["claims"][0]["status"],
        "candidate"
    );
}

#[test]
fn unknown_input_fields_and_oversized_files_fail_closed() {
    let h = Harness::new();
    let mut input = Harness::finding("one", "runner", "Runner starts worker", None);
    input["executeShell"] = json!("ignored injection");
    let path = h.input(&input);
    h.error(
        &["observation", "add", "--input", path.to_str().unwrap()],
        "invalid_input",
    );
    std::fs::write(&path, " ".repeat(65537)).unwrap();
    h.error(
        &["observation", "add", "--input", path.to_str().unwrap()],
        "invalid_input",
    );
}

#[test]
fn archived_tasks_reject_writes_and_explicit_promotion_preserves_provenance() {
    let h = Harness::new();
    h.task("migration");
    let claim = h.add(&Harness::finding(
        "one",
        "runner",
        "Reusable runner fact",
        Some("migration"),
    ));
    assert!(
        h.ok(&["context", "search", "runner"])["claims"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    h.ok(&[
        "observation",
        "promote",
        claim["claimId"].as_str().unwrap(),
        "--expected-version",
        "0",
        "--actor",
        "admin",
        "--reason",
        "verified for reuse",
    ]);
    assert_eq!(
        h.ok(&["context", "search", "runner"])["claims"][0]["taskId"],
        "migration"
    );
    h.ok(&[
        "task",
        "archive",
        "migration",
        "--expected-version",
        "0",
        "--actor",
        "admin",
    ]);
    let path=h.input(&json!({"expectedVersion":1,"actor":"codex","phase":"continue","progress":[],"decisions":[],"constraints":[],"blockers":[],"nextActions":[]}));
    h.error(
        &[
            "task",
            "checkpoint",
            "migration",
            "--input",
            path.to_str().unwrap(),
        ],
        "conflict",
    );
}

#[test]
fn metrics_preserve_unknown_counts_and_journal_paginates_without_bodies() {
    let h = Harness::new();
    h.add(&Harness::finding(
        "one",
        "runner",
        "Sensitive body stays out of journal",
        None,
    ));
    let path=h.input(&json!({"runId":"run-1","role":"librarian","model":"harness","inputTokens":null,"outputTokens":null,
        "cacheReadTokens":null,"cacheWriteTokens":null,"costUsdMicros":null,"durationMs":15,"estimated":true}));
    h.ok(&["metrics", "record", "--input", path.to_str().unwrap()]);
    let metrics = h.ok(&["metrics", "export", "--run", "run-1"]);
    assert!(metrics["records"][0]["inputTokens"].is_null());
    let journal = h.ok(&["journal", "--limit", "1"]);
    assert_eq!(journal["events"].as_array().unwrap().len(), 1);
    assert!(!journal.to_string().contains("Sensitive body"));
}
