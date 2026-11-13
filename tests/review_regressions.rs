//! Reproduces review findings around contribution approval, scoped disputes, and file opening.

mod support;
use serde_json::json;
use sha2::Digest;
use support::Harness;

#[test]
fn contributions_after_promotion_require_their_own_approval() {
    let h = Harness::new();
    h.task("private");
    let first = h.add(&Harness::finding(
        "first",
        "runner",
        "Reusable assertion",
        Some("private"),
    ));
    let id = first["claimId"].as_str().unwrap();
    h.ok(&[
        "observation",
        "promote",
        id,
        "--expected-version",
        "0",
        "--actor",
        "admin",
        "--reason",
        "approve captured evidence",
    ]);
    std::fs::write(h.root.join("private.txt"), "private source").unwrap();
    let mut later = Harness::finding("later", "runner", "Reusable assertion", Some("private"));
    later["entity"]["name"] = json!("Private label");
    later["entity"]["aliases"] = json!(["private-alias"]);
    later["evidence"][0] = json!({"path":"private.txt"});
    h.add(&later);
    let map = h.ok(&["librarian", "prepare", "runner"]);
    assert!(!map.to_string().contains("private-alias"));
    assert!(!map.to_string().contains("Private label"));
    let public = h.ok(&["context", "search", "runner"]);
    assert!(!public.to_string().contains("private.txt"));
    let verified = h.ok(&["evidence", "verify", id]);
    assert!(!verified.to_string().contains("private.txt"));
    let private = h.ok(&["context", "search", "runner", "--task", "private"]);
    assert!(private.to_string().contains("private.txt"));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Integration tests are exempt from production function-size limits."
)]
fn private_contradictions_do_not_change_general_claim_status() {
    let h = Harness::new();
    h.task("private");
    let general = h.add(&Harness::finding(
        "general",
        "runner",
        "Reusable assertion",
        None,
    ));
    let id = general["claimId"].as_str().unwrap();
    h.ok(&[
        "observation",
        "promote",
        id,
        "--expected-version",
        "0",
        "--actor",
        "admin",
        "--reason",
        "approve general evidence",
    ]);
    let mut private = Harness::finding("private", "runner", "Competing assertion", Some("private"));
    private["contradicts"] = general["claimId"].clone();
    let private = h.add(&private);
    let public = h.ok(&["context", "search", "runner"]);
    assert_eq!(public["claims"][0]["status"], "approved");
    assert_eq!(public["needsScout"], false);
    assert!(
        !public
            .to_string()
            .contains(private["claimId"].as_str().unwrap())
    );
    let task = h.ok(&["context", "search", "runner", "--task", "private"]);
    assert!(
        task["claims"]
            .as_array()
            .unwrap()
            .iter()
            .all(|claim| claim["status"] == "disputed")
    );
    h.error(
        &[
            "observation",
            "promote",
            private["claimId"].as_str().unwrap(),
            "--expected-version",
            "0",
            "--actor",
            "admin",
            "--reason",
            "unsafe promotion",
        ],
        "conflict",
    );
}

#[test]
fn lexical_entity_limit_reports_that_more_matches_exist() {
    let h = Harness::new();
    for index in 0..25 {
        h.add(&Harness::finding(
            &format!("finding-{index}"),
            &format!("module-{index}"),
            "shared search needle",
            None,
        ));
    }
    let result = h.ok(&["context", "search", "needle", "--budget", "16384"]);
    assert_eq!(result["truncated"], true);
    assert_eq!(result["lookupTruncated"], true);
}

#[test]
fn repeated_fts_hits_do_not_crowd_out_another_matching_entity() {
    let h = Harness::new();
    for index in 0..24 {
        h.add(&Harness::finding(
            &format!("finding-{index}"),
            "crowded",
            &format!("needle item {index}"),
            None,
        ));
    }
    let other = h.add(&Harness::finding(
        "other",
        "other",
        "needle item other",
        None,
    ));
    let result = h.ok(&["context", "search", "needle", "--budget", "16384"]);
    assert!(
        result
            .to_string()
            .contains(other["entityId"].as_str().unwrap())
    );
}

#[cfg(target_os = "linux")]
#[test]
fn evidence_fifo_is_rejected_without_waiting_for_a_writer() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let h = Harness::new();
    let fifo = h.root.join("pipe");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let mut input = Harness::finding("pipe", "runner", "Evidence must be regular", None);
    input["evidence"][0] = json!({"path":"pipe"});
    let path = h.input(&input);
    let mut child = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args([
            "--db",
            h.database.to_str().unwrap(),
            "--project",
            "demo",
            "observation",
            "add",
            "--input",
            path.to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("FIFO open blocked instead of rejecting a non-regular source");
        }
        std::thread::yield_now();
    }
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["error"]["code"], "invalid_input");
}

#[test]
fn json_input_requires_a_regular_file() {
    let h = Harness::new();
    h.error(
        &["observation", "add", "--input", h.root.to_str().unwrap()],
        "invalid_input",
    );
}

#[test]
fn stale_promotion_cannot_approve_an_unseen_contribution() {
    let h = Harness::new();
    h.task("private");
    let original = h.add(&Harness::finding(
        "original",
        "runner",
        "Reusable assertion",
        Some("private"),
    ));
    let claim = original["claimId"].as_str().unwrap();
    let reviewed = h.ok(&["context", "search", "runner", "--task", "private"]);
    let version = reviewed["claims"][0]["version"]
        .as_u64()
        .unwrap()
        .to_string();
    let mut unseen = Harness::finding("unseen", "runner", "Reusable assertion", Some("private"));
    unseen["entity"]["aliases"] = json!(["unreviewed-private-alias"]);
    h.add(&unseen);
    h.error(
        &[
            "observation",
            "promote",
            claim,
            "--expected-version",
            &version,
            "--actor",
            "admin",
            "--reason",
            "approved old snapshot",
        ],
        "conflict",
    );
    let general = h.ok(&["context", "search", "runner"]);
    assert_eq!(general["claims"], json!([]));
}

#[cfg(unix)]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Keep the stress operation and its thread cleanup together; tests are size-exempt."
)]
fn concurrent_symlink_swaps_never_capture_bytes_outside_the_root() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let h = Harness::new();
    let outside = h.directory.path().join("outside.txt");
    std::fs::write(&outside, "outside secret bytes").unwrap();
    std::fs::write(h.root.join("source.txt"), "inside bytes").unwrap();
    let active = Arc::new(AtomicBool::new(true));
    let writer = {
        let active = Arc::clone(&active);
        let root = h.root.clone();
        std::thread::spawn(move || {
            while active.load(Ordering::Relaxed) {
                let link = root.join("replacement-link");
                std::os::unix::fs::symlink(&outside, &link).unwrap();
                std::fs::rename(link, root.join("source.txt")).unwrap();
                let normal = root.join("replacement-file");
                std::fs::write(&normal, "inside bytes").unwrap();
                std::fs::rename(normal, root.join("source.txt")).unwrap();
            }
        })
    };
    let expected = sha2::Sha256::digest(b"inside bytes");
    let expected: String = expected.iter().map(|byte| format!("{byte:02x}")).collect();
    let mut escaped = false;
    for index in 0..300 {
        let path = h.input(&Harness::finding(
            &format!("swap-{index}"),
            "runner",
            "Untrusted concurrent pointer",
            None,
        ));
        let output = h.run(&["observation", "add", "--input", path.to_str().unwrap()]);
        if output.status.success() {
            let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let connection = rusqlite::Connection::open(&h.database).unwrap();
            let hash: String = connection
                .query_row(
                    "SELECT source_hash FROM evidence WHERE observation_id=?1",
                    [value["data"]["observationId"].as_str().unwrap()],
                    |row| row.get(0),
                )
                .unwrap();
            if hash != expected {
                escaped = true;
                break;
            }
        }
    }
    active.store(false, Ordering::Relaxed);
    writer.join().unwrap();
    assert!(
        !escaped,
        "source open followed a swapped symlink outside its directory capability"
    );
}
