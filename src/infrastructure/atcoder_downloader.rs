//! AtCoder implementation of `ProblemDownloader`. Fetches the task page and
//! scrapes the sample-input/output blocks.
//!
//! AtCoder serves both English and Japanese sample headings on the same page
//! (`Sample Input N` / `入力例 N` and their output counterparts), so we dedupe
//! by sample number and keep whichever language appears first in document order.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use anyhow::{Context, Result, anyhow};
use dom_query::Document;
use regex::Regex;

use crate::application::ports::ProblemDownloader;
use crate::domain::Sample;

pub struct AtCoderDownloader {
    agent: ureq::Agent,
}

impl AtCoderDownloader {
    pub fn new() -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .user_agent(concat!("oj-rs/", env!("CARGO_PKG_VERSION")))
                .build(),
        }
    }
}

impl Default for AtCoderDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl ProblemDownloader for AtCoderDownloader {
    fn download(&self, url: &str) -> Result<Vec<Sample>> {
        let body = self
            .agent
            .get(url)
            .call()
            .with_context(|| format!("HTTP request failed: {url}"))?
            .into_string()
            .context("failed to read response body")?;
        extract_samples(&body)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SampleKind {
    Input,
    Output,
}

static SAMPLE_HEADING_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Matches "Sample Input 1", "Sample Output 12", "入力例 1", "出力例 1".
    Regex::new(r"^(?P<kind>Sample Input|Sample Output|入力例|出力例)\s*(?P<num>\d+)$").unwrap()
});

fn extract_samples(html: &str) -> Result<Vec<Sample>> {
    let doc = Document::from(html);
    if !doc.select("#task-statement").exists() {
        return Err(anyhow!("could not find #task-statement on the page"));
    }

    let mut inputs: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut outputs: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut pending: Option<(SampleKind, u32)> = None;

    for el in doc.select("#task-statement h3, #task-statement pre").iter() {
        if el.is("h3") {
            pending = parse_heading(el.text().trim());
        } else if let Some((kind, num)) = pending.take() {
            let bucket = match kind {
                SampleKind::Input => &mut inputs,
                SampleKind::Output => &mut outputs,
            };
            // Dedup across EN/JA: first occurrence wins.
            bucket
                .entry(num)
                .or_insert_with(|| el.text().to_string().into_bytes());
        }
    }

    let samples = inputs
        .into_iter()
        .map(|(num, input)| Sample {
            name: format!("sample-{num}"),
            input,
            output: outputs.remove(&num),
        })
        .collect();
    Ok(samples)
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

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn parses_modern_atcoder_layout() {
        let html = indoc! {r#"
            <html><body>
              <div id="task-statement">
                <span class="lang">
                  <span class="lang-en">
                    <h3>Sample Input 1</h3>
                    <section><pre>1 2
            </pre></section>
                    <h3>Sample Output 1</h3>
                    <section><pre>3
            </pre></section>
                    <h3>Sample Input 2</h3>
                    <section><pre>10 20
            </pre></section>
                    <h3>Sample Output 2</h3>
                    <section><pre>30
            </pre></section>
                  </span>
                  <span class="lang-ja">
                    <h3>入力例 1</h3>
                    <section><pre>1 2
            </pre></section>
                    <h3>出力例 1</h3>
                    <section><pre>3
            </pre></section>
                  </span>
                </span>
              </div>
            </body></html>
        "#};
        let samples = extract_samples(html).unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].name, "sample-1");
        assert_eq!(samples[0].input, b"1 2\n");
        assert_eq!(samples[0].output.as_deref(), Some(&b"3\n"[..]));
        assert_eq!(samples[1].name, "sample-2");
        assert_eq!(samples[1].input, b"10 20\n");
        assert_eq!(samples[1].output.as_deref(), Some(&b"30\n"[..]));
    }

    #[test]
    fn parses_legacy_atcoder_layout() {
        let html = indoc! {r#"
            <div id="task-statement">
              <h3>Sample Input 1</h3>
              <pre>foo
            </pre>
              <h3>Sample Output 1</h3>
              <pre>bar
            </pre>
            </div>
        "#};
        let samples = extract_samples(html).unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].input, b"foo\n");
        assert_eq!(samples[0].output.as_deref(), Some(&b"bar\n"[..]));
    }

    #[test]
    fn returns_empty_when_no_samples() {
        let html = r#"<div id="task-statement"><p>no samples here</p></div>"#;
        let samples = extract_samples(html).unwrap();
        assert!(samples.is_empty());
    }

    #[test]
    fn errors_when_task_statement_missing() {
        let err = extract_samples("<html><body>nope</body></html>").unwrap_err();
        assert!(err.to_string().contains("task-statement"));
    }
}
