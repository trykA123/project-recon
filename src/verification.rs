//! Reports current evidence freshness without mutating claim truth or promotion status.

use crate::{
    catalog,
    db::{self, Db},
    error::{Error, Result},
    source, validate,
};
use rusqlite::params;
use serde_json::{Value, json};

/// Compares up to 100 captured pointers with current files; missing files stay explicit.
pub fn verify(db: &mut Db, claim: &str, task: Option<&str>) -> Result<Value> {
    validate::id(claim)?;
    let root = db.root()?;
    let tx = db.connection.transaction()?;
    db::scope(&tx, &db.project, task)?;
    let visible: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM claims WHERE project=?1 AND id=?3 AND (shared=1 OR task_id=?2))",
        params![db.project,task,claim], |row| row.get(0))?;
    if !visible {
        return Err(Error::NotFound("claim in this scope".into()));
    }
    let mut evidence = catalog::rows(&tx,
        "SELECT json_object('id',e.id,'path',e.path,'sourceHash',e.source_hash,'observationId',o.id,'actor',o.actor)
         FROM observations o JOIN evidence e ON e.project=o.project AND e.observation_id=o.id
         JOIN claims c ON c.project=o.project AND c.id=o.claim_id
         WHERE o.project=?1 AND o.claim_id=?2 AND (o.shared=1 OR c.task_id=?3) ORDER BY e.id DESC LIMIT 101", params![db.project,claim,task])?;
    let truncated = evidence.len() > 100;
    evidence.truncate(100);
    for pointer in &mut evidence {
        let path = pointer["path"]
            .as_str()
            .ok_or_else(|| Error::Invalid("stored evidence path is invalid".into()))?;
        let freshness = match source::hash(&root, path) {
            Ok(hash) if pointer["sourceHash"] == hash => "current",
            Ok(_) => "changed",
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => "missing",
            Err(error) => return Err(error),
        };
        pointer["freshness"] = json!(freshness);
    }
    tx.commit()?;
    Ok(
        json!({"claimId":claim,"evidence":evidence,"truncated":truncated,"semanticVerification":"not_performed"}),
    )
}
