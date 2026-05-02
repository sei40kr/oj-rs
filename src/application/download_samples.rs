//! Use case: fetch sample test cases for a problem URL and write them to disk.
//! Holds no I/O — depends only on ports.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::Sample;

use super::ports::{ProblemDownloader, SampleDownloadReporter, SampleWriter};

#[derive(Debug, Clone)]
pub struct DownloadSamplesInput {
    pub url: String,
    pub directory: PathBuf,
    pub format: String,
    pub dry_run: bool,
}

#[derive(Debug)]
pub struct DownloadSamplesOutput {
    pub samples_found: usize,
}

impl DownloadSamplesOutput {
    /// True when at least one sample was found. Used by `main` to pick the
    /// process exit code (matches upstream `oj download`'s "no samples = error").
    pub fn any_found(&self) -> bool {
        self.samples_found > 0
    }
}

pub struct DownloadSamples<'a> {
    pub downloader: &'a dyn ProblemDownloader,
    pub writer: &'a dyn SampleWriter,
    pub reporter: &'a dyn SampleDownloadReporter,
}

impl<'a> DownloadSamples<'a> {
    pub fn execute(&self, input: DownloadSamplesInput) -> Result<DownloadSamplesOutput> {
        let samples = self.downloader.download(&input.url)?;

        if samples.is_empty() {
            self.reporter.no_samples_found();
            return Ok(DownloadSamplesOutput { samples_found: 0 });
        }
        self.reporter.samples_found(samples.len());

        if input.dry_run {
            for sample in &samples {
                self.reporter.dry_run_sample(sample);
            }
            return Ok(DownloadSamplesOutput {
                samples_found: samples.len(),
            });
        }

        for (index, sample) in samples.iter().enumerate() {
            let input_path = resolve_path(&input.directory, &input.format, sample, index, "in");
            let output_path = sample
                .output
                .as_ref()
                .map(|_| resolve_path(&input.directory, &input.format, sample, index, "out"));

            // Match upstream: refuse to clobber existing files.
            if input_path.exists() {
                bail!("file already exists: {}", input_path.display());
            }
            if let Some(p) = &output_path {
                if p.exists() {
                    bail!("file already exists: {}", p.display());
                }
            }

            self.writer
                .write(&input_path, &sample.input)
                .with_context(|| format!("failed to write {}", input_path.display()))?;
            if let (Some(path), Some(content)) = (&output_path, &sample.output) {
                self.writer
                    .write(path, content)
                    .with_context(|| format!("failed to write {}", path.display()))?;
            }

            self.reporter
                .sample_written(sample, &input_path, output_path.as_deref());
        }

        Ok(DownloadSamplesOutput {
            samples_found: samples.len(),
        })
    }
}

fn resolve_path(
    directory: &Path,
    format: &str,
    sample: &Sample,
    index: usize,
    extension: &str,
) -> PathBuf {
    let name = &sample.name;
    let basename = Path::new(name)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.clone());
    let dirname = Path::new(name)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Substitute longest placeholders first to avoid `%s` matching inside `%system`-like
    // strings. Today all placeholders are two characters, so order is moot — but keep
    // this stable in case we add more.
    let filename = format
        .replace("%b", &basename)
        .replace("%e", extension)
        .replace("%i", &(index + 1).to_string())
        .replace("%n", name)
        .replace("%d", &dirname)
        .replace("%s", name);

    directory.join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> Sample {
        Sample {
            name: name.to_string(),
            input: Vec::new(),
            output: None,
        }
    }

    #[test]
    fn resolve_path_default_format() {
        let path = resolve_path(Path::new("test/"), "%b.%e", &sample("sample-2"), 1, "in");
        assert_eq!(path, PathBuf::from("test/sample-2.in"));
    }

    #[test]
    fn resolve_path_index_placeholder_is_one_based() {
        let path = resolve_path(Path::new("t"), "%i.%e", &sample("anything"), 0, "out");
        assert_eq!(path, PathBuf::from("t/1.out"));
    }

    #[test]
    fn resolve_path_basename_strips_dirs() {
        let path = resolve_path(Path::new("t"), "%b.%e", &sample("sub/sample-1"), 0, "in");
        assert_eq!(path, PathBuf::from("t/sample-1.in"));
    }
}
