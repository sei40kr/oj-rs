//! Composition root: parses the CLI, instantiates concrete adapters, and
//! drives the use case. This is the only place the layers are wired together.

use anyhow::{Context, Result, bail};
use clap::Parser;

mod application;
mod cli;
mod domain;
mod infrastructure;

use crate::application::{DownloadSamples, RunTests};
use crate::cli::{Cli, Command};
use crate::infrastructure::{
    AtCoderDownloader, ConsoleSampleDownloadReporter, ConsoleTestRunReporter, FsSampleWriter,
    FsTestCaseRepository, ShellJudgeRunner, ShellSolutionExecutor,
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

    let parsed = url::Url::parse(&args.url).context("invalid URL")?;
    let host = parsed.host_str().context("URL has no host")?;
    let downloader = pick_downloader(host)?;

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

fn pick_downloader(host: &str) -> Result<Box<dyn application::ports::ProblemDownloader>> {
    let host = host.trim_start_matches("www.");
    match host {
        "atcoder.jp" | "beta.atcoder.jp" => Ok(Box::new(AtCoderDownloader::new())),
        other => bail!("unsupported judge: {other}"),
    }
}
