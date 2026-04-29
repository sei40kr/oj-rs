//! Boundary traits for the application layer. Implementations live in
//! `infrastructure/`. The domain layer never references these; only use cases
//! and adapters do.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;

use crate::domain::{CaseOutcome, DisplayMode, TestCase};

/// Inputs for the test-case discovery port.
#[derive(Debug, Clone)]
pub struct DiscoveryQuery {
    pub directory: PathBuf,
    pub format: String,
    pub explicit_paths: Vec<PathBuf>,
    pub ignore_backup: bool,
}

/// Inputs for the solution executor port.
#[derive(Debug, Clone)]
pub struct ExecutionRequest<'a> {
    pub command: &'a str,
    pub stdin: &'a [u8],
    pub time_limit: Option<Duration>,
}

/// Raw output of running the user's solution against one input. Verdict is
/// AC if the process exited 0, RE if nonzero, TLE if killed by timeout —
/// **before** any output comparison. Comparison is the use case's job.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub stdout: Vec<u8>,
    pub elapsed: Duration,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

pub trait TestCaseRepository: Send + Sync {
    fn discover(&self, query: &DiscoveryQuery) -> Result<Vec<TestCase>>;
}

pub trait SolutionExecutor: Send + Sync {
    fn execute(&self, request: ExecutionRequest<'_>) -> Result<ExecutionResult>;
}

pub trait JudgeRunner: Send + Sync {
    /// Run a special judge: `command INPUT_PATH ACTUAL_PATH EXPECTED_PATH`.
    /// Exit 0 = AC, anything else = WA.
    fn judge(
        &self,
        command: &str,
        input_path: &Path,
        actual: &[u8],
        expected_path: Option<&Path>,
    ) -> Result<bool>;
}

/// User-facing presentation of test results.
///
/// Lifecycle (driven by the use case):
/// 1. `run_started(total)` — once, before any case runs.
/// 2. `case_finished(outcome, ...)` — called from worker threads as each case
///    completes. Prints the verdict line and any failure detail. With `-j`,
///    cases arrive in completion order, matching upstream `oj test`.
/// 3. `run_finished(outcomes)` — once, with the full ordered list. Prints
///    the summary footer.
///
/// Or, if no cases are discovered: `no_cases_found(query)` — and none of the
/// other methods will be called.
pub trait Reporter: Send + Sync {
    fn run_started(&self, total: usize);
    fn case_finished(
        &self,
        outcome: &CaseOutcome,
        display_mode: DisplayMode,
        silent: bool,
        print_input: bool,
    );
    fn run_finished(&self, outcomes: &[CaseOutcome]);
    fn no_cases_found(&self, query: &DiscoveryQuery);
}
