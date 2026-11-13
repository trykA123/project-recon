//! Checks directory allowlists, routing lifetime, graph visibility, and serialized byte caps.

mod support;
use proptest::prelude::*;
use rusqlite::Connection;
use serde_json::{Value, json};
use support::Harness;

#[test]
fn map_byte_truncation_never_leaves_edges_to_omitted_entities() {
    let h = Harness::new();
    let mut receipts = Vec::new();
    for index in 0..45 {
        let mut input = Harness::finding(
            &format!("entity-{index}"),
            &format!("module-{index}"),
            "A short fact",
            None,
        );
        input["entity"]["name"] = json!("n".repeat(256));
        input["entity"]["aliases"] = json!(
            (0..12)
                .map(|i| format!("{i}{}", "a".repeat(126)))
                .collect::<Vec<_>>()
        );
        receipts.push(h.add(&input));
    }
    receipts.sort_by_key(|receipt| receipt["entityId"].as_str().unwrap().to_owned());
    let first = &receipts[0];
    let last = receipts.last().unwrap();
    h.ok(&[
        "entity",
        "relate",
        last["entityId"].as_str().unwrap(),
        first["entityId"].as_str().unwrap(),
        "--relation",
        "depends_on",
        "--claim",
        last["claimId"].as_str().unwrap(),
    ]);
    let prepared = h.ok(&["librarian", "prepare", "dependency map"]);
    assert_eq!(prepared["directory"]["truncated"], true);
    let ids: Vec<_> = prepared["directory"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entity| entity["entityId"].clone())
        .collect();
    for edge in prepared["directory"]["relations"].as_array().unwrap() {
        assert!(ids.contains(&edge["from"]));
        assert!(ids.contains(&edge["to"]));
    }
}

#[test]
fn candidate_limit_is_reported_as_truncation() {
    let h = Harness::new();
    for index in 0..51 {
        h.add(&Harness::finding(
            &format!("claim-{index}"),
            "runner",
            &format!("runner fact {index}"),
            None,
        ));
    }
    let data = h.ok(&["context", "search", "runner", "--budget", "16384"]);
    assert_eq!(data["truncated"], true);
    assert!(data["omittedCount"].as_u64().unwrap() >= 1);
}

#[test]
fn routes_cannot_select_other_visible_entities_or_expired_requests() {
    let h = Harness::new();
    let a = h.add(&Harness::finding("a", "runner", "runner fact", None));
    let b = h.add(&Harness::finding("b", "worker", "worker fact", None));
    let prepared = h.ok(&["librarian", "prepare", "which module", "--limit", "1"]);
    let allowed = &prepared["directory"]["entities"][0]["entityId"];
    let forbidden = if *allowed == a["entityId"] {
        &b["entityId"]
    } else {
        &a["entityId"]
    };
    let mut route = json!({"requestId":prepared["requestId"],"mapVersion":prepared["mapVersion"],"entityIds":[forbidden],"needsScout":false,"uncertainty":[]});
    let path = h.input(&route);
    h.error(
        &["librarian", "resolve", "--input", path.to_str().unwrap()],
        "invalid_route",
    );
    route["entityIds"] = json!([]);
    let path = h.input(&route);
    assert_eq!(
        h.ok(&["librarian", "resolve", "--input", path.to_str().unwrap()])["needsScout"],
        true
    );
    let connection = Connection::open(&h.database).unwrap();
    connection
        .execute("UPDATE prepared_requests SET expires_at=0", [])
        .unwrap();
    h.error(
        &["librarian", "resolve", "--input", path.to_str().unwrap()],
        "invalid_route",
    );
    assert_eq!(h.ok(&["librarian", "prune"])["expiredRequestsRemoved"], 1);
}

#[test]
fn graph_retrieval_expands_one_hop_and_rejects_private_targets_for_general_edges() {
    let h = Harness::new();
    h.task("private");
    let a = h.add(&Harness::finding(
        "a",
        "runner",
        "runner starts worker",
        None,
    ));
    let b = h.add(&Harness::finding(
        "b",
        "worker",
        "worker executes jobs",
        None,
    ));
    let private = h.add(&Harness::finding(
        "private",
        "secret",
        "secret fact",
        Some("private"),
    ));
    h.error(
        &[
            "entity",
            "relate",
            a["entityId"].as_str().unwrap(),
            private["entityId"].as_str().unwrap(),
            "--relation",
            "depends_on",
            "--claim",
            a["claimId"].as_str().unwrap(),
        ],
        "invalid_input",
    );
    h.ok(&[
        "entity",
        "relate",
        a["entityId"].as_str().unwrap(),
        b["entityId"].as_str().unwrap(),
        "--relation",
        "depends_on",
        "--claim",
        a["claimId"].as_str().unwrap(),
    ]);
    let data = h.ok(&["context", "search", "runner"]);
    assert_eq!(data["claims"].as_array().unwrap().len(), 2);
    assert!(
        !h.ok(&["librarian", "prepare", "architecture"])
            .to_string()
            .contains(private["entityId"].as_str().unwrap())
    );
}

#[test]
fn budget_and_query_limits_cover_below_equal_and_above_boundaries() {
    let h = Harness::new();
    for budget in ["128", "129", "16383", "16384"] {
        h.ok(&["context", "search", "runner", "--budget", budget]);
    }
    for budget in ["127", "16385"] {
        h.error(
            &["context", "search", "runner", "--budget", budget],
            "invalid_input",
        );
    }
    for bytes in [1023, 1024] {
        h.ok(&["context", "search", &"a".repeat(bytes)]);
    }
    h.error(&["context", "search", &"a".repeat(1025)], "invalid_input");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    #[test]
    fn lexical_input_is_data_and_cannot_escape_the_output_cap(query in "[\\p{L}\\p{N}\\p{P}]{1,60}") {
        let h=Harness::new();
        let output=h.run(&["context","search",&query,"--budget","128"]);
        let value:Value=serde_json::from_slice(&output.stdout).unwrap();
        prop_assert!(output.status.success(),"{value}");
        prop_assert!(output.stdout.len()<=513);
        prop_assert_eq!(&value["data"]["claims"],&json!([]));
    }
}
