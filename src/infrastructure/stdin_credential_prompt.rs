//! Terminal-based implementation of `CredentialPrompt`. The username is read
//! from stdin in plaintext; the password uses `rpassword` so it isn't echoed
//! when the program is attached to a TTY.

use std::io::{self, BufRead, Write};

use anyhow::{Context, Result};

use crate::application::ports::CredentialPrompt;

pub struct StdinCredentialPrompt;

impl StdinCredentialPrompt {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StdinCredentialPrompt {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialPrompt for StdinCredentialPrompt {
    fn prompt_username(&self) -> Result<String> {
        eprint!("Username: ");
        io::stderr().flush().ok();
        let mut line = String::new();
        io::stdin()
            .lock()
            .read_line(&mut line)
            .context("failed to read username")?;
        Ok(line
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_string())
    }

    fn prompt_password(&self) -> Result<String> {
        rpassword::prompt_password("Password: ").context("failed to read password")
    }
}
