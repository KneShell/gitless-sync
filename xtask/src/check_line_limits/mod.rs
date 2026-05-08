use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_LIMIT: usize = 300;
pub(crate) const DEFAULT_SCAN_ROOTS: &[&str] = &["crates/gitless-sync/src", "xtask/src"];

const DOC_HEAVY_NUMERATOR: usize = 1;
const DOC_HEAVY_DENOMINATOR: usize = 2;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FileMetrics {
    pub(crate) path: PathBuf,
    pub(crate) total_lines: usize,
    pub(crate) doc_lines: usize,
}

impl FileMetrics {
    pub(crate) fn doc_pct(&self) -> usize {
        self.doc_lines
            .saturating_mul(100)
            .checked_div(self.total_lines)
            .unwrap_or(0)
    }

    pub(crate) fn is_doc_heavy(&self) -> bool {
        self.total_lines > 0
            && self.doc_lines * DOC_HEAVY_DENOMINATOR >= self.total_lines * DOC_HEAVY_NUMERATOR
    }
}

pub(crate) fn measure_content(path: PathBuf, content: &str) -> FileMetrics {
    let total_lines = content.lines().count();
    let doc_lines = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("///") || trimmed.starts_with("//!")
        })
        .count();
    FileMetrics {
        path,
        total_lines,
        doc_lines,
    }
}

pub(crate) fn collect_rust_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(root, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

pub(crate) fn run(scan_root: &Path, limit: usize) -> std::io::Result<u8> {
    let files = collect_rust_files(scan_root)?;
    let mut violations = 0_usize;
    let mut exemptions = 0_usize;

    println!(
        "Checking LOC limit ({limit} max) in {}",
        scan_root.display()
    );

    for path in &files {
        let content = fs::read_to_string(path)?;
        let metrics = measure_content(path.clone(), &content);

        if metrics.total_lines > limit {
            let display_path = path.strip_prefix(scan_root).unwrap_or(path);
            if metrics.is_doc_heavy() {
                println!(
                    "  EXEMPT {}: {} LOC ({}% docs)",
                    display_path.display(),
                    metrics.total_lines,
                    metrics.doc_pct()
                );
                exemptions += 1;
            } else {
                println!(
                    "  WARN   {}: {} LOC",
                    display_path.display(),
                    metrics.total_lines
                );
                violations += 1;
            }
        }
    }

    println!();
    if violations == 0 && exemptions == 0 {
        println!("All {} files within {limit} LOC.", files.len());
    } else if exemptions == 0 {
        println!("{violations} files exceed {limit} LOC (warn stage — not blocking).");
    } else {
        println!(
            "{violations} files exceed {limit} LOC, {exemptions} exempt (warn stage — not blocking)."
        );
    }

    Ok(0)
}

pub(crate) fn run_multi(scan_roots: &[&Path], limit: usize) -> std::io::Result<u8> {
    let mut max_exit = 0_u8;
    for root in scan_roots {
        let exit = run(root, limit)?;
        max_exit = max_exit.max(exit);
    }
    Ok(max_exit)
}

pub(crate) fn run_default() -> std::io::Result<u8> {
    let paths: Vec<&Path> = DEFAULT_SCAN_ROOTS.iter().copied().map(Path::new).collect();
    run_multi(&paths, DEFAULT_LIMIT)
}

#[cfg(test)]
mod tests;
