//! CS Academy (https://csacademy.com) implementation of `ProblemDownloader`.
//!
//! The site is a SPA — sample inputs/outputs aren't in the rendered HTML, so we
//! follow the same JSON endpoints the official frontend uses:
//!
//! 1. `GET https://csacademy.com/` once to receive a `csrftoken` cookie.
//! 2. `GET https://csacademy.com/contest/{contest}/` (with `x-csrftoken` /
//!    `x-requested-with: XMLHttpRequest`) returns the contest config as JSON.
//!    We look up the task id matching the URL's task slug.
//! 3. `POST https://csacademy.com/contest/get_contest_task/` with the task id as
//!    a `multipart/form-data` field returns task JSON whose
//!    `state.EvalTask[0].exampleTests` array holds `{input, output}` objects.

use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use cookie_store::CookieStore;
use regex::Regex;
use serde_json::Value;

use crate::application::ports::ProblemDownloader;
use crate::domain::Sample;

const BASE_URL: &str = "https://csacademy.com/";
const GET_CONTEST_TASK_URL: &str = "https://csacademy.com/contest/get_contest_task/";
const USER_AGENT: &str = concat!("oj-rs/", env!("CARGO_PKG_VERSION"));
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Multipart boundary. CS Academy's endpoint expects multipart/form-data
/// (matching upstream's `requests`-with-`files=` behavior); the value is
/// arbitrary as long as it doesn't appear in the body.
const BOUNDARY: &str = "----oj-rs-csacademy-boundary";

pub struct CSAcademyDownloader {
    agent: ureq::Agent,
}

impl CSAcademyDownloader {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .user_agent(USER_AGENT)
            .timeout_connect(HTTP_CONNECT_TIMEOUT)
            .timeout_read(HTTP_READ_TIMEOUT)
            .cookie_store(CookieStore::default())
            .build();
        Self { agent }
    }

    fn csrftoken(&self) -> Option<String> {
        self.agent
            .cookie_store()
            .iter_unexpired()
            .find(|c| c.name() == "csrftoken")
            .map(|c| c.value().to_string())
    }
}

impl Default for CSAcademyDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl ProblemDownloader for CSAcademyDownloader {
    fn download(&self, url: &str) -> Result<Vec<Sample>> {
        let target = parse_problem_url(url)?;

        // Prime the session: the homepage sets a `csrftoken` cookie that the
        // JSON endpoints below require to be echoed back as `x-csrftoken`.
        self.agent
            .get(BASE_URL)
            .call()
            .with_context(|| format!("HTTP request failed: {BASE_URL}"))?;
        let csrftoken = self
            .csrftoken()
            .ok_or_else(|| anyhow!("csrftoken cookie not set by csacademy.com"))?;

        let contest_url = format!("https://csacademy.com/contest/{}/", target.contest);
        let contest_body = self
            .agent
            .get(&contest_url)
            .set("x-csrftoken", &csrftoken)
            .set("x-requested-with", "XMLHttpRequest")
            .call()
            .with_context(|| format!("HTTP request failed: {contest_url}"))?
            .into_string()
            .context("reading contest config")?;
        let task_id = find_task_id(&contest_body, &target.task)?;

        let body = build_multipart_body(BOUNDARY, "contestTaskId", &task_id);
        let task_body = self
            .agent
            .post(GET_CONTEST_TASK_URL)
            .set("x-csrftoken", &csrftoken)
            .set("x-requested-with", "XMLHttpRequest")
            .set("Referer", BASE_URL)
            .set(
                "Content-Type",
                &format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .send_bytes(&body)
            .with_context(|| format!("HTTP request failed: {GET_CONTEST_TASK_URL}"))?
            .into_string()
            .context("reading task response")?;
        extract_samples(&task_body)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ProblemTarget {
    contest: String,
    task: String,
}

static PROBLEM_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^/contest/([0-9A-Za-z_-]+)/task/([0-9A-Za-z_-]+)(?:|/statement|/solution|/discussion|/statistics|/submissions)/?$",
    )
    .unwrap()
});

fn parse_problem_url(url: &str) -> Result<ProblemTarget> {
    let parsed = url::Url::parse(url).context("invalid problem URL")?;
    let host = parsed
        .host_str()
        .context("URL has no host")?
        .trim_start_matches("www.");
    if host != "csacademy.com" {
        bail!("not a CS Academy problem URL: {url}");
    }
    let captures = PROBLEM_PATH_RE
        .captures(parsed.path())
        .ok_or_else(|| anyhow!("unsupported CS Academy URL: {url}"))?;
    Ok(ProblemTarget {
        contest: captures[1].to_string(),
        task: captures[2].to_string(),
    })
}

fn find_task_id(body: &str, task_name: &str) -> Result<String> {
    let config: Value = serde_json::from_str(body).context("parsing contest config JSON")?;
    let tasks = config
        .pointer("/state/contesttask")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("state.contesttask not found in contest config"))?;
    for task in tasks {
        if task.get("name").and_then(Value::as_str) == Some(task_name) {
            // Upstream stringifies the id with `str(...)`, so accept both
            // numeric and string forms in case the API representation drifts.
            return match task.get("id") {
                Some(Value::Number(n)) => Ok(n.to_string()),
                Some(Value::String(s)) => Ok(s.clone()),
                _ => bail!("task `{task_name}` has no id"),
            };
        }
    }
    bail!("no such task on this contest: {task_name}")
}

fn extract_samples(body: &str) -> Result<Vec<Sample>> {
    let task: Value = serde_json::from_str(body).context("parsing task JSON")?;
    if task.get("title").and_then(Value::as_str) == Some("Page not found") {
        bail!("CS Academy returned 'Page not found' for the task");
    }
    let example_tests = task
        .pointer("/state/EvalTask/0/exampleTests")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("state.EvalTask[0].exampleTests not found in task response"))?;

    let mut samples = Vec::with_capacity(example_tests.len());
    for (index, example) in example_tests.iter().enumerate() {
        let input = example
            .get("input")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("example {index} missing `input` field"))?;
        let output = example.get("output").and_then(Value::as_str);
        samples.push(Sample {
            name: format!("sample-{}", index + 1),
            input: input.as_bytes().to_vec(),
            output: output.map(|s| s.as_bytes().to_vec()),
        });
    }
    Ok(samples)
}

fn build_multipart_body(boundary: &str, name: &str, value: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(b"--");
    body.extend_from_slice(boundary.as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"");
    body.extend_from_slice(name.as_bytes());
    body.extend_from_slice(b"\"\r\n\r\n");
    body.extend_from_slice(value.as_bytes());
    body.extend_from_slice(b"\r\n--");
    body.extend_from_slice(boundary.as_bytes());
    body.extend_from_slice(b"--\r\n");
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn parses_canonical_problem_url() {
        let target =
            parse_problem_url("https://csacademy.com/contest/round-38/task/path-union/").unwrap();
        assert_eq!(target.contest, "round-38");
        assert_eq!(target.task, "path-union");
    }

    #[test]
    fn parses_archive_problem_url() {
        let target =
            parse_problem_url("https://csacademy.com/contest/archive/task/swap_permutation/")
                .unwrap();
        assert_eq!(target.contest, "archive");
        assert_eq!(target.task, "swap_permutation");
    }

    #[test]
    fn parses_problem_url_with_statement_suffix() {
        let target = parse_problem_url(
            "https://csacademy.com/contest/archive/task/swap_permutation/statement/",
        )
        .unwrap();
        assert_eq!(target.contest, "archive");
        assert_eq!(target.task, "swap_permutation");
    }

    #[test]
    fn parses_problem_url_with_discussion_suffix() {
        let target =
            parse_problem_url("https://csacademy.com/contest/round-38/task/path-union/discussion")
                .unwrap();
        assert_eq!(target.contest, "round-38");
        assert_eq!(target.task, "path-union");
    }

    #[test]
    fn accepts_www_subdomain() {
        let target =
            parse_problem_url("https://www.csacademy.com/contest/round-38/task/foo/").unwrap();
        assert_eq!(target.contest, "round-38");
        assert_eq!(target.task, "foo");
    }

    #[test]
    fn rejects_non_csacademy_host() {
        let err = parse_problem_url("https://atcoder.jp/contests/abc/tasks/abc_a").unwrap_err();
        assert!(err.to_string().contains("CS Academy"));
    }

    #[test]
    fn rejects_unrelated_path() {
        let err = parse_problem_url("https://csacademy.com/about/").unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn finds_task_id_by_slug() {
        let body = indoc! {r#"
            {
              "state": {
                "contesttask": [
                  {"id": 101, "name": "other-task"},
                  {"id": 202, "name": "path-union"}
                ]
              }
            }
        "#};
        assert_eq!(find_task_id(body, "path-union").unwrap(), "202");
    }

    #[test]
    fn finds_task_id_when_id_is_a_string() {
        let body = r#"{"state":{"contesttask":[{"id":"abc-9","name":"foo"}]}}"#;
        assert_eq!(find_task_id(body, "foo").unwrap(), "abc-9");
    }

    #[test]
    fn errors_when_task_slug_missing() {
        let body = r#"{"state":{"contesttask":[{"id":1,"name":"foo"}]}}"#;
        let err = find_task_id(body, "missing").unwrap_err();
        assert!(err.to_string().contains("no such task"));
    }

    #[test]
    fn errors_when_contesttask_missing() {
        let body = r#"{"state":{}}"#;
        let err = find_task_id(body, "anything").unwrap_err();
        assert!(err.to_string().contains("contesttask"));
    }

    #[test]
    fn extracts_samples_from_task_response() {
        let body = indoc! {r#"
            {
              "state": {
                "EvalTask": [
                  {
                    "exampleTests": [
                      {"input": "1 2\n", "output": "3\n"},
                      {"input": "10 20\n", "output": "30\n"}
                    ]
                  }
                ]
              }
            }
        "#};
        let samples = extract_samples(body).unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].name, "sample-1");
        assert_eq!(samples[0].input, b"1 2\n");
        assert_eq!(samples[0].output.as_deref(), Some(&b"3\n"[..]));
        assert_eq!(samples[1].name, "sample-2");
        assert_eq!(samples[1].input, b"10 20\n");
        assert_eq!(samples[1].output.as_deref(), Some(&b"30\n"[..]));
    }

    #[test]
    fn returns_empty_when_task_has_no_examples() {
        let body = r#"{"state":{"EvalTask":[{"exampleTests":[]}]}}"#;
        let samples = extract_samples(body).unwrap();
        assert!(samples.is_empty());
    }

    #[test]
    fn errors_when_page_not_found() {
        let body = r#"{"title":"Page not found"}"#;
        let err = extract_samples(body).unwrap_err();
        assert!(err.to_string().contains("Page not found"));
    }

    #[test]
    fn builds_multipart_body() {
        let body = build_multipart_body("BOUND", "contestTaskId", "42");
        let text = std::str::from_utf8(&body).unwrap();
        assert_eq!(
            text,
            "--BOUND\r\nContent-Disposition: form-data; name=\"contestTaskId\"\r\n\r\n42\r\n--BOUND--\r\n"
        );
    }
}
