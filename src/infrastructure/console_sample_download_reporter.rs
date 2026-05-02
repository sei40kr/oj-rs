//! Console implementation of `SampleDownloadReporter`. Output mirrors upstream
//! `oj download` so existing scripts that grep its lines keep working.

use std::path::Path;
use std::sync::Mutex;

use crate::application::ports::SampleDownloadReporter;
use crate::domain::Sample;

pub struct ConsoleSampleDownloadReporter {
    silent: bool,
    print_lock: Mutex<()>,
}

impl ConsoleSampleDownloadReporter {
    pub fn new(silent: bool) -> Self {
        Self {
            silent,
            print_lock: Mutex::new(()),
        }
    }
}

impl SampleDownloadReporter for ConsoleSampleDownloadReporter {
    fn samples_found(&self, count: usize) {
        if self.silent {
            return;
        }
        let _guard = self.print_lock.lock().unwrap();
        println!("[*] {count} samples found");
    }

    fn sample_written(&self, sample: &Sample, input_path: &Path, output_path: Option<&Path>) {
        if self.silent {
            return;
        }
        let _guard = self.print_lock.lock().unwrap();
        println!("[+] {}: input  -> {}", sample.name, input_path.display());
        if let Some(path) = output_path {
            println!("[+] {}: output -> {}", sample.name, path.display());
        }
    }

    fn dry_run_sample(&self, sample: &Sample) {
        let _guard = self.print_lock.lock().unwrap();
        println!("[*] {}", sample.name);
        print_block("input", &sample.input);
        if let Some(output) = &sample.output {
            print_block("output", output);
        }
    }

    fn no_samples_found(&self) {
        let _guard = self.print_lock.lock().unwrap();
        eprintln!("[!] no samples found on the page");
    }
}

fn print_block(label: &str, data: &[u8]) {
    println!("[*] {label}:");
    let text = String::from_utf8_lossy(data);
    for line in text.lines() {
        println!("    {line}");
    }
    if !data.is_empty() && !data.ends_with(b"\n") {
        println!("    [no trailing newline]");
    }
}
