//! Runs RECON's local memory commands and emits the versioned JSON protocol.

mod catalog;
mod cli;
mod conflicts;
mod db;
mod dispatch;
mod error;
mod librarian;
mod lookup;
mod metrics;
mod model;
mod observations;
mod output;
mod relations;
mod retrieval;
mod source;
mod tasks;
mod validate;
mod verification;

use clap::{Parser, error::ErrorKind};
use std::{
    io::{self, Write},
    process::ExitCode,
};

/// Parses one command and emits a bounded JSON result, handling broken output pipes gracefully.
fn main() -> ExitCode {
    let parsed = cli::Cli::try_parse();
    let result = match parsed {
        Ok(cli) => dispatch::execute(cli),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            return if error.print().is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        Err(error) => Err(error::Error::Invalid(error.to_string())),
    };
    let (value, status) = match result {
        Ok(data) => (error::envelope(data), ExitCode::SUCCESS),
        Err(error) => (error.envelope(), ExitCode::FAILURE),
    };
    let stdout = io::stdout();
    let mut output = stdout.lock();
    if serde_json::to_writer(&mut output, &value).is_err() || writeln!(output).is_err() {
        return ExitCode::FAILURE;
    }
    status
}
