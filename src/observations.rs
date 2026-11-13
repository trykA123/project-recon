//! Appends contributions and evidence while deduplicating canonical claims without losing provenance.

use crate::{
    conflicts,
    db::{self, Db, Event},
    error::{Error, Result},
    model::{self, Observation},
    source, validate,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};

/// Looks up an idempotency receipt before reading sources; changed payloads conflict.
fn retry(
    connection: &Connection,
    project: &str,
    key: &str,
    payload: &str,
) -> Result<Option<Value>> {
    let stored: Option<(String,String)> = connection.query_row(
        "SELECT payload_hash,receipt_json FROM observations WHERE project=?1 AND idempotency_key=?2",
        params![project,key], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
    match stored {
        Some((hash, receipt)) if hash == payload => Ok(Some(serde_json::from_str(&receipt)?)),
        Some(_) => Err(Error::Conflict(
            "idempotency key already has a different contribution".into(),
        )),
        None => Ok(None),
    }
}

/// Appends a new observation transactionally; identical retries preserve their original receipt.
pub fn add(db: &mut Db, input: &Observation) -> Result<Value> {
    input.validate()?;
    let payload = validate::digest(input)?;
    if let Some(receipt) = retry(
        &db.connection,
        &db.project,
        &input.idempotency_key,
        &payload,
    )? {
        return Ok(receipt);
    }
    let hashes = source::capture(&db.root()?, &input.evidence)?;
    let tx = db
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    if let Some(receipt) = retry(&tx, &db.project, &input.idempotency_key, &payload)? {
        return Ok(receipt);
    }
    if let Some(task) = &input.task_id {
        db::task(&tx, &db.project, task, true)?;
    }
    let identity = upsert_claim(&tx, &db.project, input)?;
    let claim = &identity.claim;
    if let Some(other) = &input.contradicts {
        contradict(&tx, &db.project, input.task_id.as_deref(), (claim, other))?;
    }
    let observation: String =
        tx.query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))?;
    let version: i64 = tx.query_row(
        "SELECT version FROM claims WHERE project=?1 AND id=?2",
        params![db.project, claim],
        |row| row.get(0),
    )?;
    let receipt = json!({"claimId":claim,"entityId":identity.entity,"observationId":observation,"version":version,"deduplicated":identity.inserted==0});
    tx.execute("INSERT INTO observations(project,id,claim_id,actor,idempotency_key,payload_hash,entity_json,receipt_json,shared) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![db.project,observation,claim,input.actor,input.idempotency_key,payload,serde_json::to_string(&input.entity)?,serde_json::to_string(&receipt)?,input.task_id.is_none()])?;
    capture_evidence(&tx, &db.project, &observation, (input, &hashes))?;
    db::event(
        &tx,
        &db.project,
        Event {
            task: input.task_id.as_deref(),
            actor: &input.actor,
            kind: "observation.add",
            subject: claim,
            reason: None,
        },
    )?;
    db::touch(&tx, &db.project)?;
    tx.commit()?;
    Ok(receipt)
}

struct Identity {
    entity: String,
    claim: String,
    inserted: usize,
}

/// Resolves canonical entity/claim identity and inserts only previously unseen claim content.
fn upsert_claim(connection: &Connection, project: &str, input: &Observation) -> Result<Identity> {
    let entity = validate::digest(&(project, &input.entity.kind, &input.entity.key))?;
    let normalized = input.text.split_whitespace().collect::<Vec<_>>().join(" ");
    let claim = validate::digest(&(&entity, &input.task_id, &input.kind, &normalized))?;
    connection.execute("INSERT INTO entities(project,id,kind,canonical_key) VALUES(?1,?2,?3,?4) ON CONFLICT DO NOTHING",
        params![project,entity,enum_name(&input.entity.kind)?,input.entity.key])?;
    let inserted = connection.execute("INSERT INTO claims(project,id,entity_id,task_id,kind,body,shared) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(project,id) DO NOTHING",
        params![project,claim,entity,input.task_id,enum_name(&input.kind)?,input.text,input.task_id.is_none()])?;
    if inserted == 0 {
        connection.execute(
            "UPDATE claims SET version=version+1 WHERE project=?1 AND id=?2",
            params![project, claim],
        )?;
    }
    Ok(Identity {
        entity,
        claim,
        inserted,
    })
}

/// Serializes a closed enum to its schema label; non-string variants indicate a programming error.
fn enum_name(value: &impl serde::Serialize) -> Result<String> {
    match serde_json::to_value(value)? {
        Value::String(name) => Ok(name),
        _ => Err(Error::Invalid("expected a closed enum label".into())),
    }
}

/// Inserts captured evidence using one prepared statement inside the owning transaction.
fn capture_evidence(
    connection: &Connection,
    project: &str,
    observation: &str,
    data: (&Observation, &[String]),
) -> Result<()> {
    let mut statement = connection.prepare("INSERT INTO evidence(project,observation_id,path,symbol,line_start,line_end,source_hash,commit_sha) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)")?;
    for (source, hash) in data.0.evidence.iter().zip(data.1) {
        statement.execute(params![
            project,
            observation,
            source.path,
            source.symbol,
            source.line_start,
            source.line_end,
            hash,
            source.commit
        ])?;
    }
    Ok(())
}

/// Appends a scoped disagreement; retrieval derives disputed status from visible evidence only.
fn contradict(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    pair: (&str, &str),
) -> Result<()> {
    let (claim, other) = pair;
    if claim == other {
        return Err(Error::Invalid("a claim cannot contradict itself".into()));
    }
    let exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM claims WHERE project=?1 AND id=?2 AND (shared=1 OR task_id=?3))",
        params![project,other,task], |row| row.get(0))?;
    if !exists {
        return Err(Error::NotFound("contradicted claim in this scope".into()));
    }
    connection.execute("INSERT INTO contradictions(project,first_id,second_id,task_id,shared) VALUES(?1,?2,?3,?4,?5) ON CONFLICT DO NOTHING", params![project,claim,other,task,task.is_none()])?;
    Ok(())
}

pub struct Promotion<'a> {
    pub claim: &'a str,
    pub version: u64,
    pub actor: &'a str,
    pub reason: &'a str,
}

/// Approves explicit cross-task reuse with an optimistic version and an attributed reason.
pub fn promote(db: &mut Db, promotion: Promotion<'_>) -> Result<Value> {
    validate::id(promotion.claim)?;
    validate::id(promotion.actor)?;
    validate::text(promotion.reason, "promotion reason", 1024)?;
    let expected = model::version(promotion.version)?;
    let tx = db
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let task: Option<Option<String>> = tx
        .query_row(
            "SELECT task_id FROM claims WHERE project=?1 AND id=?2",
            params![db.project, promotion.claim],
            |row| row.get(0),
        )
        .optional()?;
    let task = task.ok_or_else(|| Error::NotFound("claim".into()))?;
    if !conflicts::pairs(
        &tx,
        &db.project,
        task.as_deref(),
        &[promotion.claim.to_owned()],
    )?
    .is_empty()
    {
        return Err(Error::Conflict(
            "claim has an unresolved disagreement in its source scope".into(),
        ));
    }
    let count = tx.execute("UPDATE claims SET shared=1,status='approved',version=version+1 WHERE project=?1 AND id=?2 AND version=?3",
        params![db.project,promotion.claim,expected])?;
    if count != 1 {
        return Err(Error::Conflict("claim version changed".into()));
    }
    tx.execute(
        "UPDATE observations SET shared=1 WHERE project=?1 AND claim_id=?2",
        params![db.project, promotion.claim],
    )?;
    db::event(
        &tx,
        &db.project,
        Event {
            task: task.as_deref(),
            actor: promotion.actor,
            kind: "observation.promote",
            subject: promotion.claim,
            reason: Some(promotion.reason),
        },
    )?;
    db::touch(&tx, &db.project)?;
    tx.commit()?;
    Ok(json!({"claimId":promotion.claim,"version":expected+1,"status":"approved"}))
}
