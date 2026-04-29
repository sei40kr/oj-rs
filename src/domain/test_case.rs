use std::path::PathBuf;

/// One sample test case discovered on disk: the input file, optionally a
/// matching expected-output file, and a human-readable name (the part of the
/// filename that matched `%s` in the format).
#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub input: PathBuf,
    pub expected: Option<PathBuf>,
}
