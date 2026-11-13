//! Validates CLI boundaries and delegates each operation to one RECON service.

use crate::{
    cli::{self, Cli, Command},
    db::Db,
    error::Result,
    librarian, metrics,
    observations::{self, Promotion},
    relations::{self, Relation},
    retrieval, tasks, validate, verification,
};
use serde_json::{Value, json};

/// Opens the explicit database/project and returns one command's structured result.
pub fn execute(cli: Cli) -> Result<Value> {
    if let Command::Init { repo } = &cli.command {
        let db = Db::init(&cli.db, &cli.project, repo)?;
        return Ok(json!({"project":cli.project,"root":db.root()?,"schemaVersion":1}));
    }
    let mut db = Db::open(&cli.db, &cli.project)?;
    db.root()?;
    match cli.command {
        Command::Init { .. } => unreachable!("init returned before opening an existing store"),
        Command::Task { command } => task(&mut db, command),
        Command::Observation { command } => observation(&mut db, command),
        Command::Context {
            command:
                cli::Context::Search {
                    query,
                    entity,
                    options,
                },
        } => retrieval::search(
            &mut db,
            &query,
            (options.task.as_deref(), entity.as_deref(), options.budget),
        ),
        Command::Entity {
            command:
                cli::Entity::Relate {
                    from,
                    to,
                    relation,
                    claim,
                },
        } => relations::add(
            &mut db,
            Relation {
                from: &from,
                to: &to,
                kind: &relation,
                claim: &claim,
            },
        ),
        Command::Evidence {
            command: cli::Evidence::Verify { id, task },
        } => verification::verify(&mut db, &id, task.as_deref()),
        Command::Librarian { command } => librarian(&mut db, command),
        Command::Journal { task, after, limit } => {
            metrics::journal(&db, task.as_deref(), after, limit)
        }
        Command::Metrics { command } => metric(&mut db, command),
    }
}

/// Dispatches persistent task operations after typed JSON parsing where needed.
fn task(db: &mut Db, command: cli::Task) -> Result<Value> {
    match command {
        cli::Task::Open {
            id,
            title,
            objective,
        } => tasks::open(db, &id, &title, &objective),
        cli::Task::Resume { id, budget } => tasks::resume(db, &id, budget),
        cli::Task::Checkpoint { id, input } => {
            tasks::checkpoint(db, &id, &validate::input(&input)?)
        }
        cli::Task::Archive {
            id,
            expected_version,
            actor,
        } => tasks::archive(db, &id, expected_version, &actor),
    }
}

/// Dispatches validated append-only contributions and explicit promotion requests.
fn observation(db: &mut Db, command: cli::Observation) -> Result<Value> {
    match command {
        cli::Observation::Add { input } => observations::add(db, &validate::input(&input)?),
        cli::Observation::Promote {
            id,
            expected_version,
            actor,
            reason,
        } => observations::promote(
            db,
            Promotion {
                claim: &id,
                version: expected_version,
                actor: &actor,
                reason: &reason,
            },
        ),
    }
}

/// Dispatches directory preparation, route resolution, or expired-request cleanup.
fn librarian(db: &mut Db, command: cli::Librarian) -> Result<Value> {
    match command {
        cli::Librarian::Prepare {
            query,
            options,
            limit,
        } => librarian::prepare(db, &query, (options.task.as_deref(), options.budget, limit)),
        cli::Librarian::Resolve { input } => librarian::resolve(db, &validate::input(&input)?),
        cli::Librarian::Prune => librarian::prune(db),
    }
}

/// Dispatches explicit measurement writes and cursor-based accounting export.
fn metric(db: &mut Db, command: cli::Metrics) -> Result<Value> {
    match command {
        cli::Metrics::Record { input } => metrics::record(db, &validate::input(&input)?),
        cli::Metrics::Export { run, after, limit } => metrics::export(db, &run, after, limit),
    }
}
