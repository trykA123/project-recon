<!-- Maps the Rust CLI's boundaries and persistent-state invariants for contributors. -->
# RECON source map

This repository builds one Rust binary. `cli` declares commands; `dispatch` parses external JSON and calls services; `main` emits one versioned envelope. `error`, `model`, `validate`, and `output` define typed failures, bounded input records, deterministic IDs, and serialized response caps.

`db` recognizes the application/schema before changing database settings. `migrations/001_init.sql` creates STRICT tables, scoped foreign keys, FTS maintenance triggers, and indexes. Durable writes use immediate transactions and FULL synchronous mode. Existing unrelated files and unsupported schemas are rejected. This first version creates fresh stores; it does not migrate the Markdown prototype.

`tasks` owns task continuity and optimistic checkpoint versions. `observations` appends attributed contributions, deduplicates canonical claim text, and records explicit promotion of existing contributions. Later contributions stay private. `conflicts` derives disagreement visibility per scope. `source` opens targeted evidence through a confined directory capability with nonblocking Unix file opens; matching hashes establish source bytes, not claim truth. `verification` reports freshness separately.

`catalog` derives labels from visible contributions. `relations` requires visible endpoints and supporting claims. `lookup` selects distinct exact/FTS5 matches and reports lookup truncation. `retrieval` performs one bounded relationship hop and returns scoped provenance. Every read package uses one SQLite snapshot. Budgets enforce a byte cap and label approximate token accounting.

`librarian` prepares versioned directories without claim bodies, persists one-hour Session State requests, and validates replies against the original scope/budget/allowlist in one snapshot. It invokes no model or shell. Semantic routing runs in the caller's harness. Stored directory generations and provider prompt caches are separate concepts.

`metrics` stores explicit external accounting and paginates scoped journal events. Null means unavailable; the CLI does not infer provider costs or cache hits. General/task memory is durable; only expired routing requests can be pruned.

`adapters` contains optional harness instructions. It does not install itself or alter existing global instructions. `tests` drives real CLI subprocesses and temporary SQLite files. Benchmark experiments, other-harness validation, GUI, automatic Git indexing, and model-provider clients remain outside this slice.

Git contains source, migrations, lockfile, tests, and build/use documentation. Compiled binaries, operational knowledge, runtime maps, and SQLite databases remain local. A database backup repository is a separate future decision.
