<!-- Explains source compilation and the portable RECON memory/Librarian workflow. -->
# Build and use RECON

RECON is a local CLI prototype. It requires Rust 1.99.1, Cargo and the platform's C compiler/linker for bundled SQLite. Compile the source on each computer:

```sh
cargo build --release --locked
target/release/recon --help
```

On Windows, use `target\release\recon.exe`. Windows has not been exercised yet. `rust-toolchain.toml` pins Rust when rustup manages the toolchain; the host used for this implementation uses the matching system compiler.

For development, `make check` runs formatting, linting, technical tests, release build and dependency auditing. Install cargo-audit separately through its official Cargo package if needed. Building RECON itself does not require that audit tool.

## Choose local operational state

Create the database's parent directory yourself, outside your checkout if convenient. Every command requires an explicit database and project, so switching harnesses does not silently select a different library. Examples below use placeholder paths; replace them with your own.

```sh
target/release/recon --db /local/recon.sqlite3 --project demo init --repo /path/to/repository
target/release/recon --db /local/recon.sqlite3 --project demo task open investigation --title "Investigate runner" --objective "Understand worker startup"
target/release/recon --db /local/recon.sqlite3 --project demo task resume investigation
```

Project roots are fixed by init. This slice supports a single registered repository root per project; use a separate project/store for a separate worktree. A project database can contain several isolated project namespaces. Task IDs are explicit and survive branch or harness changes.

## Save findings and checkpoints

Create a JSON input following [the CLI contract](cli-contract.md). Observation evidence points to real files relative to the registered root; Core captures their SHA-256 hashes. Optional sourceHash rejects mismatched bytes. An identical idempotency key returns the original receipt even if the source later changes.

```sh
target/release/recon --db /local/recon.sqlite3 --project demo observation add --input /local/finding.json
target/release/recon --db /local/recon.sqlite3 --project demo task checkpoint investigation --input /local/checkpoint.json
target/release/recon --db /local/recon.sqlite3 --project demo context search runner --task investigation --budget 1200
```

Checkpoint JSON has expectedVersion, actor, phase, progress, decisions, constraints, blockers, nextActions, and optional handoff. Read the current version with task resume before updating it. A conflict means another writer changed the task; resume and reconcile before retrying. Repeating a checkpoint with the old version is rejected.

New claims are candidates. Their hashes do not establish semantic truth. Explicit promotion records actor and reason and approves the claim's existing contributions for general retrieval. Later task contributions remain private until approved again. Visible disagreements determine disputed status in each scope; hidden task disagreements do not change the general view. Evidence verification compares current bytes; a Scout still needs to inspect meaning when appropriate.

Inspect the claim in its originating task scope before promotion. New contributions advance its version; a promotion based on an earlier version conflicts instead of publishing unseen evidence.

## Route conceptual questions through Librarian

```sh
target/release/recon --db /local/recon.sqlite3 --project demo librarian prepare "Which modules control worker startup?" --task investigation --budget 1200
```

Give the returned directory, query, requestId, and mapVersion to the Librarian role. It must select real entity IDs from that directory and return only this JSON shape:

```json
{
  "requestId": "copy-from-prepare",
  "mapVersion": "copy-from-prepare",
  "entityIds": [],
  "needsScout": true,
  "uncertainty": ["No matching entity was found in the directory."]
}
```

When matches exist, replace entityIds with the selected directory IDs and describe any remaining uncertainty. The calling agent writes this reply to a local file and resolves it:

```sh
target/release/recon --db /local/recon.sqlite3 --project demo librarian resolve --input /local/route.json
```

Core validates project, request lifetime, map version and allowed IDs, then returns evidence under the original budget. If knowledge changed, prepare a fresh map. Requests expire after one hour; librarian prune removes expired requests. The directory has no claim bodies; routes are not themselves factual answers.

The CLI never starts or bills a model. A harness agent performs semantic routing. Optional [common instructions](../adapters/common/AGENTS.md) and [Claude role](../adapters/claude-code/recon-librarian.md) are source templates; install/merge them only where desired. Their execution in Claude and other harnesses remains untested.

## Inspect history and costs

Use journal with an optional task and cursor. Metrics record accepts explicit model/run accounting; metrics export preserves null for unavailable counts/costs. Record all participants and indexing/maintenance work when conducting later experiments. No savings or provider-cache behavior has been validated.

Responses label utf8BytesDiv4 token estimates and enforce four UTF-8 bytes per budget unit over the complete compact JSON envelope. This is not an exact tokenizer. Claims are omitted whole; omittedCount counts omissions within selected entities. lookupTruncated reports additional lookup/graph matches, and truncated covers both limits. Directory output has a separate 64 KiB ceiling.

## Keep knowledge local

Git tracks source and compilation instructions. `.gitignore` excludes target output, local RECON state, SQLite files and their WAL/SHM sidecars. Do not force-add operational exports or local map/route files. Keep those in the local state directory or outside the checkout.

No database backup repository or automatic synchronization is configured. A future backup must use SQLite's online backup mechanism rather than copying an active database without its WAL state.
