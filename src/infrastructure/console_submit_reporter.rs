//! Console implementation of `SubmitReporter`. Output line prefixes mirror
//! the rest of the CLI (`[*]` / `[+]` / `[!]`).

use crate::application::ports::{SubmissionReceipt, SubmitReporter};
use crate::domain::Language;

pub struct ConsoleSubmitReporter;

impl ConsoleSubmitReporter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsoleSubmitReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl SubmitReporter for ConsoleSubmitReporter {
    fn source_loaded(&self, byte_count: usize) {
        println!("[*] code ({byte_count} bytes)");
    }

    fn language_chosen(&self, language: &Language) {
        println!("[+] chosen language: {} ({})", language.id, language.name);
    }

    fn language_unresolved(&self, candidates: &[Language]) {
        if candidates.is_empty() {
            eprintln!("[!] no language matched");
            return;
        }
        eprintln!("[!] you have to choose:");
        let mut sorted: Vec<&Language> = candidates.iter().collect();
        sorted.sort_by(|a, b| a.id.cmp(&b.id));
        for lang in sorted {
            eprintln!("    {} ({})", lang.id, lang.name);
        }
    }

    fn submission_aborted(&self) {
        eprintln!("[!] submission aborted.");
    }

    fn submission_succeeded(&self, receipt: &SubmissionReceipt) {
        println!("[+] success: {}", receipt.submission_url);
    }
}
