//! Test sibling for `gitattributes.rs`. Loaded via
//! `#[cfg(test)] #[path = "gitattributes_tests.rs"] mod tests;` so the
//! parser implementation stays within the 300-LOC gate.

use std::fs;

use tempfile::TempDir;

use super::*;

fn kv(name: &str, value: &str) -> RawAttribute {
    RawAttribute::KeyValue {
        name: name.to_string(),
        value: value.to_string(),
    }
}

#[test]
fn empty_root_returns_empty_attributes() {
    let dir = TempDir::new().unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert!(attrs.is_empty());
    assert!(attrs.match_path("any/file.txt").is_empty());
}

#[test]
fn root_gitattributes_matches_simple_glob() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert_eq!(attrs.match_path("notes.txt"), vec![kv("text", "auto")]);
    assert!(attrs.match_path("README.md").is_empty());
}

#[test]
fn comment_and_blank_lines_are_ignored() {
    let dir = TempDir::new().unwrap();
    let body = "# header comment\n\n*.txt text=auto\n# trailing\n";
    fs::write(dir.path().join(".gitattributes"), body).unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert_eq!(attrs.match_path("a.txt"), vec![kv("text", "auto")]);
}

#[test]
fn parses_set_unset_keyvalue_and_unspecified_attributes() {
    let dir = TempDir::new().unwrap();
    let body = "*.bin binary -diff filter=lfs !ident\n";
    fs::write(dir.path().join(".gitattributes"), body).unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert_eq!(
        attrs.match_path("foo.bin"),
        vec![
            RawAttribute::Set("binary".into()),
            RawAttribute::Unset("diff".into()),
            kv("filter", "lfs"),
            RawAttribute::Unspecified("ident".into()),
        ]
    );
}

#[test]
fn negation_pattern_is_silently_skipped() {
    let dir = TempDir::new().unwrap();
    let body = "*.log text=auto\n!keep.log binary\n";
    fs::write(dir.path().join(".gitattributes"), body).unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert_eq!(attrs.match_path("debug.log"), vec![kv("text", "auto")]);
    // !keep.log line skipped, but keep.log still matches the *.log rule
    assert_eq!(attrs.match_path("keep.log"), vec![kv("text", "auto")]);
}

#[test]
fn multi_level_deepest_appears_last_in_accumulated_order() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
    let sub = dir.path().join("docs");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join(".gitattributes"), "*.txt eol=lf\n").unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert_eq!(
        attrs.match_path("docs/notes.txt"),
        vec![kv("text", "auto"), kv("eol", "lf")]
    );
}

#[test]
fn line_level_order_preserves_natural_order() {
    let dir = TempDir::new().unwrap();
    let body = "*.txt text=auto\n*.txt eol=lf\n";
    fs::write(dir.path().join(".gitattributes"), body).unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert_eq!(
        attrs.match_path("a.txt"),
        vec![kv("text", "auto"), kv("eol", "lf")]
    );
}

#[test]
fn sub_dir_attributes_do_not_match_outside_their_dir() {
    let dir = TempDir::new().unwrap();
    let sub = dir.path().join("docs");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join(".gitattributes"), "*.md text=auto\n").unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert!(attrs.match_path("README.md").is_empty());
    assert!(!attrs.match_path("docs/foo.md").is_empty());
}

#[test]
fn dot_git_directory_is_skipped() {
    let dir = TempDir::new().unwrap();
    let dot_git = dir.path().join(".git");
    fs::create_dir(&dot_git).unwrap();
    fs::write(dot_git.join(".gitattributes"), "*.txt text=auto\n").unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert!(attrs.is_empty());
}

#[test]
fn target_directory_is_not_skipped() {
    // BUILTIN_IGNORES is for scan walking, not attribute discovery —
    // working-tree dirs (target/, node_modules/) still load attributes.
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join(".gitattributes"), "*.bin binary\n").unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert_eq!(
        attrs.match_path("target/foo.bin"),
        vec![RawAttribute::Set("binary".into())]
    );
}

#[test]
fn pattern_only_line_without_attributes_is_skipped() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitattributes"), "*.txt\n").unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert!(attrs.match_path("notes.txt").is_empty());
}
