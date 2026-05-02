//! Aizu Online Judge (https://onlinejudge.u-aizu.ac.jp) implementation of
//! `ProblemDownloader`.
//!
//! AOJ exposes a separate testcase data host that returns a JSON array of
//! `{serial, in, out}` objects for a given problem id:
//!
//!   `GET https://judgedat.u-aizu.ac.jp/testcases/samples/{problem_id}`
//!
//! Some problems return `[]` from that endpoint despite carrying samples in
//! the body of the statement; for those we fall back to scraping the rendered
//! description HTML at `judgeapi.u-aizu.ac.jp/resources/descriptions/ja/{id}`,
//! which is what upstream Python `oj` does. The `en` endpoint silently returns
//! the `ja` body for problems without an English version, so a single fetch
//! covers both languages.
//!
//! Problem URLs come in several shapes — the modern site uses path-based
//! routing under `/courses/...` and `/challenges/...`, while the legacy host
//! `judge.u-aizu.ac.jp` still serves a `?id=` query-string form. In all cases
//! the trailing token identifies the problem and is what we hand to the API.

use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use dom_query::Document;
use regex::Regex;
use serde::Deserialize;

use crate::application::ports::ProblemDownloader;
use crate::domain::Sample;

const TESTCASES_API_BASE: &str = "https://judgedat.u-aizu.ac.jp";
const DESCRIPTION_API_BASE: &str = "https://judgeapi.u-aizu.ac.jp";
const USER_AGENT: &str = concat!("oj-rs/", env!("CARGO_PKG_VERSION"));
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct AizuOnlineJudgeDownloader {
    agent: ureq::Agent,
}

impl AizuOnlineJudgeDownloader {
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

impl Default for AizuOnlineJudgeDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl ProblemDownloader for AizuOnlineJudgeDownloader {
    fn download(&self, url: &str) -> Result<Vec<Sample>> {
        let problem_id = parse_problem_id(url)?;
        let samples = self.fetch_testcase_samples(&problem_id)?;
        if !samples.is_empty() {
            return Ok(samples);
        }
        self.fetch_description_samples(&problem_id)
    }
}

impl AizuOnlineJudgeDownloader {
    fn fetch_testcase_samples(&self, problem_id: &str) -> Result<Vec<Sample>> {
        let endpoint = format!("{TESTCASES_API_BASE}/testcases/samples/{problem_id}");
        let body = self
            .agent
            .get(&endpoint)
            .call()
            .with_context(|| format!("HTTP request failed: {endpoint}"))?
            .into_string()
            .context("failed to read sample testcases response")?;
        parse_samples(&body)
    }

    fn fetch_description_samples(&self, problem_id: &str) -> Result<Vec<Sample>> {
        let endpoint = format!("{DESCRIPTION_API_BASE}/resources/descriptions/ja/{problem_id}");
        let body = self
            .agent
            .get(&endpoint)
            .call()
            .with_context(|| format!("HTTP request failed: {endpoint}"))?
            .into_string()
            .context("failed to read description response")?;
        let description: DescriptionResponse =
            serde_json::from_str(&body).context("failed to parse AOJ description response")?;
        Ok(extract_description_samples(&description.html))
    }
}

#[derive(Debug, Deserialize)]
struct RawSample {
    #[serde(default)]
    serial: Option<u32>,
    #[serde(rename = "in", default)]
    input: String,
    #[serde(rename = "out", default)]
    output: String,
}

fn parse_samples(body: &str) -> Result<Vec<Sample>> {
    let raw: Vec<RawSample> =
        serde_json::from_str(body).context("failed to parse AOJ sample testcases response")?;
    Ok(raw
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let num = s.serial.unwrap_or((i + 1) as u32);
            Sample {
                name: format!("sample-{num}"),
                input: s.input.into_bytes(),
                output: Some(s.output.into_bytes()),
            }
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct DescriptionResponse {
    #[serde(default)]
    html: String,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SampleKind {
    Input,
    Output,
}

static SAMPLE_HEADING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<kind>Sample Input|Sample Output|入力例|出力例)\s*(?P<num>\d+)$").unwrap()
});

fn extract_description_samples(html: &str) -> Vec<Sample> {
    let doc = Document::from(html);
    let mut inputs: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut outputs: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut pending: Option<(SampleKind, u32)> = None;

    for el in doc.select("h2, pre").iter() {
        if el.is("h2") {
            pending = parse_heading(el.text().trim());
        } else if let Some((kind, num)) = pending.take() {
            let bucket = match kind {
                SampleKind::Input => &mut inputs,
                SampleKind::Output => &mut outputs,
            };
            bucket
                .entry(num)
                .or_insert_with(|| pre_text_to_bytes(&el.text()));
        }
    }

    inputs
        .into_iter()
        .map(|(num, input)| Sample {
            name: format!("sample-{num}"),
            input,
            output: outputs.remove(&num),
        })
        .collect()
}

fn parse_heading(text: &str) -> Option<(SampleKind, u32)> {
    let captures = SAMPLE_HEADING_RE.captures(text)?;
    let kind = match &captures["kind"] {
        "Sample Input" | "入力例" => SampleKind::Input,
        "Sample Output" | "出力例" => SampleKind::Output,
        _ => return None,
    };
    let num = captures["num"].parse().ok()?;
    Some((kind, num))
}

/// AOJ's `<pre>` blocks have a leading newline from being on their own line in
/// the source HTML; browsers swallow it visually, so we do too.
fn pre_text_to_bytes(text: &str) -> Vec<u8> {
    text.strip_prefix('\n').unwrap_or(text).as_bytes().to_vec()
}

fn parse_problem_id(url: &str) -> Result<String> {
    let parsed = url::Url::parse(url).context("invalid problem URL")?;
    let host = parsed
        .host_str()
        .context("URL has no host")?
        .trim_start_matches("www.");

    match host {
        "judge.u-aizu.ac.jp" => parse_legacy_url(&parsed, url),
        "onlinejudge.u-aizu.ac.jp" => parse_modern_url(&parsed, url),
        _ => bail!("not an Aizu Online Judge URL: {url}"),
    }
}

fn parse_legacy_url(parsed: &url::Url, url: &str) -> Result<String> {
    if parsed.path() != "/onlinejudge/description.jsp" {
        bail!("unsupported Aizu Online Judge URL: {url}");
    }
    let id = parsed
        .query_pairs()
        .find(|(k, _)| k == "id")
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| anyhow!("missing `id` query parameter: {url}"))?;
    if id.is_empty() {
        bail!("empty `id` query parameter: {url}");
    }
    Ok(id)
}

fn parse_modern_url(parsed: &url::Url, url: &str) -> Result<String> {
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|s| s.filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    match segments.as_slice() {
        // /problems/<id> — beta UI
        ["problems", id] => Ok((*id).to_string()),
        // /challenges|courses/<group>/<...>/<id>, 4-6 segments total. Examples:
        //   /challenges/sources/JAG/Prelim/2881
        //   /courses/lesson/8/CGL/1/CGL_1_A
        [head, group, .., id]
            if matches!(*head, "challenges" | "courses")
                && matches!(*group, "sources" | "library" | "lesson" | "chapter")
                && segments.len() <= 6 =>
        {
            Ok((*id).to_string())
        }
        _ => bail!("unsupported Aizu Online Judge URL: {url}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_query_url() {
        assert_eq!(
            parse_problem_id("http://judge.u-aizu.ac.jp/onlinejudge/description.jsp?id=2256")
                .unwrap(),
            "2256"
        );
    }

    #[test]
    fn parses_challenges_url() {
        assert_eq!(
            parse_problem_id("https://onlinejudge.u-aizu.ac.jp/challenges/sources/JAG/Prelim/2881")
                .unwrap(),
            "2881"
        );
    }

    #[test]
    fn parses_courses_url() {
        assert_eq!(
            parse_problem_id("https://onlinejudge.u-aizu.ac.jp/courses/lesson/8/CGL/1/CGL_1_A")
                .unwrap(),
            "CGL_1_A"
        );
    }

    #[test]
    fn parses_problems_url() {
        assert_eq!(
            parse_problem_id("https://onlinejudge.u-aizu.ac.jp/problems/0001").unwrap(),
            "0001"
        );
    }

    #[test]
    fn rejects_legacy_url_without_id() {
        let err =
            parse_problem_id("http://judge.u-aizu.ac.jp/onlinejudge/description.jsp").unwrap_err();
        assert!(err.to_string().contains("`id`"));
    }

    #[test]
    fn rejects_legacy_url_with_empty_id() {
        let err = parse_problem_id("http://judge.u-aizu.ac.jp/onlinejudge/description.jsp?id=")
            .unwrap_err();
        assert!(err.to_string().contains("`id`"));
    }

    #[test]
    fn rejects_unrelated_host() {
        let err = parse_problem_id("https://atcoder.jp/contests/abc/tasks/abc_a").unwrap_err();
        assert!(err.to_string().contains("Aizu Online Judge"));
    }

    #[test]
    fn rejects_root_modern_url() {
        let err = parse_problem_id("https://onlinejudge.u-aizu.ac.jp/").unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn rejects_modern_url_with_too_few_segments() {
        let err =
            parse_problem_id("https://onlinejudge.u-aizu.ac.jp/challenges/sources").unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn rejects_modern_url_with_unknown_group() {
        let err = parse_problem_id("https://onlinejudge.u-aizu.ac.jp/challenges/whatever/X/Y/Z")
            .unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn parses_samples_from_api_response() {
        let body = r#"[
            {"serial":1,"in":"1 2\n","out":"3\n","score":0},
            {"serial":2,"in":"10 20\n","out":"30\n","score":0}
        ]"#;
        let samples = parse_samples(body).unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].name, "sample-1");
        assert_eq!(samples[0].input, b"1 2\n");
        assert_eq!(samples[0].output.as_deref(), Some(&b"3\n"[..]));
        assert_eq!(samples[1].name, "sample-2");
        assert_eq!(samples[1].input, b"10 20\n");
        assert_eq!(samples[1].output.as_deref(), Some(&b"30\n"[..]));
    }

    #[test]
    fn falls_back_to_index_when_serial_missing() {
        let body = r#"[{"in":"a","out":"b"}]"#;
        let samples = parse_samples(body).unwrap();
        assert_eq!(samples[0].name, "sample-1");
    }

    #[test]
    fn parses_empty_array() {
        let samples = parse_samples("[]").unwrap();
        assert!(samples.is_empty());
    }

    #[test]
    fn errors_on_non_array_body() {
        let err = parse_samples(r#"{"error":"not found"}"#).unwrap_err();
        assert!(err.to_string().contains("AOJ sample testcases"));
    }

    #[test]
    fn extracts_japanese_description_samples() {
        let html = "<h2>入力例 1</h2>\n<pre>\n2\n4\n</pre>\n\
                    <h2>出力例 1</h2>\n<pre>\n9\n</pre>\n\
                    <h2>入力例 2</h2>\n<pre>\n15\n30\n</pre>\n\
                    <h2>出力例 2</h2>\n<pre>\n48\n</pre>";
        let samples = extract_description_samples(html);
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].name, "sample-1");
        assert_eq!(samples[0].input, b"2\n4\n");
        assert_eq!(samples[0].output.as_deref(), Some(&b"9\n"[..]));
        assert_eq!(samples[1].name, "sample-2");
        assert_eq!(samples[1].input, b"15\n30\n");
        assert_eq!(samples[1].output.as_deref(), Some(&b"48\n"[..]));
    }

    #[test]
    fn extracts_english_description_samples() {
        let html = "<h2>Sample Input 1</h2><pre>1 2\n</pre>\
                    <h2>Sample Output 1</h2><pre>3\n</pre>";
        let samples = extract_description_samples(html);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].input, b"1 2\n");
        assert_eq!(samples[0].output.as_deref(), Some(&b"3\n"[..]));
    }

    #[test]
    fn ignores_non_sample_h2_headings() {
        let html = "<h2>問題文</h2><p>...</p>\
                    <h2>制約</h2><ul><li>...</li></ul>\
                    <h2>入力例 1</h2><pre>\n7\n</pre>\
                    <h2>出力例 1</h2><pre>\n7\n</pre>";
        let samples = extract_description_samples(html);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].input, b"7\n");
    }

    #[test]
    fn description_samples_empty_when_no_headings() {
        let samples = extract_description_samples("<p>just prose, no samples</p>");
        assert!(samples.is_empty());
    }
}
