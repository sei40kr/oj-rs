//! Use case: authenticate with an online judge so subsequent commands can
//! act on the user's behalf. Holds no I/O — depends only on ports.

use anyhow::{Result, bail};

use super::ports::{Authenticator, CredentialPrompt, Credentials, LoginReporter};

#[derive(Debug, Clone, Default)]
pub struct LoginInput {
    pub username: Option<String>,
    pub password: Option<String>,
    pub check_only: bool,
}

#[derive(Debug)]
pub struct LoginOutput {
    pub logged_in: bool,
}

pub struct Login<'a> {
    pub authenticator: &'a dyn Authenticator,
    pub prompt: &'a dyn CredentialPrompt,
    pub reporter: &'a dyn LoginReporter,
}

impl<'a> Login<'a> {
    pub fn execute(&self, input: LoginInput) -> Result<LoginOutput> {
        if self.authenticator.is_logged_in()? {
            self.reporter.already_logged_in();
            return Ok(LoginOutput { logged_in: true });
        }
        if input.check_only {
            self.reporter.not_logged_in();
            return Ok(LoginOutput { logged_in: false });
        }

        let username = match input.username {
            Some(u) => u,
            None => self.prompt.prompt_username()?,
        };
        if username.is_empty() {
            bail!("username must not be empty");
        }
        let password = match input.password {
            Some(p) => p,
            None => self.prompt.prompt_password()?,
        };
        if password.is_empty() {
            bail!("password must not be empty");
        }

        self.authenticator
            .login(&Credentials { username, password })?;
        self.reporter.login_succeeded();
        Ok(LoginOutput { logged_in: true })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeAuthenticator {
        already_logged_in: bool,
        login_calls: Mutex<Vec<Credentials>>,
        accept_login: bool,
    }

    impl Authenticator for FakeAuthenticator {
        fn is_logged_in(&self) -> Result<bool> {
            Ok(self.already_logged_in
                || (self.accept_login && !self.login_calls.lock().unwrap().is_empty()))
        }

        fn login(&self, credentials: &Credentials) -> Result<()> {
            self.login_calls.lock().unwrap().push(credentials.clone());
            if self.accept_login {
                Ok(())
            } else {
                anyhow::bail!("login rejected")
            }
        }
    }

    #[derive(Default)]
    struct ScriptedPrompt {
        username: Mutex<Option<String>>,
        password: Mutex<Option<String>>,
    }

    impl CredentialPrompt for ScriptedPrompt {
        fn prompt_username(&self) -> Result<String> {
            Ok(self
                .username
                .lock()
                .unwrap()
                .take()
                .expect("no username staged"))
        }
        fn prompt_password(&self) -> Result<String> {
            Ok(self
                .password
                .lock()
                .unwrap()
                .take()
                .expect("no password staged"))
        }
    }

    #[derive(Default)]
    struct RecordingReporter {
        events: Mutex<Vec<&'static str>>,
    }

    impl LoginReporter for RecordingReporter {
        fn already_logged_in(&self) {
            self.events.lock().unwrap().push("already_logged_in");
        }
        fn not_logged_in(&self) {
            self.events.lock().unwrap().push("not_logged_in");
        }
        fn login_succeeded(&self) {
            self.events.lock().unwrap().push("login_succeeded");
        }
    }

    #[test]
    fn skips_login_when_already_authenticated() {
        let auth = FakeAuthenticator {
            already_logged_in: true,
            accept_login: true,
            ..Default::default()
        };
        let prompt = ScriptedPrompt::default();
        let reporter = RecordingReporter::default();
        let use_case = Login {
            authenticator: &auth,
            prompt: &prompt,
            reporter: &reporter,
        };

        let output = use_case.execute(LoginInput::default()).unwrap();
        assert!(output.logged_in);
        assert!(auth.login_calls.lock().unwrap().is_empty());
        assert_eq!(*reporter.events.lock().unwrap(), vec!["already_logged_in"]);
    }

    #[test]
    fn check_only_reports_not_logged_in_without_prompting() {
        let auth = FakeAuthenticator::default();
        let prompt = ScriptedPrompt::default();
        let reporter = RecordingReporter::default();
        let use_case = Login {
            authenticator: &auth,
            prompt: &prompt,
            reporter: &reporter,
        };

        let output = use_case
            .execute(LoginInput {
                check_only: true,
                ..Default::default()
            })
            .unwrap();
        assert!(!output.logged_in);
        assert_eq!(*reporter.events.lock().unwrap(), vec!["not_logged_in"]);
    }

    #[test]
    fn prompts_only_for_missing_fields() {
        let auth = FakeAuthenticator {
            accept_login: true,
            ..Default::default()
        };
        let prompt = ScriptedPrompt {
            password: Mutex::new(Some("pw".to_string())),
            ..Default::default()
        };
        let reporter = RecordingReporter::default();
        let use_case = Login {
            authenticator: &auth,
            prompt: &prompt,
            reporter: &reporter,
        };

        use_case
            .execute(LoginInput {
                username: Some("alice".to_string()),
                password: None,
                check_only: false,
            })
            .unwrap();

        let calls = auth.login_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].username, "alice");
        assert_eq!(calls[0].password, "pw");
        assert_eq!(*reporter.events.lock().unwrap(), vec!["login_succeeded"]);
    }

    #[test]
    fn rejects_empty_username() {
        let auth = FakeAuthenticator::default();
        let prompt = ScriptedPrompt::default();
        let reporter = RecordingReporter::default();
        let use_case = Login {
            authenticator: &auth,
            prompt: &prompt,
            reporter: &reporter,
        };

        let err = use_case
            .execute(LoginInput {
                username: Some(String::new()),
                password: Some("pw".to_string()),
                check_only: false,
            })
            .unwrap_err();
        assert!(err.to_string().contains("username"));
        assert!(auth.login_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn rejects_empty_password() {
        let auth = FakeAuthenticator::default();
        let prompt = ScriptedPrompt::default();
        let reporter = RecordingReporter::default();
        let use_case = Login {
            authenticator: &auth,
            prompt: &prompt,
            reporter: &reporter,
        };

        let err = use_case
            .execute(LoginInput {
                username: Some("alice".to_string()),
                password: Some(String::new()),
                check_only: false,
            })
            .unwrap_err();
        assert!(err.to_string().contains("password"));
        assert!(auth.login_calls.lock().unwrap().is_empty());
    }
}
