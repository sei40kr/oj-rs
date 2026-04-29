//! Composition root: parses the CLI, instantiates concrete adapters, and
//! drives the use case. This is the only place the layers are wired together.

use anyhow::Result;
use clap::Parser;

mod application;
mod cli;
mod domain;
mod infrastructure;

use crate::application::RunTests;
use crate::cli::{Cli, Command};
use crate::infrastructure::{
    ConsoleReporter, FsTestCaseRepository, ShellJudgeRunner, ShellSolutionExecutor,
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
    }
}

fn run_test_command(args: cli::TestArgs) -> Result<i32> {
    let input = args.into_use_case_input();

    let repository = FsTestCaseRepository::new();
    let executor = ShellSolutionExecutor::new();
    let judge = ShellJudgeRunner::new();
    let reporter = ConsoleReporter::new();

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
