//! Selects distinct visible entities and reports when exact or lexical lookup exceeds its cap.

use crate::error::Result;
use rusqlite::{Connection, params};

pub struct Selection {
    pub ids: Vec<String>,
    pub truncated: bool,
}

impl Selection {
    /// Caps already distinct ordered candidates while preserving their omission signal.
    fn capped(mut ids: Vec<String>) -> Self {
        let truncated = ids.len() > 20;
        ids.truncate(20);
        Self { ids, truncated }
    }
}

/// Finds exact visible identifiers, labels, and aliases before lexical retrieval.
fn exact(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    query: &str,
) -> Result<Selection> {
    let mut statement=connection.prepare(
        "SELECT DISTINCT c.entity_id FROM claims c JOIN entities e ON e.project=c.project AND e.id=c.entity_id
         JOIN observations o ON o.project=c.project AND o.claim_id=c.id
         WHERE c.project=?1 AND (c.shared=1 OR c.task_id=?2) AND (o.shared=1 OR c.task_id=?2) AND
          (c.id=?3 OR e.id=?3 OR e.canonical_key=?3 OR lower(json_extract(o.entity_json,'$.name'))=lower(?3)
           OR EXISTS(SELECT 1 FROM json_each(json_extract(o.entity_json,'$.aliases')) WHERE lower(value)=lower(?3)))
         ORDER BY c.entity_id LIMIT 21")?;
    let ids = statement
        .query_map(params![project, task, query], |row| row.get(0))?
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(Selection::capped(ids))
}

/// Searches quoted FTS tokens and ranks distinct entities before applying the 20-entity limit.
fn lexical(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    query: &str,
) -> Result<Selection> {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|term| !term.is_empty())
        .take(16)
        .map(|term| format!("\"{term}\""))
        .collect();
    if terms.is_empty() {
        return Ok(Selection::capped(Vec::new()));
    }
    let mut statement=connection.prepare(
        "WITH hits AS MATERIALIZED (
          SELECT c.entity_id,bm25(claims_fts) AS score FROM claims_fts JOIN claims c ON c.row_id=claims_fts.rowid
          WHERE claims_fts MATCH ?3 AND c.project=?1 AND (c.shared=1 OR c.task_id=?2))
         SELECT entity_id FROM hits GROUP BY entity_id ORDER BY min(score),entity_id LIMIT 21")?;
    let ids = statement
        .query_map(params![project, task, terms.join(" OR ")], |row| row.get(0))?
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(Selection::capped(ids))
}

/// Returns exact matches when available, otherwise a bounded lexical entity selection.
pub fn find(
    connection: &Connection,
    project: &str,
    task: Option<&str>,
    query: &str,
) -> Result<Selection> {
    let selected = exact(connection, project, task, query)?;
    if selected.ids.is_empty() {
        lexical(connection, project, task, query)
    } else {
        Ok(selected)
    }
}
