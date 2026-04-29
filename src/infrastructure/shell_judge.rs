//! Shell-based implementation of `JudgeRunner` for the `--judge-command`
//! special-judge protocol: `cmd INPUT_PATH ACTUAL_PATH EXPECTED_PATH`.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use tempfile::NamedTempFile;

use crate::application::ports::JudgeRunner;

pub struct ShellJudgeRunner;

impl ShellJudgeRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShellJudgeRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl JudgeRunner for ShellJudgeRunner {
    fn judge(
        &self,
        command: &str,
        input_path: &Path,
        actual: &[u8],
        expected_path: Option<&Path>,
    ) -> Result<bool> {
        let mut actual_file = NamedTempFile::with_prefix("ojrs-actual-")
            .context("failed to create temp file for actual output")?;
        std::io::Write::write_all(actual_file.as_file_mut(), actual)
            .context("failed to write actual to temp file")?;

        let abs_input = input_path
            .canonicalize()
            .unwrap_or_else(|_| input_path.to_path_buf());
        let abs_actual = actual_file
            .path()
            .canonicalize()
            .unwrap_or_else(|_| actual_file.path().to_path_buf());
        let abs_expected = expected_path
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let cmdline = format!(
            "{command} {} {} {}",
            shlex::try_quote(&abs_input.to_string_lossy())?,
            shlex::try_quote(&abs_actual.to_string_lossy())?,
            shlex::try_quote(&abs_expected)?,
        );

        let status = build_shell_command(&cmdline).status()?;
        Ok(status.success())
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
