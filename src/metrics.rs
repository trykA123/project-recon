//! Stores explicit run accounting and exposes bounded cursor-based audit history.

use crate::{
    catalog,
    db::{self, Db},
    error::{Error, Result},
    validate,
};
use rusqlite::{TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Measurement {
    pub run_id: String,
    pub role: String,
    pub model: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub cost_usd_micros: Option<u64>,
    pub duration_ms: u64,
    pub estimated: bool,
}

/// Appends externally supplied accounting without replacing unknown measurements with zero.
pub fn record(db: &mut Db, input: &Measurement) -> Result<Value> {
    validate::id(&input.run_id)?;
    validate::id(&input.role)?;
    validate::text(&input.model, "model", 256)?;
    for counter in [
        input.input_tokens,
        input.output_tokens,
        input.cache_read_tokens,
        input.cache_write_tokens,
        input.cost_usd_micros,
        Some(input.duration_ms),
    ]
    .into_iter()
    .flatten()
    {
        if i64::try_from(counter).is_err() {
            return Err(Error::Invalid(
                "measurement exceeds supported integer range".into(),
            ));
        }
    }
    let tx = db
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(
        "INSERT INTO metrics(project,run_id,data_json) VALUES(?1,?2,?3)",
        params![db.project, input.run_id, serde_json::to_string(input)?],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok(json!({"id":id,"runId":input.run_id}))
}

/// Validates a cursor and capped page length before any list query.
fn page(after: i64, limit: usize) -> Result<i64> {
    if after < 0 || !(1..=100).contains(&limit) {
        return Err(Error::Invalid(
            "cursor must be nonnegative and limit must be 1-100".into(),
        ));
    }
    i64::try_from(limit + 1).map_err(|_| Error::Budget)
}

/// Exports one project's run records in insertion order with an optional continuation cursor.
pub fn export(db: &Db, run: &str, after: i64, limit: usize) -> Result<Value> {
    validate::id(run)?;
    let rows = catalog::rows(
        &db.connection,
        "SELECT json_set(data_json,'$.id',id,'$.createdAt',created_at) FROM metrics
         WHERE project=?1 AND run_id=?2 AND id>?3 ORDER BY id LIMIT ?4",
        params![db.project, run, after, page(after, limit)?],
    )?;
    paginate(rows, "records", limit)
}

/// Lists only general or selected-task events without returning claim or source contents.
pub fn journal(db: &Db, task: Option<&str>, after: i64, limit: usize) -> Result<Value> {
    db::scope(&db.connection, &db.project, task)?;
    let rows = catalog::rows(&db.connection,
        "SELECT json_object('id',id,'taskId',task_id,'type',event_type,'actor',actor,'subjectId',subject_id,'reason',reason,'occurredAt',occurred_at)
         FROM events WHERE project=?1 AND (task_id IS NULL OR task_id=?2) AND id>?3 ORDER BY id LIMIT ?4",
        params![db.project,task,after,page(after,limit)?])?;
    paginate(rows, "events", limit)
}

/// Creates a bounded page and exposes its last record ID only when another page exists.
fn paginate(mut rows: Vec<Value>, key: &str, limit: usize) -> Result<Value> {
    let more = rows.len() > limit;
    rows.truncate(limit);
    let cursor = if more {
        rows.last()
            .map(|row| row["id"].clone())
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let mut data = json!({"nextCursor":cursor});
    data[key] = json!(rows);
    Ok(data)
}
