//! CLI argument types (clap). Lives at the edge of the system: parses raw
//! argv, then `From` impls translate into `application::RunTestsInput`.
//! The application layer never sees clap types.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

use crate::application::{
    DiscoveryQuery, DownloadSamplesInput, LoginInput, RunTestsInput, SubmitInput,
};
use crate::domain::{CompareMode as DomainCompareMode, DisplayMode as DomainDisplayMode};

#[derive(Parser)]
#[command(name = "oj", version, about = "Online Judge tools (Rust port)")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(alias = "t", about = "Run tests against sample cases")]
    Test(TestArgs),

    #[command(
        aliases = ["d", "dl"],
        about = "Download sample test cases from a problem URL"
    )]
    Download(DownloadArgs),

    #[command(about = "Sign in to an online judge")]
    Login(LoginArgs),

    #[command(alias = "s", about = "Submit a source file to an online judge")]
    Submit(SubmitArgs),

    #[command(about = "Generate shell completion script")]
    Completion(CompletionArgs),
}

#[derive(Args, Debug)]
pub struct CompletionArgs {
    /// Target shell.
    #[arg(value_name = "SHELL")]
    pub shell: Shell,
}

#[derive(Args, Debug)]
pub struct TestArgs {
    /// Solution command to execute.
    #[arg(short = 'c', long = "command", default_value = "./a.out")]
    pub command: String,

    /// Format of the file name. `%s` matches the case name, `%e` matches `in`/`out`.
    #[arg(short = 'f', long = "format", default_value = "%s.%e")]
    pub format: String,

    /// Directory containing test cases.
    #[arg(short = 'd', long = "directory", default_value = "test/")]
    pub directory: PathBuf,

    /// Output comparison mode.
    #[arg(short = 'm', long = "compare-mode", value_enum, default_value_t = CliCompareMode::CrlfInsensitiveExactMatch)]
    pub compare_mode: CliCompareMode,

    /// Display mode.
    #[arg(short = 'M', long = "display-mode", value_enum, default_value_t = CliDisplayMode::Summary)]
    pub display_mode: CliDisplayMode,

    /// Alias for `--compare-mode=ignore-spaces`.
    #[arg(short = 'S', long = "ignore-spaces", conflicts_with_all = ["ignore_spaces_and_newlines", "compare_mode"])]
    pub ignore_spaces: bool,

    /// Alias for `--compare-mode=ignore-spaces-and-newlines`.
    #[arg(short = 'N', long = "ignore-spaces-and-newlines", conflicts_with_all = ["ignore_spaces", "compare_mode"])]
    pub ignore_spaces_and_newlines: bool,

    /// Alias for `--display-mode=diff`.
    #[arg(short = 'D', long = "diff", conflicts_with = "display_mode")]
    pub diff: bool,

    /// Suppress input/output printing on failures.
    #[arg(short = 's', long = "silent")]
    pub silent: bool,

    /// Floating-point tolerance (relative and absolute).
    #[arg(short = 'e', long = "error")]
    pub error: Option<f64>,

    /// Time limit in seconds (per case).
    #[arg(short = 't', long = "tle")]
    pub tle: Option<f64>,

    /// Number of parallel workers.
    #[arg(short = 'j', long = "jobs")]
    pub jobs: Option<usize>,

    /// Print input on failure (default: true).
    #[arg(short = 'i', long = "print-input", default_value_t = true, action = clap::ArgAction::Set)]
    pub print_input: bool,

    /// Skip backup files (`*~`, `#*#`, `.*`). Default: true.
    #[arg(long = "ignore-backup", default_value_t = true, action = clap::ArgAction::Set)]
    pub ignore_backup: bool,

    /// Special-judge command: `judge_cmd INPUT ACTUAL EXPECTED`. Exit 0 = AC.
    #[arg(long = "judge-command")]
    pub judge_command: Option<String>,

    /// Explicit test case paths. When omitted, all matching files in `--directory` are used.
    #[arg(value_name = "TEST")]
    pub paths: Vec<PathBuf>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum CliCompareMode {
    #[value(name = "exact-match")]
    ExactMatch,
    #[value(name = "crlf-insensitive-exact-match")]
    CrlfInsensitiveExactMatch,
    #[value(name = "ignore-spaces")]
    IgnoreSpaces,
    #[value(name = "ignore-spaces-and-newlines")]
    IgnoreSpacesAndNewlines,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum CliDisplayMode {
    #[value(name = "summary")]
    Summary,
    #[value(name = "all")]
    All,
    #[value(name = "diff")]
    Diff,
    #[value(name = "diff-all")]
    DiffAll,
}

impl From<CliCompareMode> for DomainCompareMode {
    fn from(m: CliCompareMode) -> Self {
        match m {
            CliCompareMode::ExactMatch => DomainCompareMode::ExactMatch,
            CliCompareMode::CrlfInsensitiveExactMatch => {
                DomainCompareMode::CrlfInsensitiveExactMatch
            }
            CliCompareMode::IgnoreSpaces => DomainCompareMode::IgnoreSpaces,
            CliCompareMode::IgnoreSpacesAndNewlines => DomainCompareMode::IgnoreSpacesAndNewlines,
        }
    }
}

impl From<CliDisplayMode> for DomainDisplayMode {
    fn from(m: CliDisplayMode) -> Self {
        match m {
            CliDisplayMode::Summary => DomainDisplayMode::Summary,
            CliDisplayMode::All => DomainDisplayMode::All,
            CliDisplayMode::Diff => DomainDisplayMode::Diff,
            CliDisplayMode::DiffAll => DomainDisplayMode::DiffAll,
        }
    }
}

#[derive(Args, Debug)]
pub struct DownloadArgs {
    /// Problem URL.
    #[arg(value_name = "URL")]
    pub url: String,

    /// Format string for the output filenames. `%b` = sample name, `%e` = `in`/`out`,
    /// `%i` = sample index (1-based), `%n` = full sample name, `%d` = sample dirname.
    #[arg(short = 'f', long = "format", default_value = "%b.%e")]
    pub format: String,

    /// Directory to write test cases into.
    #[arg(short = 'd', long = "directory", default_value = "test/")]
    pub directory: PathBuf,

    /// Print samples to stdout instead of writing files.
    #[arg(short = 'n', long = "dry-run")]
    pub dry_run: bool,

    /// Download system tests (requires login). Not yet implemented.
    #[arg(short = 'a', long = "system")]
    pub system: bool,

    /// Suppress per-sample output.
    #[arg(short = 's', long = "silent")]
    pub silent: bool,
}

#[derive(Args, Debug)]
pub struct LoginArgs {
    /// Service URL (e.g., `https://atcoder.jp/`).
    #[arg(value_name = "URL")]
    pub url: String,

    /// Username (prompted if omitted).
    #[arg(short = 'u', long = "username")]
    pub username: Option<String>,

    /// Password (prompted if omitted; passing on the command line is insecure).
    #[arg(short = 'p', long = "password")]
    pub password: Option<String>,

    /// Check whether the persisted session is still valid, then exit.
    #[arg(long = "check")]
    pub check: bool,

    /// Path to the cookie jar file.
    #[arg(long = "cookie", value_name = "PATH")]
    pub cookie: Option<PathBuf>,
}

impl LoginArgs {
    pub fn into_use_case_input(self) -> LoginInput {
        LoginInput {
            username: self.username,
            password: self.password,
            check_only: self.check,
        }
    }
}

#[derive(Args, Debug)]
pub struct SubmitArgs {
    /// Problem URL (e.g., `https://atcoder.jp/contests/abc123/tasks/abc123_a`).
    #[arg(value_name = "URL")]
    pub url: String,

    /// Source file to submit.
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// Language ID, or one or more substring keywords matched against
    /// language names (e.g., `-l "C++ 20"`). When omitted, guessed from the
    /// file extension; pass `--no-guess` to disable.
    #[arg(short = 'l', long = "language")]
    pub language: Option<String>,

    /// Disable file-extension based language guessing.
    #[arg(long = "no-guess")]
    pub no_guess: bool,

    /// Skip the confirmation prompt.
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Total wait in seconds, split before and after the prompt.
    #[arg(
        short = 'w',
        long = "wait",
        value_name = "SECOND",
        default_value_t = 3.0
    )]
    pub wait: f64,

    /// Path to the cookie jar file.
    #[arg(long = "cookie", value_name = "PATH")]
    pub cookie: Option<PathBuf>,
}

impl SubmitArgs {
    pub fn into_use_case_input(self) -> SubmitInput {
        SubmitInput {
            problem_url: self.url,
            file: self.file,
            language: self.language,
            guess: !self.no_guess,
            yes: self.yes,
            wait: Duration::from_secs_f64(self.wait.max(0.0)),
        }
    }
}

impl DownloadArgs {
    pub fn into_use_case_input(self) -> DownloadSamplesInput {
        DownloadSamplesInput {
            url: self.url,
            directory: self.directory,
            format: self.format,
            dry_run: self.dry_run,
        }
    }
}

impl TestArgs {
    pub fn into_use_case_input(self) -> RunTestsInput {
        let compare_mode = if self.ignore_spaces {
            DomainCompareMode::IgnoreSpaces
        } else if self.ignore_spaces_and_newlines {
            DomainCompareMode::IgnoreSpacesAndNewlines
        } else {
            self.compare_mode.into()
        };
        let display_mode = if self.diff {
            DomainDisplayMode::Diff
        } else {
            self.display_mode.into()
        };

        RunTestsInput {
            command: self.command,
            discovery: DiscoveryQuery {
                directory: self.directory,
                format: self.format,
                explicit_paths: self.paths,
                ignore_backup: self.ignore_backup,
            },
            compare_mode,
            display_mode,
            error_tolerance: self.error,
            time_limit: self.tle.map(Duration::from_secs_f64),
            workers: self.jobs.unwrap_or(1).max(1),
            silent: self.silent,
            print_input: self.print_input,
            judge_command: self.judge_command,
        }
    }
}
