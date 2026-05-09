//! Test sibling for `gitattributes.rs` covering the K1.5 `AttributeMatch`
//! whitelist + reduction (`GitAttributes::classify_path`). Split from
//! `gitattributes_tests.rs` to keep both files within the 300-LOC gate.

use std::fs;

use tempfile::TempDir;

use super::*;

fn classify(body: &str, path: &str) -> AttributeMatch {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitattributes"), body).unwrap();
    GitAttributes::load(dir.path()).unwrap().classify_path(path)
}

// 5 whitelist scenarios

#[test]
fn classify_text_auto_returns_text_auto() {
    assert_eq!(
        classify("*.txt text=auto\n", "notes.txt"),
        AttributeMatch::TextAuto
    );
}

#[test]
fn classify_binary_set_returns_binary() {
    assert_eq!(
        classify("*.bin binary\n", "data.bin"),
        AttributeMatch::Binary
    );
}

#[test]
fn classify_eol_lf_returns_eol_lf() {
    assert_eq!(classify("*.sh eol=lf\n", "run.sh"), AttributeMatch::EolLf);
}

#[test]
fn classify_eol_crlf_returns_eol_crlf() {
    assert_eq!(
        classify("*.bat eol=crlf\n", "run.bat"),
        AttributeMatch::EolCrlf
    );
}

#[test]
fn classify_filter_lfs_returns_lfs_pointer() {
    assert_eq!(
        classify("*.psd filter=lfs\n", "art.psd"),
        AttributeMatch::LfsPointer
    );
}

// 5 unsupported scenarios

#[test]
fn classify_working_tree_encoding_returns_unsupported() {
    assert_eq!(
        classify("*.foo working-tree-encoding=UTF-16\n", "x.foo"),
        AttributeMatch::Unsupported {
            attribute_name: "working-tree-encoding".into(),
        }
    );
}

#[test]
fn classify_ident_set_returns_unsupported() {
    assert_eq!(
        classify("*.c ident\n", "main.c"),
        AttributeMatch::Unsupported {
            attribute_name: "ident".into(),
        }
    );
}

#[test]
fn classify_filter_clean_non_lfs_returns_unsupported() {
    assert_eq!(
        classify("*.css filter=clean\n", "site.css"),
        AttributeMatch::Unsupported {
            attribute_name: "filter".into(),
        }
    );
}

#[test]
fn classify_legacy_crlf_bare_returns_unsupported() {
    // `crlf` (bare Set) is the legacy form, not in the whitelist. Only
    // `eol=crlf` (KeyValue) is supported.
    assert_eq!(
        classify("*.txt crlf\n", "notes.txt"),
        AttributeMatch::Unsupported {
            attribute_name: "crlf".into(),
        }
    );
}

#[test]
fn classify_eol_native_returns_unsupported() {
    // Only `eol=lf` and `eol=crlf` are whitelisted; `eol=native` is not.
    assert_eq!(
        classify("*.txt eol=native\n", "notes.txt"),
        AttributeMatch::Unsupported {
            attribute_name: "eol".into(),
        }
    );
}

// Precedence / edge cases pinning the 4-rule order documented on
// `GitAttributes::classify_path`.

#[test]
fn classify_empty_match_returns_unspecified() {
    let dir = TempDir::new().unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert_eq!(
        attrs.classify_path("anything.txt"),
        AttributeMatch::Unspecified
    );
}

#[test]
fn classify_no_attributes_match_returns_unspecified() {
    // .gitattributes exists but no rule matches the queried path.
    assert_eq!(
        classify("*.bin binary\n", "notes.txt"),
        AttributeMatch::Unspecified
    );
}

#[test]
fn classify_filter_lfs_with_co_attributes_wins_over_unsupported() {
    // Canonical git-lfs line: filter=lfs is authoritative; diff=lfs and
    // merge=lfs are advisory; -text would normally be unsupported.
    assert_eq!(
        classify("*.psd filter=lfs diff=lfs merge=lfs -text\n", "art.psd"),
        AttributeMatch::LfsPointer
    );
}

#[test]
fn classify_filter_lfs_wins_over_whitelist_co_attributes() {
    assert_eq!(
        classify("*.bin filter=lfs binary\n", "blob.bin"),
        AttributeMatch::LfsPointer
    );
}

#[test]
fn classify_unsupported_wins_over_whitelist_when_no_lfs() {
    // Rule 2 fires even when a whitelist attribute also matches.
    assert_eq!(
        classify("*.txt working-tree-encoding=UTF-16 eol=lf\n", "notes.txt"),
        AttributeMatch::Unsupported {
            attribute_name: "working-tree-encoding".into(),
        }
    );
}

#[test]
fn classify_unsupported_attribute_name_is_first_non_whitelist() {
    // First non-whitelist token wins the attribute_name.
    assert_eq!(
        classify("*.c ident filter=clean\n", "main.c"),
        AttributeMatch::Unsupported {
            attribute_name: "ident".into(),
        }
    );
}

#[test]
fn classify_text_auto_then_binary_uses_last_wins() {
    assert_eq!(
        classify("*.x text=auto\n*.x binary\n", "thing.x"),
        AttributeMatch::Binary
    );
}

#[test]
fn classify_binary_then_text_auto_uses_last_wins() {
    assert_eq!(
        classify("*.x binary\n*.x text=auto\n", "thing.x"),
        AttributeMatch::TextAuto
    );
}

#[test]
fn classify_bare_text_set_is_unsupported() {
    // Whitelist requires `text=auto` (KeyValue). Bare `text` (Set) is not
    // honored.
    assert_eq!(
        classify("*.txt text\n", "notes.txt"),
        AttributeMatch::Unsupported {
            attribute_name: "text".into(),
        }
    );
}

#[test]
fn classify_unset_text_is_unsupported() {
    // `-text` (Unset) is not in the whitelist.
    assert_eq!(
        classify("*.txt -text\n", "notes.txt"),
        AttributeMatch::Unsupported {
            attribute_name: "text".into(),
        }
    );
}

#[test]
fn classify_unspecified_bang_text_is_unsupported() {
    // `!text` (default-restoring) is not whitelisted.
    assert_eq!(
        classify("*.txt !text\n", "notes.txt"),
        AttributeMatch::Unsupported {
            attribute_name: "text".into(),
        }
    );
}

#[test]
fn classify_deeper_gitattributes_contributes_to_reduction() {
    // Last-wins applies across deepest-last accumulation order from
    // match_path, so a sub-dir entry overrides root.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
    let sub = dir.path().join("docs");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join(".gitattributes"), "*.txt eol=lf\n").unwrap();
    let attrs = GitAttributes::load(dir.path()).unwrap();
    assert_eq!(attrs.classify_path("docs/notes.txt"), AttributeMatch::EolLf);
}
