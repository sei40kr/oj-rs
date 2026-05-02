//! Boundary traits for the application layer. Implementations live in
//! `infrastructure/`. The domain layer never references these; only use cases
//! and adapters do.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;

use crate::domain::{CaseOutcome, DisplayMode, Language, Sample, TestCase};

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

/// Username + password pair for an online judge.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

pub trait Authenticator: Send + Sync {
    /// Whether the persisted session is currently valid for the service.
    fn is_logged_in(&self) -> Result<bool>;
    /// Authenticate with the service and persist the session for later use.
    fn login(&self, credentials: &Credentials) -> Result<()>;
}

/// Source of credentials when the user did not supply them on the command line.
pub trait CredentialPrompt: Send + Sync {
    fn prompt_username(&self) -> Result<String>;
    fn prompt_password(&self) -> Result<String>;
}

/// User-facing presentation of the login capability.
///
/// Lifecycle (driven by the use case):
/// - `already_logged_in()` — the persisted session is already valid; no login attempted.
/// - `not_logged_in()` — `--check` was set and the session is not valid.
/// - `login_succeeded()` — credentials were accepted and the session was persisted.
pub trait LoginReporter: Send + Sync {
    fn already_logged_in(&self);
    fn not_logged_in(&self);
    fn login_succeeded(&self);
}

pub trait SampleWriter: Send + Sync {
    fn write(&self, path: &Path, content: &[u8]) -> Result<()>;
}

/// Submits source code to an online judge. Implementations are expected to
/// have a session that has already authenticated via the `Authenticator` port.
pub trait Submitter: Send + Sync {
    /// Languages exposed by the problem's submit page. Order matches the page.
    fn available_languages(&self, problem_url: &str) -> Result<Vec<Language>>;
    /// POST the submission and return the URL of the resulting entry.
    fn submit(
        &self,
        problem_url: &str,
        language_id: &str,
        source_code: &[u8],
    ) -> Result<SubmissionReceipt>;
}

#[derive(Debug, Clone)]
pub struct SubmissionReceipt {
    pub submission_url: String,
}

/// Yes/no confirmation prompt for submission. Separate from `CredentialPrompt`
/// because the input semantics differ (single-character read vs. line read).
pub trait SubmitConfirmer: Send + Sync {
    fn confirm_submit(&self) -> Result<bool>;
}

/// User-facing presentation of the submit capability.
///
/// Lifecycle (driven by the use case):
/// 1. `source_loaded(byte_count)` — once, after the source file is read.
/// 2. Either `language_chosen(language)` once **or** `language_unresolved(...)`
///    + a `bail!` from the use case when no single language matched.
/// 3. `submission_aborted()` if the user declined the confirmation prompt.
/// 4. `submission_succeeded(receipt)` on a successful POST.
pub trait SubmitReporter: Send + Sync {
    fn source_loaded(&self, byte_count: usize);
    fn language_chosen(&self, language: &Language);
    fn language_unresolved(&self, candidates: &[Language]);
    fn submission_aborted(&self);
    fn submission_succeeded(&self, receipt: &SubmissionReceipt);
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
