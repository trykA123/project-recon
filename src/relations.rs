//! Stores evidence-supported relationships and restricts graph expansion to visible endpoints.

use crate::{
    catalog,
    db::{self, Db, Event},
    error::{Error, Result},
    validate,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};

pub struct Relation<'a> {
    pub from: &'a str,
    pub to: &'a str,
    pub kind: &'a str,
    pub claim: &'a str,
}

/// Adds one supported relationship; its claim controls visibility and both endpoints must be visible.
pub fn add(db: &mut Db, input: Relation<'_>) -> Result<Value> {
    for id in [input.from, input.to, input.claim] {
        validate::id(id)?;
    }
    if !["contains", "depends_on", "calls", "configures", "tested_by"].contains(&input.kind) {
        return Err(Error::Invalid("unsupported relationship kind".into()));
    }
    let tx = db
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let support: Option<(String, Option<String>, bool)> = tx
        .query_row(
            "SELECT entity_id,task_id,shared FROM claims WHERE project=?1 AND id=?2",
            params![db.project, input.claim],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (source, task, shared) =
        support.ok_or_else(|| Error::NotFound("supporting claim".into()))?;
    let scope = if shared { None } else { task.as_deref() };
    if source != input.from || !catalog::visible_entity(&tx, &db.project, scope, input.to)? {
        return Err(Error::Invalid(
            "relationship must connect visible entities and use a source-entity claim".into(),
        ));
    }
    let inserted = tx.execute("INSERT INTO edges(project,source_id,target_id,relation,claim_id) VALUES(?1,?2,?3,?4,?5) ON CONFLICT DO NOTHING",
        params![db.project,input.from,input.to,input.kind,input.claim])?;
    if inserted > 0 {
        db::event(
            &tx,
            &db.project,
            Event {
                task: scope,
                actor: "cli",
                kind: "entity.relate",
                subject: input.from,
                reason: None,
            },
        )?;
        db::touch(&tx, &db.project)?;
    }
    tx.commit()?;
    Ok(json!({"from":input.from,"to":input.to,"relation":input.kind,"claimId":input.claim}))
}

/// Reads bounded edges whose supporting claims and both endpoints are visible in this scope.
pub fn list(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    ids: &[String],
) -> Result<Vec<Value>> {
    catalog::rows(connection,
        "SELECT json_object('from',x.source_id,'to',x.target_id,'relation',x.relation,'claimId',x.claim_id)
         FROM edges x JOIN claims c ON c.project=x.project AND c.id=x.claim_id
         WHERE x.project=?1 AND (c.shared=1 OR c.task_id=?2)
           AND x.source_id IN (SELECT value FROM json_each(?3))
           AND EXISTS(SELECT 1 FROM claims a WHERE a.project=?1 AND a.entity_id=x.source_id AND (a.shared=1 OR a.task_id=?2))
           AND EXISTS(SELECT 1 FROM claims b WHERE b.project=?1 AND b.entity_id=x.target_id AND (b.shared=1 OR b.task_id=?2))
         ORDER BY x.id LIMIT 100", params![project,task,serde_json::to_string(ids)?])
}

/// Extends selected entities by one outgoing relationship hop, capped at 20 IDs.
pub fn expand(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    ids: &mut Vec<String>,
) -> Result<bool> {
    let edges = list(connection, project, task, ids)?;
    for edge in edges {
        if let Some(id) = edge["to"].as_str()
            && !ids.iter().any(|existing| existing == id)
        {
            if ids.len() >= 20 {
                return Ok(true);
            }
            ids.push(id.into());
        }
    }
    Ok(false)
}
