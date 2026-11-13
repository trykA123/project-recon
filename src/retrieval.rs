//! Retrieves bounded scoped claims using exact lookup, lexical FTS5, and one-hop relationships.

use crate::{
    catalog, conflicts,
    db::{self, Db},
    error::{Error, Result},
    lookup::{self, Selection},
    output, relations, validate,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub struct Query<'a> {
    pub project: &'a str,
    pub task: Option<&'a str>,
    pub text: &'a str,
    pub budget: usize,
    pub entities: Option<&'a [String]>,
    pub needs_scout: bool,
    pub uncertainty: &'a [String],
    pub route: &'static str,
}

/// Fetches candidate claims and their metadata under a consistent caller-owned snapshot.
fn claims(connection: &Connection, query: &Query<'_>, ids: &[String]) -> Result<Vec<Value>> {
    catalog::rows(connection,
        "SELECT json_object('claimId',c.id,'entityId',c.entity_id,'entityKey',e.canonical_key,'taskId',c.task_id,
         'kind',c.kind,'body',c.body,'status',c.status,'version',c.version)
         FROM claims c JOIN entities e ON e.project=c.project AND e.id=c.entity_id
         WHERE c.project=?1 AND (c.shared=1 OR c.task_id=?2) AND c.entity_id IN (SELECT value FROM json_each(?3))
         ORDER BY CASE WHEN c.task_id=?2 THEN 0 ELSE 1 END,c.row_id DESC LIMIT 50",
        params![query.project,query.task,serde_json::to_string(ids)?])
}

/// Reads at most three provenance pointers per selected claim using one batched query.
pub fn evidence(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    ids: &[String],
) -> Result<Vec<Value>> {
    catalog::rows(connection,
        "WITH selected AS (SELECT e.id,e.path,e.symbol,e.line_start,e.line_end,e.source_hash,e.commit_sha,
          e.observed_at,o.claim_id,o.actor,o.id AS observation_id,
          row_number() OVER(PARTITION BY o.claim_id ORDER BY e.id DESC) AS position
          FROM observations o JOIN evidence e ON e.project=o.project AND e.observation_id=o.id
          JOIN claims c ON c.project=o.project AND c.id=o.claim_id
          WHERE o.project=?1 AND o.claim_id IN (SELECT value FROM json_each(?2))
            AND (c.shared=1 OR c.task_id=?3) AND (o.shared=1 OR c.task_id=?3))
         SELECT json_object('id',id,'claimId',claim_id,'observationId',observation_id,'actor',actor,'path',path,'symbol',symbol,
          'lineStart',line_start,'lineEnd',line_end,'sourceHash',source_hash,'commit',commit_sha,'observedAt',observed_at,'freshness','unchecked')
         FROM selected WHERE position<=3 ORDER BY id DESC LIMIT 150",
        params![project,serde_json::to_string(ids)?,task])
}

/// Adds scoped provenance and contradiction IDs to a bounded candidate list.
fn enrich(connection: &Connection, query: &Query<'_>, claims: &mut [Value]) -> Result<()> {
    let ids: Vec<String> = claims
        .iter()
        .filter_map(|claim| claim["claimId"].as_str().map(Into::into))
        .collect();
    let pointers = evidence(connection, query.project, query.task, &ids)?;
    let disputes = conflicts::pairs(connection, query.project, query.task, &ids)?;
    for claim in claims {
        let id = claim["claimId"].clone();
        claim["evidence"] = json!(
            pointers
                .iter()
                .filter(|source| source["claimId"] == id)
                .collect::<Vec<_>>()
        );
        claim["contradictions"] = json!(
            disputes
                .iter()
                .filter_map(|pair| {
                    if pair[0] == id {
                        Some(pair[1].clone())
                    } else if pair[1] == id {
                        Some(pair[0].clone())
                    } else {
                        None
                    }
                })
                .take(10)
                .collect::<Vec<_>>()
        );
        if claim["contradictions"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
        {
            claim["status"] = json!("disputed");
        }
    }
    Ok(())
}

/// Builds a scoped response and drops whole claims until the complete JSON envelope fits its cap.
pub fn build(connection: &Connection, query: Query<'_>) -> Result<Value> {
    validate::text(query.text, "query", 1024)?;
    validate::budget(query.budget)?;
    db::scope(connection, query.project, query.task)?;
    let Selection {
        mut ids,
        mut truncated,
    } = match query.entities {
        Some(ids) => Selection {
            ids: ids.to_vec(),
            truncated: false,
        },
        None => lookup::find(connection, query.project, query.task, query.text)?,
    };
    if query.route == "direct" {
        truncated |= relations::expand(connection, query.project, query.task, &mut ids)?;
    }
    let mut selected = claims(connection, &query, &ids)?;
    enrich(connection, &query, &mut selected)?;
    let count: i64 = connection.query_row(
        "SELECT count(*) FROM claims c WHERE c.project=?1 AND (c.shared=1 OR c.task_id=?2)
        AND c.entity_id IN (SELECT value FROM json_each(?3))",
        params![query.project, query.task, serde_json::to_string(&ids)?],
        |row| row.get(0),
    )?;
    let count = usize::try_from(count).map_err(|_| Error::Budget)?;
    loop {
        let data = json!({"claims":selected,"omittedCount":count-selected.len(),"truncated":truncated || count>selected.len(),"lookupTruncated":truncated,
            "needsScout":query.needs_scout || selected.is_empty() || selected.iter().any(|c|c["status"]!="approved"),
            "uncertainty":query.uncertainty,"route":query.route});
        match output::bounded(data, query.budget) {
            Ok(result) => return Ok(result),
            Err(Error::Budget) if !selected.is_empty() => {
                selected.pop();
            }
            Err(error) => return Err(error),
        }
    }
}

/// Performs direct deterministic retrieval inside one read snapshot, without a model call.
pub fn search(
    db: &mut Db,
    text: &str,
    options: (Option<&str>, Option<&str>, usize),
) -> Result<Value> {
    let tx = db.connection.transaction()?;
    let entities = options.1.map(|entity| vec![entity.to_owned()]);
    if let Some(entity) = options.1 {
        validate::id(entity)?;
        if !catalog::visible_entity(&tx, &db.project, options.0, entity)? {
            return Err(Error::NotFound("entity in this scope".into()));
        }
    }
    let value = build(
        &tx,
        Query {
            project: &db.project,
            task: options.0,
            text,
            budget: options.2,
            entities: entities.as_deref(),
            needs_scout: false,
            uncertainty: &[],
            route: "direct",
        },
    )?;
    tx.commit()?;
    Ok(value)
}
