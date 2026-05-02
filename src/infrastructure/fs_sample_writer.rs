//! Filesystem-backed implementation of `SampleWriter`. Creates parent
//! directories as needed and writes the bytes verbatim.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::application::ports::SampleWriter;

pub struct FsSampleWriter;

impl FsSampleWriter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsSampleWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl SampleWriter for FsSampleWriter {
    fn write(&self, path: &Path, content: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create directory {}", parent.display()))?;
            }
        }
        fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }
}
