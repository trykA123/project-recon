//! Declares RECON's portable command-line surface; commands delegate to scoped services.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Local evidence-backed memory and optional Librarian routing for coding harnesses"
)]
pub struct Cli {
    /// Explicit operational database path; never implicitly selects a global store.
    #[arg(long)]
    pub db: PathBuf,
    /// Project namespace registered by init.
    #[arg(long)]
    pub project: String,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize a new store or register a project in an existing RECON store.
    Init {
        #[arg(long)]
        repo: PathBuf,
    },
    /// Open, resume, checkpoint, or archive a persistent task.
    Task {
        #[command(subcommand)]
        command: Task,
    },
    /// Append findings or explicitly approve knowledge for general reuse.
    Observation {
        #[command(subcommand)]
        command: Observation,
    },
    /// Retrieve evidence through deterministic exact and lexical search.
    Context {
        #[command(subcommand)]
        command: Context,
    },
    /// Connect canonical entities using evidence-supported relationships.
    Entity {
        #[command(subcommand)]
        command: Entity,
    },
    /// Compare stored source hashes with current files.
    Evidence {
        #[command(subcommand)]
        command: Evidence,
    },
    /// Prepare semantic routing maps and validate harness-generated routes.
    Librarian {
        #[command(subcommand)]
        command: Librarian,
    },
    /// Read a bounded audit page without raw prompts or source contents.
    Journal {
        #[arg(long)]
        task: Option<String>,
        #[arg(long, default_value_t = 0)]
        after: i64,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Record and export measured or explicitly estimated run costs.
    Metrics {
        #[command(subcommand)]
        command: Metrics,
    },
}

#[derive(Debug, Subcommand)]
pub enum Task {
    Open {
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        objective: String,
    },
    Resume {
        id: String,
        #[arg(long, default_value_t = 1200)]
        budget: usize,
    },
    Checkpoint {
        id: String,
        #[arg(long)]
        input: PathBuf,
    },
    Archive {
        id: String,
        #[arg(long)]
        expected_version: u64,
        #[arg(long)]
        actor: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum Observation {
    Add {
        #[arg(long)]
        input: PathBuf,
    },
    Promote {
        id: String,
        #[arg(long)]
        expected_version: u64,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        reason: String,
    },
}

#[derive(Debug, Args)]
pub struct SearchOptions {
    #[arg(long)]
    pub task: Option<String>,
    #[arg(long, default_value_t = 1200)]
    pub budget: usize,
}

#[derive(Debug, Subcommand)]
pub enum Context {
    Search {
        query: String,
        #[arg(long)]
        entity: Option<String>,
        #[command(flatten)]
        options: SearchOptions,
    },
}

#[derive(Debug, Subcommand)]
pub enum Entity {
    Relate {
        from: String,
        to: String,
        #[arg(long)]
        relation: String,
        #[arg(long)]
        claim: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum Evidence {
    Verify {
        id: String,
        #[arg(long)]
        task: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum Librarian {
    Prepare {
        query: String,
        #[command(flatten)]
        options: SearchOptions,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    Resolve {
        #[arg(long)]
        input: PathBuf,
    },
    Prune,
}

#[derive(Debug, Subcommand)]
pub enum Metrics {
    Record {
        #[arg(long)]
        input: PathBuf,
    },
    Export {
        #[arg(long)]
        run: String,
        #[arg(long, default_value_t = 0)]
        after: i64,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
}
