//! Library Checker (https://judge.yosupo.jp) implementation of `ProblemDownloader`.
//!
//! The site is a SPA and does not server-render samples, so we cannot scrape the
//! HTML page the way the AtCoder downloader does. Instead we use the same data
//! sources the official frontend hits:
//!
//! 1. `GET https://v3.api.judge.yosupo.jp/problems/{name}` — REST endpoint that
//!    returns the problem's `testcases_version` (a content hash).
//! 2. `https://storage.googleapis.com/v2-prod-library-checker-data-public/v4/examples/{name}/{hash}/{in|out}/example_NN.{in|out}`
//!    — public GCS objects holding the example inputs/outputs.
//!
//! Upstream Python `oj` clones `yosupo06/library-checker-problems` and runs
//! `generate.py` to materialize examples on disk; we deliberately diverge to
//! avoid the git+toolchain dependency. The trade-off is that we only support
//! sample cases (the public bucket does not expose `random_*` / `hand_*`),
//! which mirrors `oj download` without `--system`.

use std::io::Read;
use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;

use crate::application::ports::ProblemDownloader;
use crate::domain::Sample;

const REST_API_BASE: &str = "https://v3.api.judge.yosupo.jp";
const PUBLIC_BUCKET_BASE: &str =
    "https://storage.googleapis.com/v2-prod-library-checker-data-public";
const USER_AGENT: &str = concat!("oj-rs/", env!("CARGO_PKG_VERSION"));
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Hard cap on the example-index probe loop. Today the largest problem has
/// well under 20 examples; 100 leaves head-room without letting a misbehaving
/// server pull us into an unbounded fetch.
const MAX_EXAMPLES: u32 = 100;

pub struct LibraryCheckerDownloader {
    agent: ureq::Agent,
}

impl LibraryCheckerDownloader {
    pub fn new() -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .user_agent(USER_AGENT)
                .timeout_connect(HTTP_CONNECT_TIMEOUT)
                .timeout_read(HTTP_READ_TIMEOUT)
                .build(),
        }
    }
}

impl Default for LibraryCheckerDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl ProblemDownloader for LibraryCheckerDownloader {
    fn download(&self, url: &str) -> Result<Vec<Sample>> {
        let problem_id = parse_problem_id(url)?;
        let info = self.fetch_problem_info(&problem_id)?;
        self.fetch_examples(&problem_id, &info.testcases_version)
    }
}

impl LibraryCheckerDownloader {
    fn fetch_problem_info(&self, problem_id: &str) -> Result<ProblemInfo> {
        let url = format!("{REST_API_BASE}/problems/{problem_id}");
        let body = self
            .agent
            .get(&url)
            .call()
            .with_context(|| format!("HTTP request failed: {url}"))?
            .into_string()
            .context("failed to read problem info response")?;
        parse_problem_info(&body)
    }

    fn fetch_examples(&self, problem_id: &str, testcases_version: &str) -> Result<Vec<Sample>> {
        let mut samples = Vec::new();
        for index in 0..MAX_EXAMPLES {
            let name = format!("example_{index:02}");
            let input = match self.fetch_example_file(problem_id, testcases_version, &name, "in")? {
                Some(bytes) => bytes,
                None => break,
            };
            let output = self.fetch_example_file(problem_id, testcases_version, &name, "out")?;
            samples.push(Sample {
                name,
                input,
                output,
            });
        }
        Ok(samples)
    }

    fn fetch_example_file(
        &self,
        problem_id: &str,
        testcases_version: &str,
        name: &str,
        kind: &str,
    ) -> Result<Option<Vec<u8>>> {
        let url = format!(
            "{PUBLIC_BUCKET_BASE}/v4/examples/{problem_id}/{testcases_version}/{kind}/{name}.{kind}"
        );
        match self.agent.get(&url).call() {
            Ok(response) => {
                let mut buf = Vec::new();
                response
                    .into_reader()
                    .read_to_end(&mut buf)
                    .with_context(|| format!("reading body from {url}"))?;
                Ok(Some(buf))
            }
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(ureq::Error::Status(code, _)) => {
                bail!("unexpected response {code} from {url}")
            }
            Err(e) => bail!("HTTP request failed: {url}: {e}"),
        }
    }
}

fn parse_problem_id(url: &str) -> Result<String> {
    let parsed = url::Url::parse(url).context("invalid problem URL")?;
    let host = parsed
        .host_str()
        .context("URL has no host")?
        .trim_start_matches("www.");
    if host != "judge.yosupo.jp" && host != "old.yosupo.jp" {
        bail!("not a Library Checker problem URL: {url}");
    }
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|s| s.filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    match segments.as_slice() {
        ["problem", id, ..] if !id.is_empty() => Ok((*id).to_string()),
        _ => bail!("unsupported Library Checker URL: {url}"),
    }
}

#[derive(Debug)]
struct ProblemInfo {
    testcases_version: String,
}

static TESTCASES_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    // The REST response is small JSON we don't otherwise inspect, so a single
    // anchored regex avoids pulling in serde_json just for one field.
    Regex::new(r#""testcases_version"\s*:\s*"([0-9a-fA-F]+)""#).unwrap()
});

fn parse_problem_info(body: &str) -> Result<ProblemInfo> {
    let captures = TESTCASES_VERSION_RE
        .captures(body)
        .ok_or_else(|| anyhow!("testcases_version not found in problem info response"))?;
    Ok(ProblemInfo {
        testcases_version: captures[1].to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_problem_id_from_modern_url() {
        assert_eq!(
            parse_problem_id("https://judge.yosupo.jp/problem/aplusb").unwrap(),
            "aplusb"
        );
    }

    #[test]
    fn parses_problem_id_with_trailing_slash() {
        assert_eq!(
            parse_problem_id("https://judge.yosupo.jp/problem/unionfind/").unwrap(),
            "unionfind"
        );
    }

    #[test]
    fn parses_problem_id_from_old_host() {
        assert_eq!(
            parse_problem_id("https://old.yosupo.jp/problem/aplusb").unwrap(),
            "aplusb"
        );
    }

    #[test]
    fn rejects_non_library_checker_host() {
        let err = parse_problem_id("https://atcoder.jp/contests/abc/tasks/abc_a").unwrap_err();
        assert!(err.to_string().contains("Library Checker"));
    }

    #[test]
    fn rejects_unrelated_path() {
        let err = parse_problem_id("https://judge.yosupo.jp/").unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn extracts_testcases_version() {
        let body =
            r#"{"overall_version":"abc","testcases_version":"deadbeef1234","title":"A + B"}"#;
        let info = parse_problem_info(body).unwrap();
        assert_eq!(info.testcases_version, "deadbeef1234");
    }

    #[test]
    fn errors_when_testcases_version_missing() {
        let body = r#"{"title":"A + B"}"#;
        let err = parse_problem_info(body).unwrap_err();
        assert!(err.to_string().contains("testcases_version"));
    }
}
