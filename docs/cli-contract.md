<!-- Defines RECON's first executable CLI and harness Librarian protocol. -->
# CLI contract v1

Run `recon --db <explicit-path> --project <id> <command>`.
Only `init` creates databases. No network, full scans, model API calls, or existing-memory migration.
The user's benchmark experiments follow implementation; technical checks run during development.

## Boundaries and persistence

- Success: `{schemaVersion: 1, ok: true, data: ...}`; failure: `{schemaVersion: 1, ok: false, error: {code, message}}` and nonzero exit.
- Help/version remain CLI text. JSON uses camelCase and rejects unknown/duplicate fields.
- Input files: at most 64 KiB. IDs: 1-128 ASCII letters, digits, `_`, `-`, `.`. Query: at most 1,024 bytes.
- SQLite: application ID and schema version, STRICT tables, WAL, foreign keys, FULL synchronous writes, immediate transactions, five-second busy timeout.
- Existing non-RECON databases are rejected without migration. UTC RFC 3339 timestamps.
- Database scopes prevent accidental mixing; they do not isolate hostile users with direct file access.

## Tasks

- `init --repo <directory>` registers the canonical repository root; same root is idempotent, a changed root conflicts.
- `task open <id> --title <title> --objective <objective>` creates or returns an identical task. IDs survive harness/branch changes.
- `task resume <id> --budget 1200` returns its checkpoint.
- `task checkpoint <id> --input <json>` accepts `expectedVersion`, `actor`, `phase`, `progress`, `decisions`, `constraints`, `blockers`, `nextActions`, optional `handoff`.
- Task version starts at zero; stale checkpoints conflict without changing state. Arrays: 20 entries, 512 bytes each; title: 256 bytes; objective: 4,096 bytes.
- `task archive <id> --expected-version <n> --actor <actor>` archives without deletion; archived tasks reject checkpoint writes.
- Budgets: 128-16,384 estimated tokens. Compact JSON envelope is capped at `4 * budget` UTF-8 bytes; essential content that cannot fit returns `budget_exceeded`.
- Accounting explicitly uses `utf8BytesDiv4`, an approximation rather than a model-specific token guarantee.

## Knowledge

`observation add --input <json>` accepts this shape:

```json
{
  "taskId": "ci-migration", "actor": "codex", "idempotencyKey": "finding-001",
  "entity": {"kind": "module", "key": "ci.runner", "name": "Runner", "aliases": ["executor"]},
  "kind": "finding", "text": "Each job has an isolated build directory.",
  "evidence": [{"path": "src/runner.rs", "symbol": "run_job", "lineStart": 1, "lineEnd": 10}],
  "contradicts": null
}
```

- Optional `taskId`: omission selects general scope. Entity kinds: module/file/symbol/concept. Claim kinds: finding/decision/constraint/failure.
- Entity identity: project + kind + case-sensitive key. At most 12 aliases; claim text: 8,192 bytes; evidence: 1-8 pointers.
- Evidence paths are root-relative; reject traversal and symlink escapes. SHA-256 hashes cover at most 16 MiB per file; optional `sourceHash` must match. Optional `commit` records provenance only.
- Matching bytes do not prove semantic truth. Claims start `candidate`.
- Canonical claim identity includes entity, task/general scope, kind, whitespace-normalized text; every contribution retains its own observation and evidence.
- Appending a new contribution advances its claim version; idempotent retries do not. Promotion rejects a version inspected before another contribution arrived.
- Check idempotency before filesystem reads: identical keys/payloads return the original receipt even after sources change or disappear; changed payload conflicts.
- Optional `contradicts` must name a visible claim; preserve both and mark both `disputed`.
- `observation promote <claim-id> --expected-version <n> --actor <actor> --reason <reason>` explicitly exposes a non-disputed task claim to general retrieval, with audit. Promotion is attributed approval, not model-generated proof.
- `entity relate <from-id> <to-id> --relation <kind> --claim <id>` adds contains/depends_on/calls/configures/tested_by. Supporting claim must belong to the source entity; relation visibility follows its claim.

## Retrieval

- `context search <query> [--task <id>] [--entity <id>] [--budget 1200]`: exact ID/key/name, then safely quoted lexical FTS5; task scope includes its claims plus general/promoted knowledge.
- Return IDs, statuses, evidence pointers, visible unresolved contradiction IDs, omission counts, and `needsScout`. Candidate claims remain explicitly unverified.
- Labels/aliases and evidence come only from visible contributions. Promotion approves contributions present at that moment; later task contributions require another approval. Graph edges require visible supporting claims and endpoints.
- Disagreement events have their own scope; disputed status is derived from visible disagreements and never changes a general view due to a hidden task event.
- Lookup deduplicates before selecting at most 20 entities. At most 50 candidate claims and three evidence pointers per claim enter the builder; budgeting omits whole claims. Relationship expansion: one hop, at most 20 entities.
- `lookupTruncated` signals extra lookup/graph matches; `omittedCount` counts omitted claims among selected entities. `truncated` covers either limitation.
- Evidence uses a confined directory capability and checks the opened handle; Unix opens are nonblocking so special-file swaps cannot wait for a FIFO writer.
- `evidence verify <claim-id> [--task <id>]` compares targeted source bytes and reports current/changed/missing; it never silently promotes claims.
- `journal [--task <id>] [--after <id>] [--limit 20]` returns ordered events, limit at most 100.

## Librarian

1. `librarian prepare <query> [--task <id>] [--budget 1200] [--limit 100]` returns a versioned directory and routing instructions.
2. A read-only harness role returns `{requestId, mapVersion, entityIds: [...], needsScout: false, uncertainty: []}`.
3. `librarian resolve --input <route.json>` validates routing and retrieves evidence under the original scope/budget.

- Directory contains labels, aliases, IDs and relationships; no claim bodies or code excerpts. Maximum: 100 entities, 100 relations, 64 KiB serialized envelope.
- Prepared requests are Session State: bound to project/task/query/budget/map/allowlist, valid for one hour. `librarian prune` deletes expired requests only.
- Prepare captures directory/version/allowlist in one transaction; resolve validates expiry/version and retrieves within one read snapshot, including under concurrent writes.
- Knowledge mutations increment project map version. Old routes return `stale_map`.
- Reject wrong project, missing/expired request, invented/out-of-map IDs, duplicate IDs, more than 20 IDs, and malformed uncertainty arrays.
- Empty routes return no invented facts and set `needsScout`.
- Harness directory reuse and provider prompt caching are separate. The CLI promises no provider caching or billing savings.
- Librarian has no SQL/shell execution capability through this protocol; the harness performs its semantic reasoning step.

## Metrics and acceptance

- `metrics record --input <json>` appends run ID, role, model, optional token/cache counters, optional USD micro-cost, durationMs, and an explicit estimated flag.
- `metrics export --run <id> [--after <id>] [--limit 100]` preserves null for unavailable values and supplies a cursor.
- Journal records operation names and IDs, not raw prompts or evidence contents.
- Tests use real temporary databases, independent processes, failed writes and concurrent checkpoints. New tests run red first.
- Gate: format check, clippy with warnings denied, tests, release build, dependency audit.
- A/B/C/D experiments and operation in other harnesses remain UNTESTED until the user's subsequent testing phase.
