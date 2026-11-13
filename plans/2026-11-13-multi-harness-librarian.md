<!-- Records the pre-implementation comparison and proposed first RECON slice. -->
# Multi-harness RECON with early Librarian routing

Status: done for the first CLI slice; user-designed benchmark trials deferred.
Repo: `/srv/atlas/project-recon-multi-harness`
Branch: `feature/multi-harness`
Base: `origin/main@4bf3bcb4fc471882c83f7a2ceca8f17358a307cc`
Started: 2026-11-13

Not in scope: GUI, mandatory MCP, automatic Git indexing, embeddings, production migration, or Copilot implementation.

## Goal

Preserve verified findings and task checkpoints across coding harnesses.
Let an optional Librarian route ambiguous questions to existing knowledge, with bounded evidence-backed output.
Appetite for the first implementation slice: one local repository, one persistent task, and one end-to-end Librarian example.
Implement the first usable slice on this branch; user-designed benchmark trials follow afterwards.

## Source comparison

- At the base commit, the repository contains README, architecture v0.1/v0.2, and annexes 01/02; no implementation or project gate existed.
- ZIP checked: `/home/claud/Downloads/shared-brain-multi-harness.zip`.
- ZIP integrity check passed; it contains 16 entries.
- Neither architecture document exists in the ZIP, by filename or SHA-256 content match.
- There are no shared relative file paths between the ZIP and the repository's current files.
- ZIP: Markdown task records, task-memory skill, Scout adapters, setup scripts, optional checkpoint hook.
- v0.1: SQLite retrieval/graph, integrated repository indexing and Git invalidation, optional retrieval/decision models.
- v0.2: persistent Task Brain, General Brain, transient Session State, and Git investigation through Scout.
- Annex 01: Rust CLI first; SQLite authority only after explicit migration; Markdown remains authoritative in the ZIP prototype.
- Annex 02: optional Librarian, canonical entities, observations/evidence, and separate memory/Librarian experiments.
- These are related design stages, not identical copies of the prototype.

## Decisions

D1-D6 were accepted by the user's instruction to implement.
Boundary contracts are in `docs/cli-contract.md`; review precedes code.
Technical checks run now; benchmark experiments follow the user's testing ideas later.

- D1 — Accepted: create `feature/multi-harness` and `feature/copilot` from the fetched base above, in separate worktrees.
- D2 — Accepted: preserve v0.1's retrieval goals; use v0.2 and the annexes for changed Git, memory-lifetime, and transport choices.
  Rust CLI + SQLite/FTS5 keeps operations local and shared across harnesses; freshness needs explicit verification.
- D3 — Accepted: use explicit persistent task IDs; branch/worktree names are contextual aliases, not permanent task identity.
  General/task/session scopes share one project database, with explicit repository/worktree attribution and scoped queries.
  First tests use an explicit temporary database path; no existing harness memory is migrated or replaced.
- D4 — Accepted: exact lookup and lexical retrieval bypass Librarian; ambiguous conceptual queries can request semantic routing.
  Librarian sees a compact, versioned directory and returns candidate IDs, filters, uncertainty, and verification requests.
  Core validates scope, directory version, IDs, and result bounds before fetching evidence.
  Missing or invalid routes produce an explicit error or bounded direct-search fallback, never invented facts.
- D5 — Accepted: initially invoke Librarian as a role in the current harness through a JSON request/response contract.
  This avoids choosing an API provider prematurely; each harness's invocation and model availability must be verified.
  Model calls remain optional and on demand; provider prompt caching is separate from the versioned directory cache.
- D6 — Accepted: entities, observations, evidence, and append-only events are distinct records.
  Use exact deduplication, idempotency, and version checks; keep conflicts visible and promotion explicit.
  Preserve provenance and source hashes; request targeted Scout verification when freshness matters.
- D7 — Accepted: GitHub contains source, tests, migrations, lockfile and build documentation only. Operational knowledge, SQLite and compiled binaries remain local; a backup repository is a later decision.

## Alternatives and consequences

- Literal v0.1 implementation would add automatic Git indexing that v0.2 and the annexes remove; do not start there.
- Importing the ZIP unchanged would provide Markdown continuity but no structured retrieval or Librarian.
- Sending every query through Librarian would add cost to simple lookups; keep a direct route for comparison.
- A permanent model service or separate provider API can follow only if the initial harness-driven experiment needs it.
- Building RECON in isolation preserves the ZIP baseline; importing real task history requires a later, explicit authority cutover.

## Risks and rollback

- Stale evidence: return verification metadata and uncertainty; Scout checks relevant source files when current behavior matters.
- Cross-task/worktree leakage: filter by explicit project/task/repository context; test isolation on separate temporary stores.
- Concurrent updates: short transactions, foreign keys, busy timeout, idempotency, and optimistic checkpoint versions.
- Hallucinated routes: validate returned IDs and map version before executing retrieval.
- Unbounded context: cap input/output and report token estimates explicitly until a model tokenizer is selected.
- Misleading cache savings: record cache misses, refreshes, map construction, and all indirect model calls.
- Data loss during migration: no live migration in this slice; later migration requires backup and a tested restore command.
- Delivery adds source and build instructions on the feature branch; original architecture documents remain intact.
- No deployment, global installation or existing-store migration occurred. The baseline checkout remains `/srv/atlas/project-recon`.
  New databases must remain local when changing source versions; this delivery performs no worktree or branch deletion.

## Components

| Component | Layer | Contract (in -> out) | New / change | Verification |
| --- | --- | --- | --- | --- |
| Memory store | adapter | scoped records + version -> committed records or typed conflict | new | real temporary SQLite database |
| Task continuity | service | task ID + checkpoint -> resumable task state | new | separate CLI processes resume the same task |
| Direct retrieval | service | scoped query + budget -> evidence-backed context | new | exact/FTS queries and isolated scopes |
| Librarian directory | service | scoped entity metadata -> bounded versioned map | new | deterministic map generation and refresh |
| Librarian routing | boundary + service | map + query -> validated route -> context | new | real harness route and invalid-ID rejection |
| Metrics | boundary | run/component events -> accounting records | new | distinguish measured, estimated, and unavailable values |

## Atoms

Milestones below were implemented in component increments, with technical checks against temporary stores.
Librarian model/provider selection and cross-harness experiments remain for the next testing phase.

| Atom | Component | Role | Effort | After | Check | Status | Cost |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A0 | Contracts | orchestrator | high | discussion | reviewed JSON/storage contracts | done | unmeasured |
| A1 | Memory store | orchestrator | high | A0 | transactional finding/evidence write in a temporary database | done | unmeasured |
| A2 | Task continuity and direct retrieval | orchestrator | high | A1 | resume across processes; exact/FTS retrieval with provenance | done | unmeasured |
| A3 | Librarian directory | orchestrator | medium | A2 | compact map is versioned, bounded, and scoped | done | unmeasured |
| A4 | Librarian routing | orchestrator | high | A3 | scoped routes to real IDs; invalid routes fail safely | done | unmeasured |
| A5 | Metrics and technical acceptance | orchestrator | high | A4 | real temporary CLI/database checks and full Rust gate | done | unmeasured |

## Done when

- A finding has entity identity, evidence, producing agent, task, source revision/hash, and verification status.
- A task resumes from another process/harness without requiring prior conversation history.
- A known ID uses direct retrieval without a model call.
- A conceptual dependency question uses Librarian routing and returns a bounded context package with source pointers.
- Invalid IDs, stale map versions, unresolved contradictions, and cross-scope requests have defined, tested outcomes.
- Baseline arms remain A: orchestrator handoff, B: no supplied context, C: direct RECON; D adds Librarian routing.
- Deferred after this delivery: pilot runs repeat three times per task/arm, with individual outcomes, failures, and cold/warm cache conditions.
- Include memory construction, maintenance, Librarian, Scout, implementer, and review costs; make no savings claim without results.
- Before code completion, run the Rust gate: format, clippy with warnings denied, tests, release build.
- Cross-harness operation is reported UNTESTED until exercised in each named harness; CLI-process tests alone do not prove it.

## Log

- 2026-11-13 17:55 EET — Fetched origin; main and origin/main are identical at the base above; working tree was clean.
- 2026-11-13 — Read both architecture documents, both annexes, README, and every ZIP entry; compared names and SHA-256 contents.
- 2026-11-13 — Created both local feature branches and worktrees; Copilot worktree remains unchanged.
- 2026-11-13 — Reserved D1-D6 for this draft. Recorded proposed implementation choices before code, as requested.
- 2026-11-13 — Implementation and runtime tests have not started. Next step: discuss this proposal, then finalize A0 contracts.
- 2026-11-13 — User authorized implementation and deferred experimental trials. Added CLI/storage/routing contracts for pre-build review.
- 2026-11-13 — Implement this feature in small component increments; total scope necessarily exceeds one three-file/150-line atom. No Copilot edits or live-memory migration.
- 2026-11-13 — Plan review closed: visible metadata/edge endpoints and contradiction IDs; atomic map snapshots; idempotency before source reads; deferred benchmark gate.
- 2026-11-13 — Added Rust CLI services, SQLite schema/FTS triggers, task CAS, observation/evidence storage, graph retrieval, Librarian prepare/resolve, metrics and adapters.
- 2026-11-13 — Initial acceptance red: `test result: FAILED. 0 passed; 10 failed`. Expanded routing tests caught omitted-node edges and missing result truncation; both fixed.
- 2026-11-13 — Each added integrity/routing test also demonstrated failure through a targeted temporary guard mutation; all production guards restored. Snapshot/CAS races repeat 12 times.
- 2026-11-13 — Pre-review technical suite: 24 tests passed. CLI limits are enforced by clippy; direct dependency versions checked against crates.io; audit found no advisory findings.
- 2026-11-13 — Source-only constraint D7 added; ignore rules verified for SQLite/WAL/SHM, compiled output and local library paths. Main and Copilot worktrees remain unchanged.
- 2026-11-13 — Independent implementation review found four gaps; regression run reproduced all five related cases (`0 passed; 5 failed`). Added per-contribution approval, scoped dispute views, distinct lookup ranking/truncation and confined source opening.
- 2026-11-13 — Source-swap race also failed on the original implementation; confined directory reads passed 300 concurrent swaps. JSON inputs reject special files. New filesystem dependencies checked against official registry and pinned; releases younger than three days were excluded.
- 2026-11-13 — Fix round: clippy and all 31 technical tests pass. Independent re-review and full release gate remain in progress.
- 2026-11-13 — Re-review caught a promotion CAS race; a new regression failed before the fix. New contributions now advance claim versions, so stale approval cannot publish unseen evidence. Final reviewer found no remaining blockers.
- 2026-11-13 — Final `make check` exited 0: `RECON gate PASS`. Format, clippy, 32 technical tests, release build and dependency audit passed. Indexed claim/journal/metrics query plans verified on 1,000 synthetic entities.
- 2026-11-13 — Release CLI smoke passed: separate invocations resumed a checkpoint; a conceptual request routed to Task continuity through prepare/resolve; current source evidence and SQLite integrity verified. Temporary smoke state removed.
- 2026-11-13 — Source-only stage audit passed: no SQLite, knowledge library or compiled output. Claude/other harnesses, Windows and model/cache cost savings remain UNTESTED; custom benchmark experiments deferred to the user.
