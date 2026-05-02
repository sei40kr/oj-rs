//! AtCoder implementation of `Submitter`. Reuses the same JSON cookie jar the
//! authenticator writes, so a prior `oj login` carries over to `oj submit`.
//!
//! Submission flow (mirrors upstream Python `oj`):
//! 1. Parse contest/problem ids from the user-supplied task URL.
//! 2. Language list — GET the task page and scrape
//!    `<form action="/contests/{cid}/submit"> <select name="data.LanguageId">`.
//!    Requires authentication; AtCoder hides the dropdown behind login.
//! 3. Submit — GET the submit page to grab the CSRF token, then POST
//!    `csrf_token`, `data.TaskScreenName`, `data.LanguageId`, `sourceCode`.
//!    AtCoder responds with `302`; success redirects to `/submissions/me`.
//! 4. Resolve the freshly created submission URL from the first row of
//!    `/submissions/me`.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use cookie_store::CookieStore;
use dom_query::Document;
use regex::Regex;

use crate::application::ports::{SubmissionReceipt, Submitter};
use crate::domain::Language;

const ATCODER_ORIGIN: &str = "https://atcoder.jp";
const USER_AGENT: &str = concat!("oj-rs/", env!("CARGO_PKG_VERSION"));
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct AtCoderSubmitter {
    agent: ureq::Agent,
    cookie_path: PathBuf,
}

impl AtCoderSubmitter {
    pub fn new(cookie_path: PathBuf) -> Result<Self> {
        let store = load_cookie_store(&cookie_path)?;
        let agent = ureq::AgentBuilder::new()
            .user_agent(USER_AGENT)
            // Inspect the 302 from POST /submit ourselves so we can detect
            // login redirects vs. the success redirect to /submissions/me.
            .redirects(0)
            .timeout_connect(HTTP_CONNECT_TIMEOUT)
            .timeout_read(HTTP_READ_TIMEOUT)
            .cookie_store(store)
            .build();
        Ok(Self { agent, cookie_path })
    }

    fn save_cookies(&self) -> Result<()> {
        if let Some(parent) = self.cookie_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let file = create_private_file(&self.cookie_path)?;
        let mut writer = BufWriter::new(file);
        let store = self.agent.cookie_store();
        cookie_store::serde::json::save(&store, &mut writer)
            .map_err(|e| anyhow!("saving cookies: {e}"))?;
        Ok(())
    }
}

impl Submitter for AtCoderSubmitter {
    fn available_languages(&self, problem_url: &str) -> Result<Vec<Language>> {
        ProblemRef::parse(problem_url)?;
        let body = follow_redirects_get(&self.agent, problem_url)?;
        if body_looks_like_login(&body) {
            bail!("not signed in: run `oj login {ATCODER_ORIGIN}/` first");
        }
        let languages = extract_languages(&body);
        if languages.is_empty() {
            bail!(
                "no submission languages found on the problem page (are you signed in to AtCoder?)"
            );
        }
        Ok(languages)
    }

    fn submit(
        &self,
        problem_url: &str,
        language_id: &str,
        source_code: &[u8],
    ) -> Result<SubmissionReceipt> {
        let problem = ProblemRef::parse(problem_url)?;
        let submit_url = format!("{ATCODER_ORIGIN}/contests/{}/submit", problem.contest_id);

        let body = follow_redirects_get(&self.agent, &submit_url)?;
        if body_looks_like_login(&body) {
            bail!("not signed in: run `oj login {ATCODER_ORIGIN}/` first");
        }
        let csrf_token = extract_csrf_token(&body).with_context(|| format!("on {submit_url}"))?;
        let has_turnstile = page_has_turnstile(&body);

        let source = std::str::from_utf8(source_code).context("source file is not valid UTF-8")?;

        let response = self
            .agent
            .post(&submit_url)
            .set("Referer", &submit_url)
            .send_form(&[
                ("data.TaskScreenName", problem.problem_id.as_str()),
                ("data.LanguageId", language_id),
                ("sourceCode", source),
                ("csrf_token", csrf_token.as_str()),
            ]);

        let location = match response {
            Ok(resp) if (300..400).contains(&resp.status()) => resp
                .header("Location")
                .map(|s| s.to_string())
                .unwrap_or_default(),
            Ok(resp) if resp.status() == 200 => {
                // AtCoder responds 200 with a re-rendered form when validation
                // fails — most commonly because the Cloudflare Turnstile widget
                // wasn't solved. Match the authenticator's diagnostic.
                if has_turnstile {
                    bail!(
                        "submission rejected: AtCoder's submit form is protected by a Cloudflare \
                         Turnstile challenge that can't be solved without a real browser."
                    );
                }
                bail!("submission rejected by AtCoder (200 from POST {submit_url})")
            }
            Ok(resp) => bail!("unexpected response from submit: {}", resp.status()),
            Err(ureq::Error::Status(code, _)) => bail!("unexpected response from submit: {code}"),
            Err(e) => bail!("HTTP request failed: {e}"),
        };

        if !location.contains("/submissions/me") {
            // AtCoder bounces back to the submit page with an `<div class="alert">`
            // on duplicate submissions, language mismatches, etc. Surface what
            // we can without re-parsing those alerts in detail.
            if location.contains("/login") {
                bail!("submission rejected: session expired, run `oj login` again");
            }
            bail!("submission rejected by AtCoder (redirected to {location})");
        }

        let me_url = format!(
            "{ATCODER_ORIGIN}/contests/{}/submissions/me",
            problem.contest_id
        );
        let me_body = follow_redirects_get(&self.agent, &me_url)?;
        let submission_url = latest_submission_url(&me_body, &problem.contest_id)
            .with_context(|| "could not locate submission URL on /submissions/me")?;

        // Best-effort: refresh persisted cookies so the next run starts from
        // the latest session state. Submission already succeeded, so don't fail
        // the whole call if the disk write hits an error.
        if let Err(e) = self.save_cookies() {
            eprintln!("[!] warning: failed to persist cookies: {e:#}");
        }

        Ok(SubmissionReceipt { submission_url })
    }
}

#[derive(Debug)]
struct ProblemRef {
    contest_id: String,
    problem_id: String,
}

impl ProblemRef {
    fn parse(problem_url: &str) -> Result<Self> {
        let url = url::Url::parse(problem_url).context("invalid problem URL")?;
        let host = url
            .host_str()
            .context("URL has no host")?
            .trim_start_matches("www.");
        if host != "atcoder.jp" && host != "beta.atcoder.jp" {
            bail!("not an AtCoder problem URL: {problem_url}");
        }
        let segments: Vec<&str> = url
            .path_segments()
            .map(|s| s.filter(|x| !x.is_empty()).collect())
            .unwrap_or_default();
        match segments.as_slice() {
            ["contests", contest, "tasks", problem, ..] => Ok(Self {
                contest_id: (*contest).to_string(),
                problem_id: (*problem).to_string(),
            }),
            _ => bail!("unsupported AtCoder URL: {problem_url}"),
        }
    }
}

fn load_cookie_store(path: &Path) -> Result<CookieStore> {
    if !path.exists() {
        return Ok(CookieStore::default());
    }
    let file =
        File::open(path).with_context(|| format!("opening cookie jar at {}", path.display()))?;
    cookie_store::serde::json::load(BufReader::new(file))
        .map_err(|e| anyhow!("loading cookie jar: {e}"))
}

fn create_private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .with_context(|| format!("creating {}", path.display()))
}

/// GET while manually following a small number of redirects. The agent has
/// `redirects(0)` so the POST flow can introspect the 302 it gets back, but
/// for plain page fetches we still want to follow them.
fn follow_redirects_get(agent: &ureq::Agent, url: &str) -> Result<String> {
    let mut current = url.to_string();
    for _ in 0..5 {
        let response = agent
            .get(&current)
            .call()
            .with_context(|| format!("HTTP request failed: {current}"))?;
        let status = response.status();
        if (300..400).contains(&status) {
            let next = response
                .header("Location")
                .ok_or_else(|| anyhow!("redirect from {current} had no Location"))?;
            current = absolute_url(&current, next)?;
            continue;
        }
        return response
            .into_string()
            .with_context(|| format!("reading body from {current}"));
    }
    bail!("too many redirects starting at {url}")
}

fn absolute_url(base: &str, href: &str) -> Result<String> {
    let base = url::Url::parse(base).context("invalid base URL")?;
    let joined = base.join(href).context("invalid redirect target")?;
    Ok(joined.to_string())
}

fn body_looks_like_login(body: &str) -> bool {
    body.contains("name=\"username\"") && body.contains("name=\"password\"")
}

fn page_has_turnstile(body: &str) -> bool {
    body.contains("challenges.cloudflare.com/turnstile") || body.contains("cf-turnstile")
}

fn extract_languages(html: &str) -> Vec<Language> {
    let doc = Document::from(html);
    let select = doc.select("select[name=\"data.LanguageId\"]").first();
    if !select.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for option in select.select("option").iter() {
        let id = match option.attr("value") {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => continue,
        };
        let name = option.text().trim().to_string();
        if name.is_empty() {
            continue;
        }
        out.push(Language { id, name });
    }
    out
}

fn extract_csrf_token(html: &str) -> Result<String> {
    let doc = Document::from(html);
    let candidates = [
        r#"form[action$="/submit"] input[name="csrf_token"]"#,
        r#"form input[name="csrf_token"]"#,
    ];
    for selector in candidates {
        let selection = doc.select(selector).first();
        if let Some(value) = selection.attr("value") {
            return Ok(value.to_string());
        }
    }
    Err(anyhow!("csrf_token not found on submit page"))
}

static SUBMISSION_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/contests/([^/]+)/submissions/(\d+)").unwrap());

fn latest_submission_url(html: &str, contest_id: &str) -> Option<String> {
    SUBMISSION_LINK_RE
        .captures_iter(html)
        .find(|c| &c[1] == contest_id)
        .map(|c| format!("{ATCODER_ORIGIN}/contests/{}/submissions/{}", &c[1], &c[2]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn problem_ref_parses_modern_url() {
        let r = ProblemRef::parse("https://atcoder.jp/contests/abc123/tasks/abc123_a").unwrap();
        assert_eq!(r.contest_id, "abc123");
        assert_eq!(r.problem_id, "abc123_a");
    }

    #[test]
    fn problem_ref_rejects_non_atcoder() {
        let err = ProblemRef::parse("https://codeforces.com/contest/1/problem/A").unwrap_err();
        assert!(err.to_string().contains("AtCoder"));
    }

    #[test]
    fn problem_ref_rejects_unrelated_path() {
        let err = ProblemRef::parse("https://atcoder.jp/home").unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn extracts_languages_from_select() {
        let html = indoc! {r#"
            <form action="/contests/abc123/submit" method="POST">
              <select name="data.LanguageId">
                <option value="">(select)</option>
                <option value="5001">C++ 20 (gcc 12.2)</option>
                <option value="5028">Python (CPython 3.11.4)</option>
              </select>
            </form>
        "#};
        let langs = extract_languages(html);
        assert_eq!(langs.len(), 2);
        assert_eq!(langs[0].id, "5001");
        assert_eq!(langs[0].name, "C++ 20 (gcc 12.2)");
        assert_eq!(langs[1].id, "5028");
    }

    #[test]
    fn extracts_csrf_token_anchored_to_submit_form() {
        let html = indoc! {r#"
            <form action="/logout" method="POST">
              <input type="hidden" name="csrf_token" value="logout-token">
            </form>
            <form action="/contests/abc123/submit" method="POST">
              <input type="hidden" name="csrf_token" value="submit-token">
              <select name="data.LanguageId"></select>
            </form>
        "#};
        assert_eq!(extract_csrf_token(html).unwrap(), "submit-token");
    }

    #[test]
    fn submission_url_picks_first_matching_contest_row() {
        let html = indoc! {r#"
            <table><tbody>
              <tr><td><a href="/contests/abc123/submissions/9001">Detail</a></td></tr>
              <tr><td><a href="/contests/abc123/submissions/8000">Detail</a></td></tr>
            </tbody></table>
        "#};
        let url = latest_submission_url(html, "abc123").unwrap();
        assert_eq!(url, "https://atcoder.jp/contests/abc123/submissions/9001");
    }

    #[test]
    fn submission_url_ignores_other_contest_rows() {
        let html = indoc! {r#"
            <a href="/contests/abc999/submissions/1">x</a>
            <a href="/contests/abc123/submissions/42">match</a>
        "#};
        let url = latest_submission_url(html, "abc123").unwrap();
        assert!(url.ends_with("/contests/abc123/submissions/42"));
    }

    #[test]
    fn login_page_is_detectable() {
        let html = r#"<form><input name="username"><input name="password"></form>"#;
        assert!(body_looks_like_login(html));
    }

    #[test]
    fn turnstile_widget_is_detectable() {
        let html = r#"<script src="https://challenges.cloudflare.com/turnstile/v0/api.js"></script>
            <div class="cf-challenge"></div>"#;
        assert!(page_has_turnstile(html));
        assert!(!page_has_turnstile(
            r#"<form><input name="csrf_token"></form>"#
        ));
    }
}
