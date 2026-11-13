# Project RECON — Architecture Annex 02
## Librarian, Evidence, Consolidation, and Benchmark Protocol

**Status:** Design hypotheses and experiment plan — not approved implementation requirements  
**Date:** 2026-11-13  
**Related:** `RECON_Architecture_Annex_01_CLI_Core.md`; `shared-brain-multi-harness.zip`  
**Change policy:** This is a separate annex. The existing ZIP and its Markdown-first MVP are not modified.

## 1. What we are testing

RECON aims to provide relevant, trustworthy context across agents, harnesses, and multi-day tasks with lower **total** token spend and no degradation in implementation quality. We need to test two different propositions:

1. **Memory hypothesis:** a structured, shared repository/task memory improves task outcomes or lowers end-to-end cost relative to ordinary orchestrator handoffs and no supplied context.
2. **Librarian hypothesis:** a small, optionally cached semantic navigator can route complex questions to the right knowledge faster or more cheaply than deterministic retrieval alone.

Do not treat a win for RECON's stored knowledge as proof that a Librarian is useful. Test those separately.

## 2. Proposed component roles

- **RECON Core (Rust + SQLite)**: canonical storage, scoped records, structured retrieval, FTS5, entity resolution, provenance, transactional writes, IDs, query limits, event logging. CLI-first; no mandatory MCP or continuously running service. JSON output recommended for agents.
- **Librarian (optional small model)**: a *map of the library*, not the books. Understands domains, task families, repository modules, relationship types, synonyms, and likely locations. Produces routing hints (IDs, namespaces, topic filters, query plans, confidence/uncertainty), not giant answers. May request verification or semantic consolidation. It is not the unconditional gate for all queries.
- **Scout (on demand)**: fetches evidence from code or Git, verifies potentially outdated or contradictory claims and reports source identifiers, commit/ref, and observed facts. Git is not integrated as an always-on RECON subsystem.
- **Consolidation / evaluation (periodic role, possibly the same small model)**: groups similar claims, proposes summaries, resolves only well-supported duplicates, evaluates retrieval traces, and recommends indexed-data or policy improvements. Changes are checked before being committed.
- **Future Studio (GPUI)**: optional read-only graph, history, inspector and timeline; no GUI requirement for v1.

The roles above need not correspond to separate persistent processes or separately trained models.

## 3. Librarian routing and caching

A librarian's cached *directory* might contain project IDs, architecture-map labels, ownership scopes, known topic synonyms, graph relationship types, and a summary of available collections. It should **not** contain full functions, massive document excerpts, or the whole database.

Request example: `Where are the reprog initialization dependencies?` → Librarian returns `{project, module, entity_ids or search filters, candidate related concepts, freshness hint}` → Core fetches a small validated context package → caller decides whether it needs deeper evidence.

Simple exact lookup should bypass the Librarian entirely. Semantic searches can use embeddings/vector search if benchmarking proves value; embeddings also carry generation, indexing, refresh, and storage costs. If a Librarian is called, provide tool-constrained routes and validate IDs against SQLite to avoid invented shelf locations.

**Cache limitations:** cached prefixes can reduce reprocessing charges only when the chosen provider/harness supports them and prefix bytes remain reusable within its retention rules. Cache is not automatically free or persistent forever; refresh and cache misses cost money. A cached map becomes stale; keep it versioned, compact, and separated from dynamic per-request context. An application-level map cache and a provider-side prompt cache are different things. Measure both explicitly.

## 4. Shared knowledge without duplicates

Store **canonical entities**, **claims/observations**, **evidence/provenance**, and **status** separately.

- Canonical entity: repository + qualified symbol + signature or another stable identifier, with path and revision information where relevant.
- Observation: agent, task, assertion, timestamp, source revision, evidence pointer, status. Agents append; do not silently overwrite one another.
- Exact duplicates: deterministic normalized identity/hash where safe; connect new corroboration to the existing claim rather than duplicating its content.
- Near duplicates: possible groups, not automatic merges. Meaning can differ even with near-identical text.
- Contradictions: preserve competing evidence; mark unresolved until verified. Do not decide by agent majority alone.
- Current retrieval view: a concise best-supported synthesis plus any consequential unresolved disagreement, with links to original observations.

Use append-only event history for important changes, with mutable **materialized views** for fast retrieval. Immutable events do not, by themselves, eliminate semantic conflicts. Use transactions, unique constraints, foreign keys, idempotency keys, and optimistic version checks as needed. Prefer archive/supersede over destructive deletion.

## 5. Three core benchmark arms, plus one ablation

Run an identical set of representative implementation tasks under controlled conditions:

| Arm | Context available to implementer | Question answered |
|---|---|---|
| A — Orchestrator handoff | Relevant context packaged by orchestrator, as in a normal workflow | Is a conventional handoff sufficient? |
| B — No pre-supplied context | Only task specification and ordinary repository/tools; can investigate as needed | What is the baseline cost of rediscovery? |
| C — RECON direct | Same task plus CLI access to prepared RECON knowledge through deterministic SQLite retrieval | Does stored RECON knowledge help? |
| D — RECON + Librarian | Same RECON knowledge, with optional cached semantic routing | Does Librarian beat direct RECON retrieval? |

The user's original three arms are A–C; D is a separate ablation to isolate the incremental value of Librarian. Do **not** replace the original comparison. For a fuller test, evaluate Librarian-on versus Librarian-off under comparable cache conditions.

**Repeat 3 times per task per arm** for an initial pilot and report **median** cost/time plus the individual runs, spread, and failures. Three repetitions are a noisy screening test, not enough for confident significance claims. Increase independent tasks and runs before making strong claims. Use the same test set, repo revision, acceptance criteria, model families, harness versions, and permitted tools. Randomize arm order and fresh-isolate agents/conversations/worktrees to prevent leakage; do not allow the baseline arms to read RECON's artifacts. Never reuse a solved implementation as the starting state for a later arm.

Task mix should include: an easy factual lookup; a cross-module dependency investigation; a real but bounded code change; a multi-agent overlapping-symbol change; a multi-day continuation/handoff; a stale-memory or contradictory-evidence case. Use hidden acceptance tests or human review where possible. Define task success before the run.

## 6. What to measure — do not hide memory-building costs

Collect, per run and per component:

- **Input tokens** and **output tokens**, separated by model/provider; report cache-read, cache-write, and uncached tokens when available.
- **Model cost** in actual billed units where possible; token counts alone are not directly comparable across different models and cache pricing.
- **Memory creation/indexing** cost: Scout calls, LLM summarization, embedding creation, deduplication and repairs, graph construction, source verification, and setup.
- **Memory maintenance** cost: Librarian cache warm-ups/refreshes, consolidation, invalidation, reindexing, verification, evaluation, and retrieval retries.
- **Runtime**: wall-clock task completion, p50/p95 retrieval latency if enough samples, CLI invocation overhead, and number of external tool calls.
- **Correctness**: tests passed, acceptance criteria met, errors introduced, hallucinated or outdated claims, unnecessary edits, and human-review quality.
- **Behavioral signs**: repeated investigation, searches after receiving context, unused retrievals, and duplicated or conflicting claims.
- **Local resources**: SQLite size, index size, Rust process start time, peak resident RAM, disk I/O (secondary metrics; not substitutes for outcomes).

A useful accounting model is:

`total_cost_RECON(N) = build_cost + N * retrieval_and_task_cost + maintenance_cost(N)`

Compare with `total_cost_baseline(N) = N * baseline_task_cost` for repeated uses. A lower per-task retrieval cost does not imply a net saving until amortized build and maintenance costs have been covered.

For approximately fixed per-task savings, the simple break-even request count is:

`N_break_even = ceil((build_cost + fixed_maintenance_cost) / (baseline_task_cost - RECON_task_cost - variable_maintenance_cost_per_task))`

If the denominator is zero or negative, there is no break-even under those assumptions. Count wall time and quality separately; don't collapse everything into money or tokens. Reuse across future tasks may justify higher one-time build cost, but **show the amortization horizon** rather than assuming endless reuse.

## 7. Measurement discipline and confounders

- Keep total permitted information comparable. Orchestrator handoff should include real relevant context but should not receive hidden extra facts unavailable to other arms except as defined by the arm.
- Distinguish **cold start**, **warm RECON**, **cold Librarian cache**, **warm Librarian cache**. Use explicit cache keys, revision map versions, and realistic hit/miss rates; report invalidations.
- Reset repo, worktree state, database snapshot, and agent context per run as appropriate. Ensure each condition starts with an equivalent codebase and task spec.
- Tag every model call with its role (`orchestrator`, `implementer`, `librarian`, `scout`, `consolidator`, `evaluator`), and tie to a run ID. Token accounting must include indirect and failed calls.
- Where harnesses do not expose exact token counts, mark estimates prominently rather than mixing estimated and billed figures silently.
- For a multi-agent task, count **all** agents, including merge/review/resolution calls. Measure regression risk and useful output, not just the first successful response.
- Protect benchmark integrity: no task-specific summaries created using test answers, no knowledge contamination between trials, no unreported model/harness changes.

## 8. Proposed CLI surface (illustrative, not locked API)

`recon task resume <id> --budget 1500`  
`recon context search "reprog init dependencies" --project X --budget 1200 --format json`  
`recon observation add --task <id> --entity <id> --source <ref> --input-json <path>`  
`recon evidence verify --claim <id>` (requests Scout; not automatic)  
`recon metrics export --run <id> --format json`

Prefer predictable JSON envelopes, stable IDs, strict token budgets, provenance, and typed errors. CLI may be invoked through a harness terminal tool, so it does not eliminate all tool calls. Avoid exposing arbitrary SQL writes to agents.

## 9. Prototype sequence for experimentation

1. **Baseline**: run A and B on a small defined task set; instrument and save results.
2. **RECON direct**: minimum Rust CLI + SQLite schema and FTS5; seed knowledge from a fixed repo snapshot with build cost tracked; run C.
3. **Librarian**: add small-model routing with a compact directory map; run D under cold and warm cache scenarios.
4. **Integrity/maintenance**: append-only observations, exact deduplication, provenance and basic conflict flags; only then attempt consolidation experiments.
5. **Evaluation**: report medians, individual runs, task success, cache assumptions, all-in costs, and break-even estimates. Decide whether to retain Librarian based on measured benefit.

No GUI, MCP server, always-on multi-agent team, automated training, or heavy knowledge graph engine is necessary to test the key hypotheses.

## 10. Decision log and open questions

**Agreed direction for investigation:** CLI-first Rust/SQLite RECON Core; GPUI someday, initially read-only; Git via Scout; canonical entities + append-only observations; Librarian as cached semantic navigator to candidate knowledge locations; periodic quality review; benchmark against handoff and no-context controls; include the entire memory-creation cost.

**Still open:** exact identity model for code symbols; retrieval ranking; when to involve Librarian; embedding cost/utility; map size and refresh triggers; how conflicts are presented; compression/archive thresholds; ownership of automated consolidation; optimal memory lifetime; secure cross-harness worktree isolation; cache provider and actual pricing; whether direct retrieval wins in simple tasks; whether periodic improvement creates measurable benefit.

**Design principle:** Reuse demonstrably valuable knowledge. Optimize **successful task completion per total token and unit cost**, rather than SQLite size, agent count, or impressive graph visualizations.