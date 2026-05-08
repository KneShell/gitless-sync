//! Pure rendering helpers for the `diff` slice — given raw bytes and a
//! `keep_bom` flag, produce a `DiffOutcome`. No IO, no orchestration; called
//! from `compute.rs`. Owns the `DiffOutcome` value type so `render` is a
//! leaf module (no edges back to `compute`).

use similar::TextDiff;

use crate::shared::normalize::{is_binary, normalize_text};

/// Result of a `diff` invocation, separated from actual I/O so tests can
/// inspect `stdout` / `stderr_message` without capturing real handles.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct DiffOutcome {
    pub stdout: Vec<u8>,
    pub stderr_message: String,
}

pub(super) fn one_sided(content: &[u8], label: &'static str, keep_bom: bool) -> DiffOutcome {
    if is_binary(content) {
        DiffOutcome {
            stdout: Vec::new(),
            stderr_message: format!("{label} (binary file, content not shown)"),
        }
    } else {
        DiffOutcome {
            stdout: normalize_text(content, keep_bom),
            stderr_message: label.to_string(),
        }
    }
}

pub(super) fn both_sides(local: &[u8], remote: &[u8], key: &str, keep_bom: bool) -> DiffOutcome {
    if is_binary(local) || is_binary(remote) {
        return DiffOutcome {
            stdout: Vec::new(),
            stderr_message: "binary file, diff skipped".to_string(),
        };
    }
    let old_bytes = normalize_text(remote, keep_bom);
    let new_bytes = normalize_text(local, keep_bom);
    let old_str = String::from_utf8_lossy(&old_bytes);
    let new_str = String::from_utf8_lossy(&new_bytes);
    let diff = TextDiff::from_lines(old_str.as_ref(), new_str.as_ref());
    let header_a = format!("a/{key}");
    let header_b = format!("b/{key}");
    let mut unified = diff.unified_diff();
    unified.header(&header_a, &header_b);
    DiffOutcome {
        stdout: unified.to_string().into_bytes(),
        stderr_message: String::new(),
    }
}
