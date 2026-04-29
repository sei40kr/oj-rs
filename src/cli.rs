//! CLI argument types (clap). Lives at the edge of the system: parses raw
//! argv, then `From` impls translate into `application::RunTestsInput`.
//! The application layer never sees clap types.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::application::{DiscoveryQuery, RunTestsInput};
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
