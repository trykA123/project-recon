//! Reads only scoped entity metadata and parses bounded SQLite JSON projections.

use crate::{db, error::Result};
use rusqlite::{Connection, Params, params};
use serde_json::Value;

/// Reads bounded JSON projections; every caller supplies static SQL and bound parameters.
pub fn rows(connection: &Connection, sql: &str, parameters: impl Params) -> Result<Vec<Value>> {
    let mut statement = connection.prepare(sql)?;
    let encoded = statement
        .query_map(parameters, |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    encoded
        .into_iter()
        .map(|value| Ok(serde_json::from_str(&value)?))
        .collect()
}

/// Returns compact entity labels from visible contributions, excluding private aliases.
pub fn directory(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    limit: usize,
) -> Result<Vec<Value>> {
    db::scope(connection, project, task)?;
    rows(
        connection,
        "WITH visible AS (
          SELECT e.id,e.kind,e.canonical_key,o.entity_json,
                 row_number() OVER(PARTITION BY e.id ORDER BY o.rowid DESC) AS position
          FROM claims c JOIN entities e ON e.project=c.project AND e.id=c.entity_id
          JOIN observations o ON o.project=c.project AND o.claim_id=c.id
          WHERE c.project=?1 AND (c.shared=1 OR c.task_id=?2) AND (o.shared=1 OR c.task_id=?2))
        SELECT json_object('entityId',id,'kind',kind,'key',canonical_key,
          'name',json_extract(entity_json,'$.name'),'aliases',json_extract(entity_json,'$.aliases'))
        FROM visible WHERE position=1 ORDER BY id LIMIT ?3",
        params![project, task, i64::try_from(limit).unwrap_or(101)],
    )
}

/// Checks that an entity has at least one claim visible in the requested task/general scope.
pub fn visible_entity(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    id: &str,
) -> Result<bool> {
    Ok(connection.query_row("SELECT EXISTS(SELECT 1 FROM claims WHERE project=?1 AND entity_id=?3 AND (shared=1 OR task_id=?2))",
        params![project,task,id], |row| row.get(0))?)
}
