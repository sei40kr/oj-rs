//! Filesystem-backed implementation of `TestCaseRepository`.
//!
//! Translates the upstream `%s.%e` format into a glob (for finding files) and
//! a regex (for extracting the case name and `in`/`out` extension to pair
//! files together).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use regex::Regex;

use crate::application::ports::{DiscoveryQuery, TestCaseRepository};
use crate::domain::TestCase;

pub struct FsTestCaseRepository;

impl FsTestCaseRepository {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsTestCaseRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCaseRepository for FsTestCaseRepository {
    fn discover(&self, query: &DiscoveryQuery) -> Result<Vec<TestCase>> {
        if !query.format.contains("%s") || !query.format.contains("%e") {
            return Err(anyhow!("--format must contain both %s and %e"));
        }

        let mut candidates: Vec<PathBuf> = if query.explicit_paths.is_empty() {
            glob_with_format(&query.directory, &query.format)?
        } else {
            query.explicit_paths.clone()
        };

        if query.ignore_backup {
            candidates.retain(|p| !is_backup_file(p));
        }

        let pair_pattern = build_pair_regex(&query.directory, &query.format)?;

        let mut by_name: BTreeMap<String, (Option<PathBuf>, Option<PathBuf>)> = BTreeMap::new();
        for path in candidates {
            let path_str = path.to_string_lossy();
            let Some(captures) = pair_pattern.captures(&path_str) else {
                continue;
            };
            let name = captures.name("s").map(|m| m.as_str().to_string());
            let extension = captures.name("e").map(|m| m.as_str().to_string());
            let (Some(name), Some(extension)) = (name, extension) else {
                continue;
            };
            let entry = by_name.entry(name).or_default();
            match extension.as_str() {
                "in" => entry.0 = Some(path.clone()),
                "out" => entry.1 = Some(path.clone()),
                _ => {}
            }
        }

        Ok(by_name
            .into_iter()
            .filter_map(|(name, (input, expected))| {
                input.map(|input| TestCase {
                    name,
                    input,
                    expected,
                })
            })
            .collect())
    }
}

fn glob_with_format(directory: &Path, format: &str) -> Result<Vec<PathBuf>> {
    let pattern = format.replace("%s", "*").replace("%e", "*");
    let directory_str = directory.to_string_lossy();
    let full_pattern = if directory_str.is_empty() {
        pattern
    } else {
        format!("{}/{}", directory_str.trim_end_matches('/'), pattern)
    };
    let mut matches = Vec::new();
    for entry in glob::glob(&full_pattern)
        .with_context(|| format!("invalid glob pattern: {full_pattern}"))?
    {
        match entry {
            Ok(path) => matches.push(path),
            Err(e) => eprintln!("[!] glob error: {e}"),
        }
    }
    Ok(matches)
}

fn build_pair_regex(directory: &Path, format: &str) -> Result<Regex> {
    let directory_str = directory.to_string_lossy();
    let full = if directory_str.is_empty() {
        format.to_string()
    } else {
        format!("{}/{}", directory_str.trim_end_matches('/'), format)
    };

    let placeholder = Regex::new(r"%[se]")?;
    let mut pattern = String::from("^");
    let mut last_end = 0usize;
    for m in placeholder.find_iter(&full) {
        pattern.push_str(&regex::escape(&full[last_end..m.start()]));
        match m.as_str() {
            "%s" => pattern.push_str("(?P<s>.+)"),
            "%e" => pattern.push_str("(?P<e>in|out)"),
            _ => unreachable!(),
        }
        last_end = m.end();
    }
    pattern.push_str(&regex::escape(&full[last_end..]));
    pattern.push('$');
    Ok(Regex::new(&pattern)?)
}

fn is_backup_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    name.starts_with('.')
        || name.starts_with('#')
        || name.ends_with('~')
        || (name.starts_with('#') && name.ends_with('#'))
}
