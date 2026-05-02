//! Use case: run a set of test cases against the user's solution and report
//! verdicts. Holds no I/O — depends only on ports.

use std::fs;
use std::time::Duration;

use anyhow::{Context, Result};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

use crate::domain::{CaseOutcome, CompareMode, DisplayMode, TestCase, Verdict, compare};

use super::ports::{
    DiscoveryQuery, ExecutionRequest, JudgeRunner, SolutionExecutor, TestCaseRepository,
    TestRunReporter,
};

#[derive(Debug, Clone)]
pub struct RunTestsInput {
    pub command: String,
    pub discovery: DiscoveryQuery,
    pub compare_mode: CompareMode,
    pub display_mode: DisplayMode,
    pub error_tolerance: Option<f64>,
    pub time_limit: Option<Duration>,
    pub workers: usize,
    pub silent: bool,
    pub print_input: bool,
    pub judge_command: Option<String>,
}

#[derive(Debug)]
pub struct RunTestsOutput {
    pub outcomes: Vec<CaseOutcome>,
}

impl RunTestsOutput {
    pub fn all_accepted(&self) -> bool {
        !self.outcomes.is_empty() && self.outcomes.iter().all(|o| o.verdict == Verdict::Accepted)
    }
}

pub struct RunTests<'a> {
    pub repository: &'a dyn TestCaseRepository,
    pub executor: &'a (dyn SolutionExecutor + 'a),
    pub judge: &'a dyn JudgeRunner,
    pub reporter: &'a dyn TestRunReporter,
}

impl<'a> RunTests<'a> {
    pub fn execute(&self, input: RunTestsInput) -> Result<RunTestsOutput> {
        let cases = self.repository.discover(&input.discovery)?;

        if cases.is_empty() {
            self.reporter.no_cases_found(&input.discovery);
            return Ok(RunTestsOutput { outcomes: vec![] });
        }
        self.reporter.run_started(cases.len());

        let outcomes = if input.workers <= 1 {
            cases
                .iter()
                .map(|case| self.run_and_report(case, &input))
                .collect::<Result<Vec<_>>>()?
        } else {
            let pool = ThreadPoolBuilder::new()
                .num_threads(input.workers)
                .build()
                .context("failed to build worker pool")?;
            pool.install(|| {
                cases
                    .par_iter()
                    .map(|case| self.run_and_report(case, &input))
                    .collect::<Result<Vec<_>>>()
            })?
        };

        self.reporter.run_finished(&outcomes);
        Ok(RunTestsOutput { outcomes })
    }

    fn run_and_report(&self, case: &TestCase, input: &RunTestsInput) -> Result<CaseOutcome> {
        let outcome = self.run_case(case, input)?;
        self.reporter.case_finished(
            &outcome,
            input.display_mode,
            input.silent,
            input.print_input,
        );
        Ok(outcome)
    }

    fn run_case(&self, case: &TestCase, input: &RunTestsInput) -> Result<CaseOutcome> {
        let stdin = fs::read(&case.input)
            .with_context(|| format!("failed to read input: {}", case.input.display()))?;
        let expected = match &case.expected {
            Some(path) => Some(
                fs::read(path)
                    .with_context(|| format!("failed to read expected: {}", path.display()))?,
            ),
            None => None,
        };

        let result = self.executor.execute(ExecutionRequest {
            command: &input.command,
            stdin: &stdin,
            time_limit: input.time_limit,
        })?;

        let verdict = if result.timed_out {
            Verdict::TimeLimitExceeded
        } else if !matches!(result.exit_code, Some(0)) {
            Verdict::RuntimeError
        } else if let Some(judge_command) = &input.judge_command {
            let accepted = self.judge.judge(
                judge_command,
                &case.input,
                &result.stdout,
                case.expected.as_deref(),
            )?;
            if accepted {
                Verdict::Accepted
            } else {
                Verdict::WrongAnswer
            }
        } else if let Some(expected_bytes) = &expected {
            if compare(
                &result.stdout,
                expected_bytes,
                input.compare_mode,
                input.error_tolerance,
            ) {
                Verdict::Accepted
            } else {
                Verdict::WrongAnswer
            }
        } else {
            // No expected output: process exit 0 already implies AC.
            Verdict::Accepted
        };

        Ok(CaseOutcome {
            name: case.name.clone(),
            verdict,
            elapsed: result.elapsed,
            input: stdin,
            actual: result.stdout,
            expected,
            exit_code: result.exit_code,
        })
    }
}
