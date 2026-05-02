//! AtCoder implementation of `Authenticator`. Performs the form-based login
//! flow and persists session cookies to disk so later commands can reuse them.
//!
//! Flow (mirrors the upstream Python `oj`):
//! 1. `GET /login` to read the CSRF token (and pick up the initial REVEL_SESSION cookie).
//! 2. `POST /login` with `username`, `password`, `csrf_token`. Both success and
//!    failure respond with `302`; the difference is whether the resulting
//!    session is authorized.
//! 3. Verify by hitting an authenticated endpoint and writing the cookie jar.
//!
//! The cookie jar format is `cookie_store`'s JSON layout, which is **not**
//! interchangeable with upstream's Mozilla cookies.txt format.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use cookie_store::CookieStore;
use dom_query::Document;

use crate::application::ports::{Authenticator, Credentials};

const LOGIN_URL: &str = "https://atcoder.jp/login";
/// An authenticated-only page: 200 when logged in, 302 to /login otherwise.
const SESSION_PROBE_URL: &str = "https://atcoder.jp/contests/practice/submit";
const USER_AGENT: &str = concat!("oj-rs/", env!("CARGO_PKG_VERSION"));
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct AtCoderAuthenticator {
    agent: ureq::Agent,
    cookie_path: PathBuf,
}

impl AtCoderAuthenticator {
    pub fn new(cookie_path: PathBuf) -> Result<Self> {
        let store = load_cookie_store(&cookie_path)?;
        let agent = ureq::AgentBuilder::new()
            .user_agent(USER_AGENT)
            // We need to inspect the 302 returned by /login directly, so don't
            // chase redirects for any request the authenticator makes.
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

/// Create a writable file readable only by the current user. Cookie jars hold
/// live session tokens, so on multi-user systems they must not be world-readable.
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

impl Authenticator for AtCoderAuthenticator {
    fn is_logged_in(&self) -> Result<bool> {
        let response = self
            .agent
            .get(SESSION_PROBE_URL)
            .call()
            .with_context(|| format!("HTTP request failed: {SESSION_PROBE_URL}"))?;
        Ok(response.status() == 200)
    }

    fn login(&self, credentials: &Credentials) -> Result<()> {
        let body = self
            .agent
            .get(LOGIN_URL)
            .call()
            .with_context(|| format!("HTTP request failed: {LOGIN_URL}"))?
            .into_string()
            .context("failed to read login page")?;
        let csrf_token = extract_csrf_token(&body)?;
        let has_turnstile = login_form_has_turnstile(&body);

        let response = self.agent.post(LOGIN_URL).send_form(&[
            ("username", credentials.username.as_str()),
            ("password", credentials.password.as_str()),
            ("csrf_token", csrf_token.as_str()),
        ]);
        match response {
            // AtCoder always 302s; the redirect target reveals success vs failure,
            // but we re-probe below rather than parsing the Location header.
            Ok(resp) if (300..400).contains(&resp.status()) => {}
            Ok(resp) => bail!("unexpected response from login: {}", resp.status()),
            Err(ureq::Error::Status(code, _)) => {
                bail!("unexpected response from login: {code}")
            }
            Err(e) => bail!("HTTP request failed: {e}"),
        }

        if !self.is_logged_in()? {
            // AtCoder gates the login form behind a Cloudflare Turnstile widget that
            // expects a JS-solved `cf-turnstile-response` field. We can't produce one
            // from a plain HTTP client, so the POST is rejected even with a valid
            // username/password. Tell the user what's actually going on.
            if has_turnstile {
                bail!(
                    "login failed: AtCoder's login form is protected by a Cloudflare \
                     Turnstile challenge that can't be solved without a real browser. \
                     Sign in via your browser and pass the exported cookie jar with \
                     `--cookie PATH`."
                );
            }
            bail!("login failed: incorrect username or password");
        }
        self.save_cookies()?;
        Ok(())
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

/// AtCoder's login HTML actually contains two `csrf_token` inputs: one in the
/// site-wide logout form (which appears earlier in the DOM) and one in the
/// login form. They happen to share a value today, but we anchor to the login
/// form (`<form action="" method="POST">`) so a future divergence doesn't
/// silently submit the wrong token.
fn extract_csrf_token(html: &str) -> Result<String> {
    let doc = Document::from(html);
    let candidates = [
        r#"form[action=""] input[name=csrf_token]"#,
        "form.form-horizontal input[name=csrf_token]",
        "form input[name=username] ~ input[name=csrf_token]",
        "form input[name=csrf_token]",
    ];
    for selector in candidates {
        let selection = doc.select(selector).first();
        if let Some(value) = selection.attr("value") {
            return Ok(value.to_string());
        }
    }
    Err(anyhow!("csrf_token not found on login page"))
}

fn login_form_has_turnstile(html: &str) -> bool {
    html.contains("challenges.cloudflare.com/turnstile") || html.contains("cf-turnstile")
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn extracts_csrf_token_from_login_form() {
        let html = indoc! {r#"
            <form method="POST" action="">
              <input type="hidden" name="csrf_token" value="abc123">
              <input type="text" name="username">
              <input type="password" name="password">
            </form>
        "#};
        assert_eq!(extract_csrf_token(html).unwrap(), "abc123");
    }

    #[test]
    fn errors_when_csrf_token_missing() {
        let html = r#"<form><input name="username"></form>"#;
        let err = extract_csrf_token(html).unwrap_err();
        assert!(err.to_string().contains("csrf_token"));
    }

    #[test]
    fn picks_login_form_token_not_logout_token() {
        // Mirrors AtCoder's actual page: logout form appears first, login form
        // (action="") appears later. We must pick the latter.
        let html = indoc! {r#"
            <form method="POST" name="form_logout" action="/logout">
              <input type="hidden" name="csrf_token" value="logout-token">
            </form>
            <form class="form-horizontal" action="" method="POST">
              <input type="text" name="username">
              <input type="password" name="password">
              <input type="hidden" name="csrf_token" value="login-token">
            </form>
        "#};
        assert_eq!(extract_csrf_token(html).unwrap(), "login-token");
    }

    #[test]
    fn detects_turnstile_widget() {
        let html = r#"<script src="https://challenges.cloudflare.com/turnstile/v0/api.js"></script>
            <div class="cf-challenge" data-sitekey="x"></div>"#;
        assert!(login_form_has_turnstile(html));
    }

    #[test]
    fn no_turnstile_on_plain_page() {
        assert!(!login_form_has_turnstile(
            r#"<form><input name="username"></form>"#
        ));
    }
}
