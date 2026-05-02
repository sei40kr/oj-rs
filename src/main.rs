//! Composition root: parses the CLI, instantiates concrete adapters, and
//! drives the use case. This is the only place the layers are wired together.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;

mod application;
mod cli;
mod domain;
mod infrastructure;

use crate::application::{DownloadSamples, Login, RunTests, Submit};
use crate::cli::{Cli, Command};
use crate::infrastructure::{
    AtCoderAuthenticator, AtCoderDownloader, AtCoderSubmitter, ConsoleLoginReporter,
    ConsoleSampleDownloadReporter, ConsoleSubmitReporter, ConsoleTestRunReporter, FsSampleWriter,
    FsTestCaseRepository, ShellJudgeRunner, ShellSolutionExecutor, StdinCredentialPrompt,
    StdinSubmitConfirmer,
};

fn main() {
    let cli = Cli::parse();
    let exit_code = match dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("[!] error: {e:#}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn dispatch(cli: Cli) -> Result<i32> {
    match cli.command {
        Command::Test(args) => run_test_command(args),
        Command::Download(args) => run_download_command(args),
        Command::Login(args) => run_login_command(args),
        Command::Submit(args) => run_submit_command(args),
    }
}

fn run_test_command(args: cli::TestArgs) -> Result<i32> {
    let input = args.into_use_case_input();

    let repository = FsTestCaseRepository::new();
    let executor = ShellSolutionExecutor::new();
    let judge = ShellJudgeRunner::new();
    let reporter = ConsoleTestRunReporter::new();

    let use_case = RunTests {
        repository: &repository,
        executor: &executor,
        judge: &judge,
        reporter: &reporter,
    };
    let output = use_case.execute(input)?;
    Ok(if output.outcomes.is_empty() || output.all_accepted() {
        0
    } else {
        1
    })
}

fn run_download_command(args: cli::DownloadArgs) -> Result<i32> {
    if args.system {
        bail!("--system is not yet implemented");
    }

    let service = service_from_url(&args.url)?;
    let downloader = downloader_for(service);

    let writer = FsSampleWriter::new();
    let reporter = ConsoleSampleDownloadReporter::new(args.silent);

    let use_case = DownloadSamples {
        downloader: downloader.as_ref(),
        writer: &writer,
        reporter: &reporter,
    };
    let output = use_case.execute(args.into_use_case_input())?;
    Ok(if output.any_found() { 0 } else { 1 })
}

fn run_login_command(args: cli::LoginArgs) -> Result<i32> {
    let service = service_from_url(&args.url)?;
    let cookie_path = match args.cookie.as_ref() {
        Some(path) => path.clone(),
        None => default_cookie_path()?,
    };

    let authenticator = authenticator_for(service, cookie_path)?;
    let prompt = StdinCredentialPrompt::new();
    let reporter = ConsoleLoginReporter::new();

    let use_case = Login {
        authenticator: authenticator.as_ref(),
        prompt: &prompt,
        reporter: &reporter,
    };
    let output = use_case.execute(args.into_use_case_input())?;
    Ok(if output.logged_in { 0 } else { 1 })
}

fn run_submit_command(args: cli::SubmitArgs) -> Result<i32> {
    let service = service_from_url(&args.url)?;
    let cookie_path = match args.cookie.as_ref() {
        Some(path) => path.clone(),
        None => default_cookie_path()?,
    };

    let submitter = submitter_for(service, cookie_path)?;
    let confirmer = StdinSubmitConfirmer::new();
    let reporter = ConsoleSubmitReporter::new();

    let use_case = Submit {
        submitter: submitter.as_ref(),
        confirmer: &confirmer,
        reporter: &reporter,
    };
    let output = use_case.execute(args.into_use_case_input())?;
    Ok(if output.submitted { 0 } else { 1 })
}

/// Online judges supported by this binary. Both downloaders and authenticators
/// dispatch on this enum so a new judge needs to be wired up in one place.
#[derive(Copy, Clone, Debug)]
enum Service {
    AtCoder,
}

fn service_from_url(url: &str) -> Result<Service> {
    let parsed = url::Url::parse(url).context("invalid URL")?;
    let host = parsed
        .host_str()
        .context("URL has no host")?
        .trim_start_matches("www.");
    match host {
        "atcoder.jp" | "beta.atcoder.jp" => Ok(Service::AtCoder),
        other => bail!("unsupported judge: {other}"),
    }
}

fn downloader_for(service: Service) -> Box<dyn application::ports::ProblemDownloader> {
    match service {
        Service::AtCoder => Box::new(AtCoderDownloader::new()),
    }
}

fn authenticator_for(
    service: Service,
    cookie_path: PathBuf,
) -> Result<Box<dyn application::ports::Authenticator>> {
    match service {
        Service::AtCoder => Ok(Box::new(AtCoderAuthenticator::new(cookie_path)?)),
    }
}

fn submitter_for(
    service: Service,
    cookie_path: PathBuf,
) -> Result<Box<dyn application::ports::Submitter>> {
    match service {
        Service::AtCoder => Ok(Box::new(AtCoderSubmitter::new(cookie_path)?)),
    }
}

/// Per-platform default. Prefers `dirs::state_dir()` (XDG_STATE_HOME →
/// `~/.local/state` on Linux) since session cookies are recoverable state
/// rather than persistent application data. Falls back to `data_dir()` on
/// platforms where `state_dir()` is not defined (macOS, Windows).
fn default_cookie_path() -> Result<PathBuf> {
    let base = dirs::state_dir()
        .or_else(dirs::data_dir)
        .ok_or_else(|| anyhow!("could not locate a user state or data dir"))?;
    Ok(base.join("oj-rs").join("cookie.json"))
}
