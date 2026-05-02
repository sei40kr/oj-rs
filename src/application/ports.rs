//! Boundary traits for the application layer. Implementations live in
//! `infrastructure/`. The domain layer never references these; only use cases
//! and adapters do.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;

use crate::domain::{CaseOutcome, DisplayMode, Sample, TestCase};

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

pub trait ProblemDownloader: Send + Sync {
    /// Fetch and parse the problem page at `url`, returning samples in order.
    fn download(&self, url: &str) -> Result<Vec<Sample>>;
}

pub trait SampleWriter: Send + Sync {
    fn write(&self, path: &Path, content: &[u8]) -> Result<()>;
}

/// User-facing presentation of the sample-download capability.
///
/// Lifecycle:
/// 1. `samples_found(count)` — once, after the page is parsed. Skipped when zero.
/// 2. `sample_written(...)` per case (real run) **or** `dry_run_sample(...)`
///    per case (when `--dry-run` is set).
/// 3. `no_samples_found()` is called instead of (1)+(2) when the page yielded zero samples.
pub trait SampleDownloadReporter: Send + Sync {
    fn samples_found(&self, count: usize);
    fn sample_written(&self, sample: &Sample, input_path: &Path, output_path: Option<&Path>);
    fn dry_run_sample(&self, sample: &Sample);
    fn no_samples_found(&self);
}

/// User-facing presentation of the test-run capability.
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
pub trait TestRunReporter: Send + Sync {
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
