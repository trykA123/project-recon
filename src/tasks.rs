//! Preserves task checkpoints across independent CLI invocations with optimistic versions.

use crate::{
    db::{self, Db, Event},
    error::{Error, Result},
    model::{self, Checkpoint},
    output, validate,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;

/// Returns a task's current materialized state; missing IDs fail within project scope.
pub fn read(connection: &Connection, project: &str, id: &str) -> Result<Value> {
    let value: Option<String> = connection.query_row(
        "SELECT json_object('id',id,'title',title,'objective',objective,'status',status,'version',version,
         'checkpoint',json(checkpoint),'updatedAt',updated_at) FROM tasks WHERE project=?1 AND id=?2",
        params![project,id], |row| row.get(0)).optional()?;
    Ok(serde_json::from_str(
        &value.ok_or_else(|| Error::NotFound("task".into()))?,
    )?)
}

/// Creates one task idempotently; changing an existing task's objective conflicts.
pub fn open(db: &mut Db, id: &str, title: &str, objective: &str) -> Result<Value> {
    validate::id(id)?;
    validate::text(title, "title", 256)?;
    validate::text(objective, "objective", 4096)?;
    let tx = db
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let inserted = tx.execute("INSERT INTO tasks(project,id,title,objective) VALUES(?1,?2,?3,?4) ON CONFLICT(project,id) DO NOTHING", params![db.project,id,title,objective])?;
    let value = read(&tx, &db.project, id)?;
    if value["title"] != title || value["objective"] != objective {
        return Err(Error::Conflict(
            "task ID already has another title or objective".into(),
        ));
    }
    if inserted > 0 {
        db::event(
            &tx,
            &db.project,
            Event {
                task: Some(id),
                actor: "cli",
                kind: "task.open",
                subject: id,
                reason: None,
            },
        )?;
    }
    tx.commit()?;
    Ok(value)
}

/// Returns a task checkpoint within a serialized output budget, or an explicit budget error.
pub fn resume(db: &Db, id: &str, budget: usize) -> Result<Value> {
    validate::id(id)?;
    output::bounded(read(&db.connection, &db.project, id)?, budget)
}

/// Atomically replaces a checkpoint only when the caller's version still matches.
pub fn checkpoint(db: &mut Db, id: &str, input: &Checkpoint) -> Result<Value> {
    input.validate()?;
    validate::id(id)?;
    let expected = model::version(input.expected_version)?;
    let tx = db
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    db::task(&tx, &db.project, id, true)?;
    let count = tx.execute("UPDATE tasks SET checkpoint=?1,version=version+1,updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
        WHERE project=?2 AND id=?3 AND version=?4 AND status='active'", params![serde_json::to_string(input)?,db.project,id,expected])?;
    if count != 1 {
        return Err(Error::Conflict(
            "checkpoint version changed; resume before retrying".into(),
        ));
    }
    db::event(
        &tx,
        &db.project,
        Event {
            task: Some(id),
            actor: &input.actor,
            kind: "task.checkpoint",
            subject: id,
            reason: None,
        },
    )?;
    let value = read(&tx, &db.project, id)?;
    tx.commit()?;
    Ok(value)
}

/// Archives a task without deleting its observations, using the current expected version.
pub fn archive(db: &mut Db, id: &str, version: u64, actor: &str) -> Result<Value> {
    validate::id(actor)?;
    validate::id(id)?;
    let expected = model::version(version)?;
    let tx = db
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    db::task(&tx, &db.project, id, true)?;
    let count = tx.execute("UPDATE tasks SET status='archived',version=version+1,updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
        WHERE project=?1 AND id=?2 AND version=?3 AND status='active'", params![db.project,id,expected])?;
    if count != 1 {
        return Err(Error::Conflict("task version changed".into()));
    }
    db::event(
        &tx,
        &db.project,
        Event {
            task: Some(id),
            actor,
            kind: "task.archive",
            subject: id,
            reason: None,
        },
    )?;
    let value = read(&tx, &db.project, id)?;
    tx.commit()?;
    Ok(value)
}
