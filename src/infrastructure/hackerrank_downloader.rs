//! HackerRank (https://www.hackerrank.com) implementation of `ProblemDownloader`.
//!
//! Upstream Python `oj` downloads HackerRank cases by GETting
//! `…/download_testcases`, which returns a ZIP of every input/output pair —
//! the same endpoint covers samples and system tests, and most challenges
//! gate it behind a logged-in session. We deliberately diverge: instead of a
//! ZIP and a session cookie, we hit the public REST endpoint
//! `https://www.hackerrank.com/rest/contests/{contest}/challenges/{slug}`,
//! pull `model.body_html`, and scrape the `challenge_sample_input` /
//! `challenge_sample_output` blocks the page would render.
//!
//! Trade-off: we only see the samples that appear in the problem statement,
//! which mirrors `oj download` without `--system`. System tests (which the
//! ZIP endpoint exposes) are out of scope until we add `--system` support.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use dom_query::Document;

use crate::application::ports::ProblemDownloader;
use crate::domain::Sample;

const REST_BASE: &str = "https://www.hackerrank.com";
const USER_AGENT: &str = concat!("oj-rs/", env!("CARGO_PKG_VERSION"));
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct HackerRankDownloader {
    agent: ureq::Agent,
}

impl HackerRankDownloader {
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

impl Default for HackerRankDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl ProblemDownloader for HackerRankDownloader {
    fn download(&self, url: &str) -> Result<Vec<Sample>> {
        let problem = parse_problem_url(url)?;
        let json_url = format!(
            "{REST_BASE}/rest/contests/{}/challenges/{}",
            problem.contest_slug, problem.challenge_slug
        );
        let body = self
            .agent
            .get(&json_url)
            .call()
            .with_context(|| format!("HTTP request failed: {json_url}"))?
            .into_string()
            .context("failed to read challenge response")?;
        let body_html = extract_body_html(&body)?;
        Ok(extract_samples(&body_html))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ProblemRef {
    contest_slug: String,
    challenge_slug: String,
}

fn parse_problem_url(url: &str) -> Result<ProblemRef> {
    let parsed = url::Url::parse(url).context("invalid problem URL")?;
    let host = parsed
        .host_str()
        .context("URL has no host")?
        .trim_start_matches("www.");
    if host != "hackerrank.com" {
        bail!("not a HackerRank problem URL: {url}");
    }
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|s| s.filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    // The trailing `/problem` segment is optional in upstream's matcher. Don't
    // strip it generically — a slug could literally be "problem", so spell out
    // both shapes per pattern.
    match segments.as_slice() {
        ["contests", contest, "challenges", challenge]
        | ["contests", contest, "challenges", challenge, "problem"]
            if !contest.is_empty() && !challenge.is_empty() =>
        {
            Ok(ProblemRef {
                contest_slug: (*contest).to_string(),
                challenge_slug: (*challenge).to_string(),
            })
        }
        // Master challenges (no enclosing contest) live at /challenges/{slug}.
        // Upstream represents them as contest_slug="master".
        ["challenges", challenge] | ["challenges", challenge, "problem"]
            if !challenge.is_empty() =>
        {
            Ok(ProblemRef {
                contest_slug: "master".to_string(),
                challenge_slug: (*challenge).to_string(),
            })
        }
        _ => bail!("unsupported HackerRank URL: {url}"),
    }
}

fn extract_body_html(body: &str) -> Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(body).context("parsing challenge JSON response")?;
    let body_html = value
        .get("model")
        .and_then(|m| m.get("body_html"))
        .and_then(|s| s.as_str())
        .ok_or_else(|| anyhow!("model.body_html not found in challenge response"))?;
    Ok(body_html.to_string())
}

fn extract_samples(html: &str) -> Vec<Sample> {
    let doc = Document::from(html);
    let inputs: Vec<Vec<u8>> = doc
        .select(".challenge_sample_input_body pre")
        .iter()
        .map(|el| el.text().to_string().into_bytes())
        .collect();
    let outputs: Vec<Vec<u8>> = doc
        .select(".challenge_sample_output_body pre")
        .iter()
        .map(|el| el.text().to_string().into_bytes())
        .collect();
    inputs
        .into_iter()
        .enumerate()
        .map(|(i, input)| Sample {
            // HackerRank labels samples zero-indexed ("Sample Input 0"); keep
            // that to match what users see on the problem page.
            name: format!("sample-{i}"),
            input,
            output: outputs.get(i).cloned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn parses_master_challenge_url() {
        let problem =
            parse_problem_url("https://www.hackerrank.com/challenges/solve-me-first").unwrap();
        assert_eq!(
            problem,
            ProblemRef {
                contest_slug: "master".to_string(),
                challenge_slug: "solve-me-first".to_string(),
            }
        );
    }

    #[test]
    fn parses_master_challenge_url_with_problem_suffix() {
        let problem =
            parse_problem_url("https://www.hackerrank.com/challenges/solve-me-first/problem")
                .unwrap();
        assert_eq!(problem.contest_slug, "master");
        assert_eq!(problem.challenge_slug, "solve-me-first");
    }

    #[test]
    fn parses_contest_challenge_url() {
        let problem = parse_problem_url(
            "https://www.hackerrank.com/contests/hourrank-1/challenges/beautiful-array",
        )
        .unwrap();
        assert_eq!(problem.contest_slug, "hourrank-1");
        assert_eq!(problem.challenge_slug, "beautiful-array");
    }

    #[test]
    fn parses_contest_challenge_url_with_problem_suffix() {
        let problem = parse_problem_url(
            "https://www.hackerrank.com/contests/hourrank-1/challenges/beautiful-array/problem",
        )
        .unwrap();
        assert_eq!(problem.contest_slug, "hourrank-1");
        assert_eq!(problem.challenge_slug, "beautiful-array");
    }

    #[test]
    fn slug_literally_named_problem_is_kept() {
        // `/challenges/problem` must resolve to slug="problem", not be mis-stripped
        // into `/challenges/` and rejected.
        let problem = parse_problem_url("https://www.hackerrank.com/challenges/problem").unwrap();
        assert_eq!(problem.challenge_slug, "problem");
        let with_suffix =
            parse_problem_url("https://www.hackerrank.com/challenges/problem/problem").unwrap();
        assert_eq!(with_suffix.challenge_slug, "problem");
    }

    #[test]
    fn accepts_bare_host() {
        let problem =
            parse_problem_url("https://hackerrank.com/challenges/solve-me-first").unwrap();
        assert_eq!(problem.contest_slug, "master");
    }

    #[test]
    fn rejects_non_hackerrank_host() {
        let err = parse_problem_url("https://atcoder.jp/contests/abc/tasks/abc_a").unwrap_err();
        assert!(err.to_string().contains("HackerRank"));
    }

    #[test]
    fn rejects_unrelated_path() {
        let err = parse_problem_url("https://www.hackerrank.com/").unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn extracts_body_html_from_json() {
        let body = r#"{"status":true,"model":{"body_html":"<p>hello</p>","name":"x"}}"#;
        assert_eq!(extract_body_html(body).unwrap(), "<p>hello</p>");
    }

    #[test]
    fn errors_when_body_html_missing() {
        let body = r#"{"status":true,"model":{"name":"x"}}"#;
        let err = extract_body_html(body).unwrap_err();
        assert!(err.to_string().contains("body_html"));
    }

    #[test]
    fn extracts_single_sample() {
        let html = indoc! {r#"
            <div class='challenge_sample_input'>
              <div class='msB challenge_sample_input_title'>
                <p><strong>Sample Input</strong></p>
              </div>
              <div class='msB challenge_sample_input_body'>
                <div class='hackdown-content'>
                  <pre><code>a = 2
            b = 3
            </code></pre>
                </div>
              </div>
            </div>
            <div class='challenge_sample_output'>
              <div class='msB challenge_sample_output_title'>
                <p><strong>Sample Output</strong></p>
              </div>
              <div class='msB challenge_sample_output_body'>
                <div class='hackdown-content'>
                  <pre><code>5
            </code></pre>
                </div>
              </div>
            </div>
        "#};
        let samples = extract_samples(html);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].name, "sample-0");
        assert_eq!(samples[0].input, b"a = 2\nb = 3\n");
        assert_eq!(samples[0].output.as_deref(), Some(&b"5\n"[..]));
    }

    #[test]
    fn extracts_multiple_samples_in_order() {
        let html = indoc! {r#"
            <div class='challenge_sample_input'>
              <div class='challenge_sample_input_body'><pre><code>1
            </code></pre></div>
            </div>
            <div class='challenge_sample_output'>
              <div class='challenge_sample_output_body'><pre><code>one
            </code></pre></div>
            </div>
            <div class='challenge_sample_input'>
              <div class='challenge_sample_input_body'><pre><code>2
            </code></pre></div>
            </div>
            <div class='challenge_sample_output'>
              <div class='challenge_sample_output_body'><pre><code>two
            </code></pre></div>
            </div>
        "#};
        let samples = extract_samples(html);
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].name, "sample-0");
        assert_eq!(samples[0].input, b"1\n");
        assert_eq!(samples[0].output.as_deref(), Some(&b"one\n"[..]));
        assert_eq!(samples[1].name, "sample-1");
        assert_eq!(samples[1].input, b"2\n");
        assert_eq!(samples[1].output.as_deref(), Some(&b"two\n"[..]));
    }

    #[test]
    fn returns_empty_when_no_samples() {
        let samples = extract_samples("<div><p>no samples here</p></div>");
        assert!(samples.is_empty());
    }

    #[test]
    fn input_without_matching_output_is_kept_with_none() {
        let html = indoc! {r#"
            <div class='challenge_sample_input'>
              <div class='challenge_sample_input_body'><pre><code>x
            </code></pre></div>
            </div>
        "#};
        let samples = extract_samples(html);
        assert_eq!(samples.len(), 1);
        assert!(samples[0].output.is_none());
    }
}
