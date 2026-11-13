//! Resolves disagreement visibility from its contribution scope and both claims' visibility.

use crate::{catalog, error::Result};
use rusqlite::{Connection, params};
use serde_json::Value;

/// Returns up to ten visible disagreements per selected claim, capped at 500 rows.
pub fn pairs(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    ids: &[String],
) -> Result<Vec<Value>> {
    catalog::rows(
        connection,
        "WITH selected AS (
         SELECT s.value AS claim_id,x.first_id,x.second_id,
          row_number() OVER(PARTITION BY s.value ORDER BY x.first_id,x.second_id) AS position
         FROM json_each(?3) s JOIN contradictions x ON x.first_id=s.value OR x.second_id=s.value
         JOIN claims a ON a.project=x.project AND a.id=x.first_id
         JOIN claims b ON b.project=x.project AND b.id=x.second_id
         WHERE x.project=?1 AND (a.shared=1 OR a.task_id=?2) AND (b.shared=1 OR b.task_id=?2)
          AND (x.shared=1 OR x.task_id=?2)
         ) SELECT DISTINCT json_array(first_id,second_id) FROM selected WHERE position<=10
         ORDER BY first_id,second_id LIMIT 500",
        params![project, task, serde_json::to_string(ids)?],
    )
}
