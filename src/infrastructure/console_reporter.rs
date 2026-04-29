//! Console implementation of `Reporter`. Output mirrors upstream `oj test`
//! closely so existing scripts that grep this output keep working.

use std::sync::Mutex;

use crate::application::ports::{DiscoveryQuery, Reporter};
use crate::domain::{CaseOutcome, DisplayMode, Verdict};

pub struct ConsoleReporter {
    /// Serializes prints across worker threads so per-case output blocks
    /// stay contiguous when running with `-j`.
    print_lock: Mutex<()>,
}

impl ConsoleReporter {
    pub fn new() -> Self {
        Self {
            print_lock: Mutex::new(()),
        }
    }
}

impl Default for ConsoleReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for ConsoleReporter {
    fn run_started(&self, total: usize) {
        let _guard = self.print_lock.lock().unwrap();
        println!("[*] {total} cases found");
    }

    fn no_cases_found(&self, query: &DiscoveryQuery) {
        let _guard = self.print_lock.lock().unwrap();
        eprintln!(
            "[!] no test cases found under {}/ (format: {})",
            query.directory.display(),
            query.format
        );
        println!("test success: 0 cases");
    }

    fn case_finished(
        &self,
        outcome: &CaseOutcome,
        display_mode: DisplayMode,
        silent: bool,
        print_input: bool,
    ) {
        let _guard = self.print_lock.lock().unwrap();
        let elapsed_ms = outcome.elapsed.as_millis();
        let suffix = match outcome.verdict {
            Verdict::RuntimeError => format!(
                " (exit {})",
                outcome
                    .exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".into())
            ),
            _ => String::new(),
        };
        println!(
            "[{}] {} ({elapsed_ms} ms){suffix}",
            outcome.verdict.label(),
            outcome.name,
        );

        if silent {
            return;
        }
        let dump_always = matches!(display_mode, DisplayMode::All | DisplayMode::DiffAll);
        let on_failure = outcome.verdict != Verdict::Accepted;
        if !(dump_always || on_failure) {
            return;
        }

        if print_input {
            print_block("input", &outcome.input);
        }
        match display_mode {
            DisplayMode::Diff | DisplayMode::DiffAll => {
                print_diff(&outcome.actual, outcome.expected.as_deref());
            }
            _ => {
                print_block("output", &outcome.actual);
                if let Some(expected) = &outcome.expected {
                    print_block("expected", expected);
                }
            }
        }
    }

    fn run_finished(&self, outcomes: &[CaseOutcome]) {
        let _guard = self.print_lock.lock().unwrap();
        if let Some(slowest) = outcomes.iter().max_by_key(|o| o.elapsed) {
            println!(
                "slowest: {} ms ({})",
                slowest.elapsed.as_millis(),
                slowest.name
            );
        }
        let accepted = outcomes
            .iter()
            .filter(|o| o.verdict == Verdict::Accepted)
            .count();
        let total = outcomes.len();
        if accepted == total {
            println!("[+] test success: {accepted} cases");
        } else {
            println!("[-] test failed: {accepted} AC / {total} cases");
        }
    }
}

fn print_block(label: &str, data: &[u8]) {
    println!("[*] {label}:");
    let text = String::from_utf8_lossy(data);
    for line in text.lines() {
        println!("    {line}");
    }
    if !data.is_empty() && !data.ends_with(b"\n") {
        println!("    [no trailing newline]");
    }
}

fn print_diff(actual: &[u8], expected: Option<&[u8]>) {
    let expected = expected.unwrap_or(b"");
    let actual_text = String::from_utf8_lossy(actual);
    let expected_text = String::from_utf8_lossy(expected);
    let diff = similar::TextDiff::from_lines(&expected_text, &actual_text);
    println!("[*] diff (- expected, + actual):");
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => " ",
        };
        print!("    {sign}{}", change);
        if !change.value().ends_with('\n') {
            println!();
        }
    }
}
