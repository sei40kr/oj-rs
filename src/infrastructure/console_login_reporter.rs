//! Console implementation of `LoginReporter`. Output line prefixes mirror
//! the rest of the CLI (`[+]` / `[!]`).

use crate::application::ports::LoginReporter;

pub struct ConsoleLoginReporter;

impl ConsoleLoginReporter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsoleLoginReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginReporter for ConsoleLoginReporter {
    fn already_logged_in(&self) {
        println!("[+] You have already signed in.");
    }

    fn not_logged_in(&self) {
        eprintln!("[!] You are not signed in.");
    }

    fn login_succeeded(&self) {
        println!("[+] Successfully signed in.");
    }
}
