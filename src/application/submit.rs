//! Use case: submit a source file to an online judge. Holds no I/O — depends
//! only on ports. The "wait" delay is intentionally implemented here with
//! `std::thread::sleep` to mirror upstream's split-around-the-prompt timing;
//! tests pass `wait = Duration::ZERO` to avoid actually sleeping.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::domain::Language;

use super::ports::{SubmitConfirmer, SubmitReporter, Submitter};

#[derive(Debug, Clone)]
pub struct SubmitInput {
    pub problem_url: String,
    pub file: PathBuf,
    /// Either an exact language `id` (matched verbatim against the available
    /// list) or one or more space-separated keywords used as case-insensitive
    /// substring filters over the language `name`.
    pub language: Option<String>,
    /// When true, narrow the language list by the file extension before
    /// applying the keyword filter. Mirrors upstream's `--guess` (default on).
    pub guess: bool,
    /// Skip the interactive confirmation prompt.
    pub yes: bool,
    /// Total wait, split half before the prompt and half after — gives the user
    /// time to ^C if they realised they meant a different file.
    pub wait: Duration,
}

#[derive(Debug)]
pub struct SubmitOutput {
    pub submitted: bool,
}

pub struct Submit<'a> {
    pub submitter: &'a dyn Submitter,
    pub confirmer: &'a dyn SubmitConfirmer,
    pub reporter: &'a dyn SubmitReporter,
}

impl<'a> Submit<'a> {
    pub fn execute(&self, input: SubmitInput) -> Result<SubmitOutput> {
        let source =
            fs::read(&input.file).with_context(|| format!("reading {}", input.file.display()))?;
        self.reporter.source_loaded(source.len());

        let languages = self.submitter.available_languages(&input.problem_url)?;
        let chosen = match resolve_language(
            &languages,
            input.language.as_deref(),
            input.guess,
            &input.file,
        ) {
            Resolution::Single(lang) => lang.clone(),
            Resolution::None => {
                self.reporter.language_unresolved(&languages);
                bail!("no language matched; pass --language to choose one");
            }
            Resolution::Multiple(candidates) => {
                self.reporter.language_unresolved(&candidates);
                bail!("language is ambiguous; pass --language to narrow it down");
            }
        };
        self.reporter.language_chosen(&chosen);

        let half = input.wait / 2;
        if !half.is_zero() {
            thread::sleep(half);
        }

        if !input.yes && !self.confirmer.confirm_submit()? {
            self.reporter.submission_aborted();
            return Ok(SubmitOutput { submitted: false });
        }

        if !half.is_zero() {
            thread::sleep(half);
        }

        let receipt = self
            .submitter
            .submit(&input.problem_url, &chosen.id, &source)?;
        self.reporter.submission_succeeded(&receipt);
        Ok(SubmitOutput { submitted: true })
    }
}

enum Resolution<'l> {
    Single(&'l Language),
    None,
    Multiple(Vec<Language>),
}

fn resolve_language<'l>(
    languages: &'l [Language],
    requested: Option<&str>,
    guess: bool,
    file: &Path,
) -> Resolution<'l> {
    if let Some(req) = requested {
        if let Some(exact) = languages.iter().find(|l| l.id == req) {
            return Resolution::Single(exact);
        }
    }

    let mut candidates: Vec<&Language> = if guess {
        let by_ext = guess_by_extension(languages, file);
        if by_ext.is_empty() {
            languages.iter().collect()
        } else {
            by_ext
        }
    } else {
        languages.iter().collect()
    };

    if let Some(req) = requested {
        candidates.retain(|l| matches_keywords(&l.name, req));
    }

    match candidates.as_slice() {
        [] => Resolution::None,
        [only] => Resolution::Single(only),
        many => Resolution::Multiple(many.iter().map(|l| (*l).clone()).collect()),
    }
}

/// Names like `"C++ 20 (gcc 12.2)"` should match keyword `"C++"` but not
/// `"C "` — case-insensitive substring on each space-separated keyword,
/// requiring all keywords to be present (matches upstream).
fn matches_keywords(name: &str, requested: &str) -> bool {
    let lower = name.to_lowercase();
    requested
        .split_whitespace()
        .all(|kw| lower.contains(&kw.to_lowercase()))
}

/// Per-extension dispatch matching upstream `guess_lang_ids_of_file`. The
/// `--guess-cxx-*` / `--guess-python-*` knobs are not yet ported, so for
/// `.cpp` / `.py` we return every C++ / Python entry; the user disambiguates
/// with `-l "C++ 20"` etc.
fn guess_by_extension<'l>(languages: &'l [Language], file: &Path) -> Vec<&'l Language> {
    let Some(raw_ext) = file.extension().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    // `.C` is the legacy C++ extension (case-sensitive in upstream).
    if raw_ext == "C" {
        return languages
            .iter()
            .filter(|l| is_cxx_description(&l.name))
            .collect();
    }
    let ext = raw_ext.to_ascii_lowercase();
    match ext.as_str() {
        "cpp" | "cxx" | "cc" => {
            return languages
                .iter()
                .filter(|l| is_cxx_description(&l.name))
                .collect();
        }
        "py" => {
            return languages
                .iter()
                .filter(|l| is_python_description(&l.name))
                .collect();
        }
        _ => {}
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for rule in EXTENSION_TABLE {
        if !rule.exts.iter().any(|e| *e == ext) {
            continue;
        }
        for lang in languages {
            if matches_description(&lang.name, rule.keyword, rule.split) && seen.insert(&lang.id) {
                out.push(lang);
            }
        }
    }
    out
}

struct ExtensionRule {
    exts: &'static [&'static str],
    keyword: &'static str,
    /// When true, the description is tokenized on whitespace and the keyword
    /// must equal one of the tokens — needed to keep short keywords (`c`, `d`,
    /// `go`, `perl`) from substring-matching unrelated names.
    split: bool,
}

/// Mirrors the upstream table in `submit.py:guess_lang_ids_of_file` so
/// extension coverage stays identical.
const EXTENSION_TABLE: &[ExtensionRule] = &[
    ExtensionRule {
        exts: &["awk"],
        keyword: "awk",
        split: false,
    },
    ExtensionRule {
        exts: &["sh"],
        keyword: "bash",
        split: false,
    },
    ExtensionRule {
        exts: &["bf"],
        keyword: "brainfuck",
        split: false,
    },
    ExtensionRule {
        exts: &["cs"],
        keyword: "c#",
        split: false,
    },
    ExtensionRule {
        exts: &["c"],
        keyword: "c",
        split: true,
    },
    ExtensionRule {
        exts: &["ceylon"],
        keyword: "ceylon",
        split: false,
    },
    ExtensionRule {
        exts: &["clj"],
        keyword: "clojure",
        split: false,
    },
    ExtensionRule {
        exts: &["lisp", "lsp", "cl"],
        keyword: "common lisp",
        split: false,
    },
    ExtensionRule {
        exts: &["cr"],
        keyword: "crystal",
        split: false,
    },
    ExtensionRule {
        exts: &["d"],
        keyword: "d",
        split: true,
    },
    ExtensionRule {
        exts: &["fs"],
        keyword: "f#",
        split: false,
    },
    ExtensionRule {
        exts: &["for", "f", "f90", "f95", "f03"],
        keyword: "fortran",
        split: false,
    },
    ExtensionRule {
        exts: &["go"],
        keyword: "go",
        split: true,
    },
    ExtensionRule {
        exts: &["hs"],
        keyword: "haskell",
        split: false,
    },
    ExtensionRule {
        exts: &["java"],
        keyword: "java",
        split: false,
    },
    ExtensionRule {
        exts: &["js"],
        keyword: "javascript",
        split: false,
    },
    ExtensionRule {
        exts: &["jl"],
        keyword: "julia",
        split: false,
    },
    ExtensionRule {
        exts: &["kt", "kts"],
        keyword: "kotlin",
        split: false,
    },
    ExtensionRule {
        exts: &["lua"],
        keyword: "lua",
        split: false,
    },
    ExtensionRule {
        exts: &["nim"],
        keyword: "nim",
        split: false,
    },
    ExtensionRule {
        exts: &["moon"],
        keyword: "moonscript",
        split: false,
    },
    ExtensionRule {
        exts: &["m"],
        keyword: "objective-c",
        split: false,
    },
    ExtensionRule {
        exts: &["ml"],
        keyword: "ocaml",
        split: false,
    },
    ExtensionRule {
        exts: &["m"],
        keyword: "octave",
        split: false,
    },
    ExtensionRule {
        exts: &["pas"],
        keyword: "pascal",
        split: false,
    },
    ExtensionRule {
        exts: &["p6", "pl6", "pm6"],
        keyword: "perl6",
        split: false,
    },
    ExtensionRule {
        exts: &["pl", "pm"],
        keyword: "perl",
        split: true,
    },
    ExtensionRule {
        exts: &["php"],
        keyword: "php",
        split: false,
    },
    ExtensionRule {
        exts: &["rb"],
        keyword: "ruby",
        split: false,
    },
    ExtensionRule {
        exts: &["rs"],
        keyword: "rust",
        split: false,
    },
    ExtensionRule {
        exts: &["scala"],
        keyword: "scala",
        split: false,
    },
    ExtensionRule {
        exts: &["scm"],
        keyword: "scheme",
        split: false,
    },
    ExtensionRule {
        exts: &["sed"],
        keyword: "sed",
        split: false,
    },
    ExtensionRule {
        exts: &["sml"],
        keyword: "standard ml",
        split: false,
    },
    ExtensionRule {
        exts: &["swift"],
        keyword: "swift",
        split: false,
    },
    ExtensionRule {
        exts: &["txt"],
        keyword: "text",
        split: false,
    },
    ExtensionRule {
        exts: &["ts"],
        keyword: "typescript",
        split: false,
    },
    ExtensionRule {
        exts: &["unl"],
        keyword: "unlambda",
        split: false,
    },
    ExtensionRule {
        exts: &["vim"],
        keyword: "vim script",
        split: false,
    },
    ExtensionRule {
        exts: &["vb"],
        keyword: "visual basic",
        split: false,
    },
];

fn is_cxx_description(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("c++") || lower.contains("g++")
}

fn is_python_description(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("python") || lower.contains("pypy")
}

/// Mirrors upstream `select_ids_of_matched_languages`: with `split=true` the
/// description is tokenized on whitespace and the keyword must equal one of
/// the tokens (case-insensitive); otherwise it's a plain case-insensitive
/// substring match.
fn matches_description(name: &str, keyword: &str, split: bool) -> bool {
    let lower_name = name.to_lowercase();
    let lower_keyword = keyword.to_lowercase();
    if split {
        let tokens: Vec<&str> = lower_name.split_whitespace().collect();
        lower_keyword
            .split_whitespace()
            .all(|w| tokens.contains(&w))
    } else {
        lower_keyword
            .split_whitespace()
            .all(|w| lower_name.contains(w))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::application::ports::SubmissionReceipt;

    fn lang(id: &str, name: &str) -> Language {
        Language {
            id: id.to_string(),
            name: name.to_string(),
        }
    }

    fn cxx_languages() -> Vec<Language> {
        vec![
            lang("4001", "C++ 17 (gcc 12.2)"),
            lang("4002", "C++ 20 (gcc 12.2)"),
            lang("4003", "C++ 23 (gcc 12.2)"),
            lang("4004", "C (gcc 12.2.0)"),
            lang("4005", "Python (CPython 3.11.4)"),
            lang("4006", "Python (PyPy 3.10-v7.3.12)"),
            lang("4007", "Rust (rustc 1.70.0)"),
        ]
    }

    #[test]
    fn exact_id_wins_over_guess() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, Some("4002"), true, Path::new("a.cpp"));
        match r {
            Resolution::Single(l) => assert_eq!(l.id, "4002"),
            _ => panic!("expected single"),
        }
    }

    #[test]
    fn extension_filters_to_cxx_but_remains_ambiguous() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, None, true, Path::new("a.cpp"));
        match r {
            Resolution::Multiple(c) => {
                assert_eq!(c.len(), 3);
                assert!(c.iter().all(|l| l.name.contains("C++")));
            }
            _ => panic!("expected multiple"),
        }
    }

    #[test]
    fn extension_plus_keyword_narrows_to_one() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, Some("20"), true, Path::new("a.cpp"));
        match r {
            Resolution::Single(l) => assert_eq!(l.id, "4002"),
            _ => panic!("expected single"),
        }
    }

    #[test]
    fn c_extension_does_not_match_cxx() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, None, true, Path::new("a.c"));
        match r {
            Resolution::Single(l) => assert_eq!(l.id, "4004"),
            _ => panic!("expected single C match"),
        }
    }

    #[test]
    fn python_extension_matches_both_interpreters() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, None, true, Path::new("a.py"));
        match r {
            Resolution::Multiple(c) => {
                assert_eq!(c.len(), 2);
            }
            _ => panic!("expected multiple"),
        }
    }

    #[test]
    fn python_pypy_keyword_narrows_down() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, Some("PyPy"), true, Path::new("a.py"));
        match r {
            Resolution::Single(l) => assert_eq!(l.id, "4006"),
            _ => panic!("expected single"),
        }
    }

    #[test]
    fn unknown_extension_falls_back_to_full_list() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, Some("Rust"), true, Path::new("a.unknownext"));
        match r {
            Resolution::Single(l) => assert_eq!(l.id, "4007"),
            _ => panic!("expected single"),
        }
    }

    #[test]
    fn no_guess_does_not_filter_by_extension() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, None, false, Path::new("a.cpp"));
        match r {
            Resolution::Multiple(c) => assert_eq!(c.len(), langs.len()),
            _ => panic!("expected all candidates"),
        }
    }

    #[test]
    fn keyword_with_no_match_resolves_to_none() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, Some("Brainfuck"), true, Path::new("a.cpp"));
        assert!(matches!(r, Resolution::None));
    }

    #[test]
    fn matches_keywords_requires_all_terms() {
        assert!(matches_keywords("C++ 20 (gcc 12.2)", "C++ gcc"));
        assert!(!matches_keywords("C++ 20 (gcc 12.2)", "C++ clang"));
    }

    #[test]
    fn matches_description_split_uses_token_equality() {
        // `c` with split=true must equal a token, not just be a substring.
        assert!(matches_description("C (GCC 12.2)", "c", true));
        assert!(!matches_description("Crystal (Crystal 0.33.0)", "c", true));
        assert!(!matches_description("C++ 20 (gcc 12.2)", "c", true));
    }

    #[test]
    fn matches_description_no_split_is_substring() {
        assert!(matches_description(
            "Common Lisp (SBCL 2.0.3)",
            "common lisp",
            false
        ));
        assert!(!matches_description(
            "Lisp (SBCL 2.0.3)",
            "common lisp",
            false
        ));
    }

    #[test]
    fn legacy_uppercase_c_extension_means_cxx() {
        let langs = cxx_languages();
        let r = resolve_language(&langs, None, true, Path::new("a.C"));
        match r {
            Resolution::Multiple(c) => {
                assert_eq!(c.len(), 3);
                assert!(c.iter().all(|l| l.name.contains("C++")));
            }
            _ => panic!("expected multiple C++"),
        }
    }

    #[test]
    fn ruby_extension_matches_ruby_only() {
        let langs = vec![
            lang("4007", "Rust (rustc 1.70.0)"),
            lang("4100", "Ruby (Ruby 3.2.2)"),
            lang("4200", "Crystal (Crystal 1.9.2)"),
        ];
        let r = resolve_language(&langs, None, true, Path::new("a.rb"));
        match r {
            Resolution::Single(l) => assert_eq!(l.id, "4100"),
            _ => panic!("expected single Ruby"),
        }
    }

    #[derive(Default)]
    struct FakeSubmitter {
        languages: Vec<Language>,
        submit_calls: Mutex<Vec<(String, String, Vec<u8>)>>,
        receipt_url: String,
    }

    impl Submitter for FakeSubmitter {
        fn available_languages(&self, _: &str) -> Result<Vec<Language>> {
            Ok(self.languages.clone())
        }
        fn submit(
            &self,
            url: &str,
            language_id: &str,
            source_code: &[u8],
        ) -> Result<SubmissionReceipt> {
            self.submit_calls.lock().unwrap().push((
                url.to_string(),
                language_id.to_string(),
                source_code.to_vec(),
            ));
            Ok(SubmissionReceipt {
                submission_url: self.receipt_url.clone(),
            })
        }
    }

    struct Confirmer(bool);
    impl SubmitConfirmer for Confirmer {
        fn confirm_submit(&self) -> Result<bool> {
            Ok(self.0)
        }
    }

    #[derive(Default)]
    struct RecordingReporter {
        events: Mutex<Vec<String>>,
    }
    impl SubmitReporter for RecordingReporter {
        fn source_loaded(&self, n: usize) {
            self.events.lock().unwrap().push(format!("source:{n}"));
        }
        fn language_chosen(&self, l: &Language) {
            self.events.lock().unwrap().push(format!("lang:{}", l.id));
        }
        fn language_unresolved(&self, c: &[Language]) {
            self.events
                .lock()
                .unwrap()
                .push(format!("unresolved:{}", c.len()));
        }
        fn submission_aborted(&self) {
            self.events.lock().unwrap().push("aborted".into());
        }
        fn submission_succeeded(&self, r: &SubmissionReceipt) {
            self.events
                .lock()
                .unwrap()
                .push(format!("submitted:{}", r.submission_url));
        }
    }

    fn write_temp_source(content: &[u8]) -> PathBuf {
        let dir = tempfile::tempdir().unwrap().keep();
        let path = dir.join("solution.cpp");
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn happy_path_posts_chosen_language_and_source() {
        let submitter = FakeSubmitter {
            languages: cxx_languages(),
            receipt_url: "https://atcoder.jp/contests/abc/submissions/123".into(),
            ..Default::default()
        };
        let confirmer = Confirmer(true);
        let reporter = RecordingReporter::default();
        let file = write_temp_source(b"int main(){}");

        let use_case = Submit {
            submitter: &submitter,
            confirmer: &confirmer,
            reporter: &reporter,
        };
        let out = use_case
            .execute(SubmitInput {
                problem_url: "https://atcoder.jp/contests/abc/tasks/abc_a".into(),
                file,
                language: Some("20".into()),
                guess: true,
                yes: true,
                wait: Duration::ZERO,
            })
            .unwrap();

        assert!(out.submitted);
        let calls = submitter.submit_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "4002");
        assert_eq!(calls[0].2, b"int main(){}");
        let events = reporter.events.lock().unwrap();
        assert_eq!(events[0], "source:12");
        assert_eq!(events[1], "lang:4002");
        assert!(events.last().unwrap().starts_with("submitted:"));
    }

    #[test]
    fn declining_confirmation_skips_post() {
        let submitter = FakeSubmitter {
            languages: cxx_languages(),
            ..Default::default()
        };
        let confirmer = Confirmer(false);
        let reporter = RecordingReporter::default();
        let file = write_temp_source(b"x");

        let use_case = Submit {
            submitter: &submitter,
            confirmer: &confirmer,
            reporter: &reporter,
        };
        let out = use_case
            .execute(SubmitInput {
                problem_url: "u".into(),
                file,
                language: Some("4002".into()),
                guess: true,
                yes: false,
                wait: Duration::ZERO,
            })
            .unwrap();

        assert!(!out.submitted);
        assert!(submitter.submit_calls.lock().unwrap().is_empty());
        assert!(reporter.events.lock().unwrap().contains(&"aborted".into()));
    }

    #[test]
    fn ambiguous_language_bails_after_reporting_candidates() {
        let submitter = FakeSubmitter {
            languages: cxx_languages(),
            ..Default::default()
        };
        let confirmer = Confirmer(true);
        let reporter = RecordingReporter::default();
        let file = write_temp_source(b"x");

        let use_case = Submit {
            submitter: &submitter,
            confirmer: &confirmer,
            reporter: &reporter,
        };
        let err = use_case
            .execute(SubmitInput {
                problem_url: "u".into(),
                file,
                language: None,
                guess: true,
                yes: true,
                wait: Duration::ZERO,
            })
            .unwrap_err();
        assert!(err.to_string().contains("ambiguous"));
        assert!(submitter.submit_calls.lock().unwrap().is_empty());
        let events = reporter.events.lock().unwrap();
        assert!(events.iter().any(|e| e.starts_with("unresolved:")));
    }
}
