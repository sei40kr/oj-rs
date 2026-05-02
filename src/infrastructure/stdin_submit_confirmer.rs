//! Terminal-based implementation of `SubmitConfirmer`. Reads a single character
//! from stdin and treats anything other than `y`/`Y` as a no — matches upstream
//! `oj submit`'s default-No behaviour so a stray Enter doesn't fire a real POST.

use std::io::{self, Read, Write};

use anyhow::{Context, Result};

use crate::application::ports::SubmitConfirmer;

pub struct StdinSubmitConfirmer;

impl StdinSubmitConfirmer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StdinSubmitConfirmer {
    fn default() -> Self {
        Self::new()
    }
}

impl SubmitConfirmer for StdinSubmitConfirmer {
    fn confirm_submit(&self) -> Result<bool> {
        eprint!("Are you sure? [y/N] ");
        io::stderr().flush().ok();
        let mut buf = [0u8; 1];
        match io::stdin().lock().read(&mut buf) {
            Ok(0) => Ok(false),
            Ok(_) => Ok(matches!(buf[0], b'y' | b'Y')),
            Err(e) => Err(e).context("failed to read confirmation"),
        }
    }
}
