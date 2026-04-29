//! Shell-based implementation of `SolutionExecutor`.
//!
//! Spawns the user command via `sh -c` (Unix) or `cmd /C` (Windows) inside a
//! process group (Unix) / Job Object (Windows) so that on TLE we can kill the
//! entire process tree. Without this, descendants like `sleep` keep the stdout
//! pipe open and the reader thread blocks until they finish naturally.
//!
//! Group semantics are provided by the `command-group` crate.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use command_group::CommandGroup;

use crate::application::ports::{ExecutionRequest, ExecutionResult, SolutionExecutor};

pub struct ShellSolutionExecutor;

impl ShellSolutionExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShellSolutionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SolutionExecutor for ShellSolutionExecutor {
    fn execute(&self, request: ExecutionRequest<'_>) -> Result<ExecutionResult> {
        let mut command = build_shell_command(request.command);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let started_at = Instant::now();
        let mut child = command
            .group_spawn()
            .with_context(|| format!("failed to spawn: {}", request.command))?;

        let mut stdin = child.inner().stdin.take().expect("stdin piped");
        let stdin_payload = request.stdin.to_vec();
        let stdin_writer = thread::spawn(move || {
            let _ = stdin.write_all(&stdin_payload);
        });

        let mut stdout = child.inner().stdout.take().expect("stdout piped");
        let stdout_reader = thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stdout.read_to_end(&mut buffer);
            buffer
        });

        let poll_interval = Duration::from_millis(20);
        loop {
            match child.try_wait()? {
                Some(status) => {
                    let _ = stdin_writer.join();
                    let stdout_bytes = stdout_reader.join().unwrap_or_default();
                    return Ok(ExecutionResult {
                        stdout: stdout_bytes,
                        elapsed: started_at.elapsed(),
                        exit_code: status.code(),
                        timed_out: false,
                    });
                }
                None => {
                    if let Some(limit) = request.time_limit {
                        if started_at.elapsed() >= limit {
                            let _ = child.kill();
                            let _ = child.wait();
                            let _ = stdin_writer.join();
                            let stdout_bytes = stdout_reader.join().unwrap_or_default();
                            return Ok(ExecutionResult {
                                stdout: stdout_bytes,
                                elapsed: started_at.elapsed(),
                                exit_code: None,
                                timed_out: true,
                            });
                        }
                    }
                    thread::sleep(poll_interval);
                }
            }
        }
    }
}

#[cfg(unix)]
fn build_shell_command(command: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(command);
    c
}

#[cfg(windows)]
fn build_shell_command(command: &str) -> Command {
    let mut c = Command::new("cmd");
    c.arg("/C").arg(command);
    c
}
