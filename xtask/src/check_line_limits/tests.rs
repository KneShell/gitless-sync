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
fn run_returns_one_with_violation_in_deny_stage() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let big = "fn x() {}\n".repeat(350);
    fs::write(root.join("big.rs"), big).unwrap();

    let exit = run(root, 300).unwrap();
    assert_eq!(exit, 1);
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
fn run_returns_one_when_violation_and_exemption_coexist() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let big_code = "fn x() {}\n".repeat(350);
    fs::write(root.join("big.rs"), big_code).unwrap();

    let mut docs = String::new();
    for _ in 0..400 {
        docs.push_str("/// doc line\n");
    }
    fs::write(root.join("docs.rs"), docs).unwrap();

    let exit = run(root, 300).unwrap();
    assert_eq!(exit, 1);
}

#[test]
fn run_propagates_io_error_for_missing_root() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");
    let result = run(&missing, 300);
    assert!(result.is_err());
}

#[test]
fn run_multi_visits_each_root_and_returns_one_in_deny_stage() {
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();
    fs::write(dir_a.path().join("a.rs"), "fn a() {}\n").unwrap();
    let big = "fn x() {}\n".repeat(350);
    fs::write(dir_b.path().join("b.rs"), big).unwrap();

    let roots: Vec<&Path> = vec![dir_a.path(), dir_b.path()];
    let exit = run_multi(&roots, 300).unwrap();
    assert_eq!(exit, 1);
}

#[test]
fn run_multi_propagates_io_error_for_missing_root() {
    let dir_a = tempdir().unwrap();
    let missing = dir_a.path().join("does-not-exist");
    let roots: Vec<&Path> = vec![dir_a.path(), &missing];
    let result = run_multi(&roots, 300);
    assert!(result.is_err());
}

#[test]
fn run_multi_with_empty_root_list_returns_zero() {
    let roots: Vec<&Path> = Vec::new();
    let exit = run_multi(&roots, 300).unwrap();
    assert_eq!(exit, 0);
}
