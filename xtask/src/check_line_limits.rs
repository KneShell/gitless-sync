use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_LIMIT: usize = 300;
pub(crate) const DEFAULT_SCAN_ROOT: &str = "crates/gitless-sync/src";

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

pub(crate) fn run_default() -> std::io::Result<u8> {
    run(Path::new(DEFAULT_SCAN_ROOT), DEFAULT_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn measure_empty_content_returns_zero_lines() {
        let m = measure_content(PathBuf::from("a.rs"), "");
        assert_eq!(m.total_lines, 0);
        assert_eq!(m.doc_lines, 0);
        assert!(!m.is_doc_heavy());
    }

    #[test]
    fn measure_counts_outer_doc_lines() {
        let content = "/// doc\n/// more doc\nfn x() {}\n";
        let m = measure_content(PathBuf::from("a.rs"), content);
        assert_eq!(m.total_lines, 3);
        assert_eq!(m.doc_lines, 2);
    }

    #[test]
    fn measure_counts_inner_doc_lines() {
        let content = "//! module doc\nfn x() {}\n";
        let m = measure_content(PathBuf::from("a.rs"), content);
        assert_eq!(m.total_lines, 2);
        assert_eq!(m.doc_lines, 1);
    }

    #[test]
    fn measure_ignores_regular_comments() {
        let content = "// comment\nfn x() {}\n";
        let m = measure_content(PathBuf::from("a.rs"), content);
        assert_eq!(m.total_lines, 2);
        assert_eq!(m.doc_lines, 0);
    }

    #[test]
    fn measure_handles_indented_doc_lines() {
        let content = "  /// indented doc\n    //! module doc\nfn x() {}\n";
        let m = measure_content(PathBuf::from("a.rs"), content);
        assert_eq!(m.doc_lines, 2);
    }

    #[test]
    fn doc_heavy_at_50_percent_flagged() {
        let m = FileMetrics {
            path: PathBuf::new(),
            total_lines: 10,
            doc_lines: 5,
        };
        assert!(m.is_doc_heavy());
    }

    #[test]
    fn doc_heavy_below_50_percent_not_flagged() {
        let m = FileMetrics {
            path: PathBuf::new(),
            total_lines: 10,
            doc_lines: 4,
        };
        assert!(!m.is_doc_heavy());
    }

    #[test]
    fn doc_heavy_above_50_percent_flagged() {
        let m = FileMetrics {
            path: PathBuf::new(),
            total_lines: 10,
            doc_lines: 7,
        };
        assert!(m.is_doc_heavy());
    }

    #[test]
    fn doc_heavy_zero_total_returns_false() {
        let m = FileMetrics {
            path: PathBuf::new(),
            total_lines: 0,
            doc_lines: 0,
        };
        assert!(!m.is_doc_heavy());
    }

    #[test]
    fn doc_pct_zero_total_returns_zero() {
        let m = FileMetrics {
            path: PathBuf::new(),
            total_lines: 0,
            doc_lines: 0,
        };
        assert_eq!(m.doc_pct(), 0);
    }

    #[test]
    fn doc_pct_basic() {
        let m = FileMetrics {
            path: PathBuf::new(),
            total_lines: 100,
            doc_lines: 60,
        };
        assert_eq!(m.doc_pct(), 60);
    }

    #[test]
    fn collect_rust_files_walks_subdirs_and_skips_non_rs() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/foo.rs"), "").unwrap();
        fs::write(root.join("a/b/bar.rs"), "").unwrap();
        fs::write(root.join("a/baz.txt"), "").unwrap();
        fs::write(root.join("readme.md"), "").unwrap();

        let files = collect_rust_files(root).unwrap();
        assert_eq!(files.len(), 2);
        assert!(
            files
                .iter()
                .all(|p| p.extension().and_then(|s| s.to_str()) == Some("rs"))
        );
    }

    #[test]
    fn collect_rust_files_returns_sorted_paths() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("zeta.rs"), "").unwrap();
        fs::write(root.join("alpha.rs"), "").unwrap();
        fs::write(root.join("mu.rs"), "").unwrap();

        let files = collect_rust_files(root).unwrap();
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted);
    }

    #[test]
    fn collect_rust_files_empty_dir_returns_empty() {
        let dir = tempdir().unwrap();
        let files = collect_rust_files(dir.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn collect_rust_files_missing_root_returns_io_error() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        let result = collect_rust_files(&missing);
        assert!(result.is_err());
    }

    #[test]
    fn run_returns_zero_with_violation_in_warn_stage() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let big = "fn x() {}\n".repeat(350);
        fs::write(root.join("big.rs"), big).unwrap();

        let exit = run(root, 300).unwrap();
        assert_eq!(exit, 0);
    }

    #[test]
    fn run_returns_zero_with_no_violations() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("small.rs"), "fn x() {}\n").unwrap();

        let exit = run(root, 300).unwrap();
        assert_eq!(exit, 0);
    }

    #[test]
    fn run_returns_zero_when_doc_heavy_file_exempt() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let mut content = String::new();
        for _ in 0..400 {
            content.push_str("/// doc line\n");
        }
        fs::write(root.join("docs.rs"), content).unwrap();

        let exit = run(root, 300).unwrap();
        assert_eq!(exit, 0);
    }

    #[test]
    fn run_propagates_io_error_for_missing_root() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        let result = run(&missing, 300);
        assert!(result.is_err());
    }
}
