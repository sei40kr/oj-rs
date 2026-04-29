/// Judgment for a single test case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Accepted,
    WrongAnswer,
    RuntimeError,
    TimeLimitExceeded,
}

impl Verdict {
    /// Short label for user-facing output. Matches upstream `oj` exactly.
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Accepted => "AC",
            Verdict::WrongAnswer => "WA",
            Verdict::RuntimeError => "RE",
            Verdict::TimeLimitExceeded => "TLE",
        }
    }
}
