//! Creates isolated repositories and invokes the real CLI for medium acceptance tests.

use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

pub struct Harness {
    pub directory: TempDir,
    pub root: PathBuf,
    pub database: PathBuf,
    sequence: AtomicUsize,
}

impl Harness {
    /// Creates a temporary repository and initializes its real SQLite store.
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("repository");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("source.txt"),
            "runner initializes worker\nsecond line\n",
        )
        .unwrap();
        let database = directory.path().join("knowledge.sqlite3");
        let harness = Self {
            directory,
            root,
            database,
            sequence: AtomicUsize::new(0),
        };
        harness.ok(&["init", "--repo", harness.root.to_str().unwrap()]);
        harness
    }

    /// Executes one command in the default project and captures both output streams.
    pub fn run(&self, args: &[&str]) -> Output {
        self.project_run("demo", args)
    }

    /// Executes one command in a named project against this test's database.
    pub fn project_run(&self, project: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_recon"))
            .args([
                "--db",
                self.database.to_str().unwrap(),
                "--project",
                project,
            ])
            .args(args)
            .output()
            .unwrap()
    }

    /// Returns successful command data or fails with captured diagnostics.
    pub fn ok(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{:?}: {} {}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(envelope["schemaVersion"], 1);
        assert_eq!(envelope["ok"], true);
        envelope["data"].clone()
    }

    /// Requires a typed command error and returns the complete error envelope.
    pub fn error(&self, args: &[&str], code: &str) -> Value {
        let output = self.run(args);
        assert!(
            !output.status.success(),
            "command unexpectedly succeeded: {args:?}"
        );
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(envelope["error"]["code"], code, "{envelope}");
        envelope
    }

    /// Writes a unique JSON input file owned by this test and returns its path.
    pub fn input(&self, value: &Value) -> PathBuf {
        let id = self.sequence.fetch_add(1, Ordering::Relaxed);
        let path = self.directory.path().join(format!("input-{id}.json"));
        std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }

    /// Opens a stable task with a fixed objective for later CLI processes.
    pub fn task(&self, id: &str) -> Value {
        self.ok(&[
            "task",
            "open",
            id,
            "--title",
            "Investigate runner",
            "--objective",
            "Keep job outputs isolated",
        ])
    }

    /// Returns a finding input with real source evidence and optional task scope.
    pub fn finding(key: &str, entity: &str, text: &str, task: Option<&str>) -> Value {
        json!({"taskId":task,"actor":"codex","idempotencyKey":key,
            "entity":{"kind":"module","key":entity,"name":entity,"aliases":[]},
            "kind":"finding","text":text,"evidence":[{"path":"source.txt","lineStart":1,"lineEnd":2}]})
    }

    /// Publishes a structured finding through the CLI and returns its receipt.
    pub fn add(&self, value: &Value) -> Value {
        let path = self.input(value);
        self.ok(&["observation", "add", "--input", path.to_str().unwrap()])
    }
}
