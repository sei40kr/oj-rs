/// One sample test case fetched from an online judge problem page. The bytes
/// are held in memory; persisting them is the writer port's job.
#[derive(Debug, Clone)]
pub struct Sample {
    /// Display name (e.g., `"sample-1"`). Becomes the `%b`/`%n`/`%s` substitution
    /// when the use case resolves filenames.
    pub name: String,
    pub input: Vec<u8>,
    pub output: Option<Vec<u8>>,
}
