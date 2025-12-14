//! Benchmark for loading TDF fonts from the massive ROYS collection.
//!
//! This benchmark:
//! 1. Extracts the ZIP archive to a temp directory
//! 2. Loads all TDF files using the unified Font API
//! 3. Cleans up the temp directory

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use retrofont::Font;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

/// Get the workspace root directory
fn workspace_root() -> PathBuf {
    // The benchmark runs from workspace root when using cargo bench
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn zip_path() -> PathBuf {
    workspace_root().join("benches/data/ROYS-THEDRAW_TDF_FONTS_COLLECTION.ZIP")
}

fn tmp_dir() -> PathBuf {
    workspace_root().join("target/tmp/bench_tdf_fonts")
}

/// Extract the ZIP file to a temporary directory and return the path
fn extract_zip() -> PathBuf {
    let tmp = tmp_dir();
    let zip = zip_path();

    // Clean up if exists from previous run
    if tmp.exists() {
        fs::remove_dir_all(&tmp).expect("Failed to clean up existing temp dir");
    }

    fs::create_dir_all(&tmp).expect("Failed to create temp directory");

    // Use unzip command to extract
    let status = Command::new("unzip")
        .arg("-q") // quiet mode
        .arg("-o") // overwrite without prompting
        .arg(&zip)
        .arg("-d")
        .arg(&tmp)
        .status()
        .expect("Failed to execute unzip command");

    assert!(
        status.success(),
        "unzip command failed - make sure {} exists",
        zip.display()
    );

    tmp
}

/// Clean up the temporary directory
fn cleanup(dir: &PathBuf) {
    if dir.exists() {
        fs::remove_dir_all(dir).expect("Failed to clean up temp dir");
    }
}

/// Collect all .tdf files recursively from a directory
fn collect_tdf_files(dir: &PathBuf) -> Vec<PathBuf> {
    let mut tdf_files = Vec::new();

    fn visit_dir(dir: &PathBuf, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit_dir(&path, files);
                } else if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("tdf") {
                        files.push(path);
                    }
                }
            }
        }
    }

    visit_dir(dir, &mut tdf_files);
    tdf_files
}

/// Load all TDF files and count the fonts loaded
fn load_all_tdf_files(files: &[PathBuf]) -> (usize, usize) {
    let mut total_files = 0;
    let mut total_fonts = 0;

    for path in files {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(fonts) = Font::load_owned(bytes) {
                total_files += 1;
                total_fonts += fonts.len();
            }
        }
    }

    (total_files, total_fonts)
}

fn bench_tdf_loading(c: &mut Criterion) {
    // Setup: extract ZIP once before benchmarks
    let tmp_dir = extract_zip();
    let tdf_files = collect_tdf_files(&tmp_dir);

    println!("\n=== TDF Font Loading Benchmark ===");
    println!("Found {} TDF files in archive", tdf_files.len());

    // Pre-load file contents to benchmark just the parsing (Arc to avoid copies per iteration)
    let file_contents: Vec<Arc<[u8]>> = tdf_files
        .iter()
        .filter_map(|path| fs::read(path).ok())
        .map(Arc::<[u8]>::from)
        .collect();

    println!(
        "Loaded {} files into memory ({:.2} MB total)",
        file_contents.len(),
        file_contents.iter().map(|v| v.len()).sum::<usize>() as f64 / (1024.0 * 1024.0)
    );

    // Benchmark: Parse all TDF files from memory
    c.bench_function("parse_all_tdf_fonts", |b| {
        b.iter(|| {
            let mut total_fonts = 0usize;
            for bytes in &file_contents {
                if let Ok(fonts) = Font::load_arc(black_box(bytes.clone())) {
                    total_fonts += fonts.len();
                }
            }
            black_box(total_fonts)
        })
    });

    // Benchmark: Single file parse (average case)
    if let Some(sample) = file_contents.first() {
        c.bench_function("parse_single_tdf_file", |b| {
            b.iter(|| black_box(Font::load_arc(black_box(sample.clone()))))
        });
    }

    // Benchmark: Full I/O + parsing
    c.bench_function("load_all_tdf_from_disk", |b| {
        b.iter(|| black_box(load_all_tdf_files(&tdf_files)))
    });

    // Print summary
    let (files_loaded, fonts_loaded) = load_all_tdf_files(&tdf_files);
    println!(
        "\nSummary: Loaded {} fonts from {} TDF files",
        fonts_loaded, files_loaded
    );

    // Cleanup
    cleanup(&tmp_dir);
}

criterion_group!(benches, bench_tdf_loading);
criterion_main!(benches);
