-- Creates RECON's authoritative scoped memory and transient Librarian request store.
CREATE TABLE projects (
    id TEXT PRIMARY KEY, root TEXT NOT NULL, knowledge_version INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
) STRICT;
CREATE TABLE tasks (
    project TEXT NOT NULL, id TEXT NOT NULL, title TEXT NOT NULL, objective TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','archived')),
    version INTEGER NOT NULL DEFAULT 0 CHECK(version >= 0), checkpoint TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    PRIMARY KEY(project,id), FOREIGN KEY(project) REFERENCES projects(id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE entities (
    project TEXT NOT NULL, id TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('module','file','symbol','concept')),
    canonical_key TEXT NOT NULL, PRIMARY KEY(project,id), UNIQUE(project,kind,canonical_key),
    FOREIGN KEY(project) REFERENCES projects(id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE claims (
    row_id INTEGER PRIMARY KEY, project TEXT NOT NULL, id TEXT NOT NULL,
    entity_id TEXT NOT NULL, task_id TEXT, kind TEXT NOT NULL CHECK(kind IN ('finding','decision','constraint','failure')),
    body TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'candidate' CHECK(status IN ('candidate','approved')),
    shared INTEGER NOT NULL CHECK(shared IN (0,1)), version INTEGER NOT NULL DEFAULT 0 CHECK(version >= 0),
    UNIQUE(project,id), FOREIGN KEY(project) REFERENCES projects(id) ON DELETE RESTRICT,
    FOREIGN KEY(project,entity_id) REFERENCES entities(project,id) ON DELETE RESTRICT,
    FOREIGN KEY(project,task_id) REFERENCES tasks(project,id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX claims_scope ON claims(project,shared,task_id,row_id);
CREATE INDEX claims_entity ON claims(project,entity_id,shared,task_id);
CREATE TABLE observations (
    project TEXT NOT NULL, id TEXT NOT NULL, claim_id TEXT NOT NULL, actor TEXT NOT NULL,
    idempotency_key TEXT NOT NULL, payload_hash TEXT NOT NULL, entity_json TEXT NOT NULL, receipt_json TEXT NOT NULL,
    shared INTEGER NOT NULL CHECK(shared IN (0,1)),
    observed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    PRIMARY KEY(project,id), UNIQUE(project,idempotency_key),
    FOREIGN KEY(project,claim_id) REFERENCES claims(project,id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX observations_claim ON observations(project,claim_id,id);
CREATE TABLE evidence (
    id INTEGER PRIMARY KEY, project TEXT NOT NULL, observation_id TEXT NOT NULL, path TEXT NOT NULL,
    symbol TEXT, line_start INTEGER, line_end INTEGER, source_hash TEXT NOT NULL, commit_sha TEXT,
    observed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    CHECK((line_start IS NULL AND line_end IS NULL) OR (line_start > 0 AND line_end >= line_start)),
    FOREIGN KEY(project,observation_id) REFERENCES observations(project,id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX evidence_observation ON evidence(project,observation_id,id);
CREATE TABLE contradictions (
    project TEXT NOT NULL, first_id TEXT NOT NULL, second_id TEXT NOT NULL, task_id TEXT,
    shared INTEGER NOT NULL CHECK(shared IN (0,1)),
    CHECK(first_id <> second_id), PRIMARY KEY(project,first_id,second_id),
    FOREIGN KEY(project,first_id) REFERENCES claims(project,id) ON DELETE RESTRICT,
    FOREIGN KEY(project,second_id) REFERENCES claims(project,id) ON DELETE RESTRICT,
    FOREIGN KEY(project,task_id) REFERENCES tasks(project,id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX contradictions_second ON contradictions(project,second_id,first_id);
CREATE TABLE edges (
    id INTEGER PRIMARY KEY, project TEXT NOT NULL, source_id TEXT NOT NULL, target_id TEXT NOT NULL,
    relation TEXT NOT NULL CHECK(relation IN ('contains','depends_on','calls','configures','tested_by')),
    claim_id TEXT NOT NULL, UNIQUE(project,source_id,target_id,relation,claim_id),
    FOREIGN KEY(project,source_id) REFERENCES entities(project,id) ON DELETE RESTRICT,
    FOREIGN KEY(project,target_id) REFERENCES entities(project,id) ON DELETE RESTRICT,
    FOREIGN KEY(project,claim_id) REFERENCES claims(project,id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX edges_source ON edges(project,source_id,target_id);
CREATE TABLE events (
    id INTEGER PRIMARY KEY, project TEXT NOT NULL, task_id TEXT, event_type TEXT NOT NULL,
    actor TEXT NOT NULL, subject_id TEXT NOT NULL, reason TEXT,
    occurred_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    FOREIGN KEY(project) REFERENCES projects(id) ON DELETE RESTRICT,
    FOREIGN KEY(project,task_id) REFERENCES tasks(project,id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX events_project ON events(project,id);
CREATE INDEX events_task ON events(project,task_id,id);
CREATE TABLE prepared_requests (
    id TEXT PRIMARY KEY, project TEXT NOT NULL, task_id TEXT, query TEXT NOT NULL,
    budget INTEGER NOT NULL CHECK(budget BETWEEN 128 AND 16384), map_version TEXT NOT NULL,
    allowed_ids TEXT NOT NULL, expires_at INTEGER NOT NULL,
    FOREIGN KEY(project) REFERENCES projects(id) ON DELETE RESTRICT,
    FOREIGN KEY(project,task_id) REFERENCES tasks(project,id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX requests_expiry ON prepared_requests(project,expires_at);
CREATE TABLE metrics (
    id INTEGER PRIMARY KEY, project TEXT NOT NULL, run_id TEXT NOT NULL, data_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    FOREIGN KEY(project) REFERENCES projects(id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX metrics_run ON metrics(project,run_id,id);
CREATE VIRTUAL TABLE claims_fts USING fts5(body,content='claims',content_rowid='row_id');
CREATE TRIGGER claims_insert AFTER INSERT ON claims BEGIN
    INSERT INTO claims_fts(rowid,body) VALUES(new.row_id,new.body);
END;
CREATE TRIGGER claims_delete AFTER DELETE ON claims BEGIN
    INSERT INTO claims_fts(claims_fts,rowid,body) VALUES('delete',old.row_id,old.body);
END;
CREATE TRIGGER claims_update AFTER UPDATE OF body ON claims BEGIN
    INSERT INTO claims_fts(claims_fts,rowid,body) VALUES('delete',old.row_id,old.body);
    INSERT INTO claims_fts(rowid,body) VALUES(new.row_id,new.body);
END;
PRAGMA application_id = 0x5245434F;
PRAGMA user_version = 1;
