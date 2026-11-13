//! Defines typed external records for checkpoints, observations, and Librarian routes.

use crate::{
    error::{Error, Result},
    validate,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub expected_version: u64,
    pub actor: String,
    pub phase: String,
    pub progress: Vec<String>,
    pub decisions: Vec<String>,
    pub constraints: Vec<String>,
    pub blockers: Vec<String>,
    pub next_actions: Vec<String>,
    pub handoff: Option<String>,
}

impl Checkpoint {
    /// Checks version range and checkpoint field limits before a transaction starts.
    pub fn validate(&self) -> Result<()> {
        version(self.expected_version)?;
        validate::id(&self.actor)?;
        validate::text(&self.phase, "phase", 128)?;
        for list in [
            &self.progress,
            &self.decisions,
            &self.constraints,
            &self.blockers,
            &self.next_actions,
        ] {
            validate::strings(list, "checkpoint list", 20, 512)?;
        }
        if let Some(value) = &self.handoff {
            validate::text(value, "handoff", 2048)?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entity {
    pub kind: EntityKind,
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Module,
    File,
    Symbol,
    Concept,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    Finding,
    Decision,
    Constraint,
    Failure,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceInput {
    pub path: String,
    pub symbol: Option<String>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub source_hash: Option<String>,
    pub commit: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observation {
    pub task_id: Option<String>,
    pub actor: String,
    pub idempotency_key: String,
    pub entity: Entity,
    pub kind: ClaimKind,
    pub text: String,
    pub evidence: Vec<EvidenceInput>,
    pub contradicts: Option<String>,
}

impl Observation {
    /// Validates observation identity and bounded contribution fields; paths are checked separately.
    pub fn validate(&self) -> Result<()> {
        if let Some(task) = &self.task_id {
            validate::id(task)?;
        }
        validate::id(&self.actor)?;
        validate::id(&self.idempotency_key)?;
        validate::text(&self.entity.key, "entity key", 512)?;
        validate::text(&self.entity.name, "entity name", 256)?;
        validate::strings(&self.entity.aliases, "aliases", 12, 128)?;
        validate::text(&self.text, "claim text", 8192)?;
        if !(1..=8).contains(&self.evidence.len()) {
            return Err(Error::Invalid(
                "evidence requires 1-8 source pointers".into(),
            ));
        }
        for source in &self.evidence {
            source.validate()?;
        }
        if let Some(claim) = &self.contradicts {
            validate::id(claim)?;
        }
        Ok(())
    }
}

impl EvidenceInput {
    /// Checks pointer metadata; hash capture later checks containment and current bytes.
    pub fn validate(&self) -> Result<()> {
        validate::text(&self.path, "evidence path", 1024)?;
        if let Some(symbol) = &self.symbol {
            validate::text(symbol, "symbol", 512)?;
        }
        match (self.line_start, self.line_end) {
            (None, None) => (),
            (Some(start), Some(end)) if start > 0 && end >= start => (),
            _ => {
                return Err(Error::Invalid(
                    "lineStart/lineEnd must be an ordered positive pair".into(),
                ));
            }
        }
        if let Some(hash) = &self.source_hash
            && (hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(Error::Invalid(
                "sourceHash must be a SHA-256 hexadecimal digest".into(),
            ));
        }
        if let Some(commit) = &self.commit
            && (!(7..=64).contains(&commit.len()) || !commit.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(Error::Invalid(
                "commit must be a 7-64 character hexadecimal revision".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Route {
    pub request_id: String,
    pub map_version: String,
    pub entity_ids: Vec<String>,
    pub needs_scout: bool,
    pub uncertainty: Vec<String>,
}

impl Route {
    /// Checks route size and duplicate IDs before looking up the prepared request.
    pub fn validate(&self) -> Result<()> {
        validate::id(&self.request_id)?;
        validate::id(&self.map_version)?;
        validate::strings(&self.uncertainty, "uncertainty", 8, 256)?;
        if self.entity_ids.len() > 20 {
            return Err(Error::InvalidRoute("at most 20 entity IDs".into()));
        }
        let mut seen = std::collections::HashSet::new();
        for id in &self.entity_ids {
            validate::id(id)?;
            if !seen.insert(id) {
                return Err(Error::InvalidRoute("duplicate entity ID".into()));
            }
        }
        Ok(())
    }
}

/// Rejects versions that cannot increment safely inside SQLite's signed integer range.
pub fn version(value: u64) -> Result<i64> {
    i64::try_from(value)
        .ok()
        .filter(|v| *v < i64::MAX)
        .ok_or_else(|| Error::Invalid("version exceeds supported integer range".into()))
}
