use std::time::Duration;

use super::Verdict;

/// Full result of running one test case: the verdict plus everything a
/// reporter might want to display (timing, captured streams, exit code).
#[derive(Debug, Clone)]
pub struct CaseOutcome {
    pub name: String,
    pub verdict: Verdict,
    pub elapsed: Duration,
    pub input: Vec<u8>,
    pub actual: Vec<u8>,
    pub expected: Option<Vec<u8>>,
    pub exit_code: Option<i32>,
}
