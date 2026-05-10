//! Tree-entry → [`RemoteFile`] dispatch (mode bits + NFC).
//!
//! Pure function — no `GhClient`, no IO. Tests construct `TreeEntry` values
//! directly and exercise the mode-arm routing without going through the
//! `super::fetch` orchestrator.

use super::parse::TreeEntry;
use crate::shared::path::to_nfc;

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub path: String,
    pub sha: String,
    /// Tree mode bit: `100644` regular, `100755` executable, `160000`
    /// submodule, `120000` symlink. Carried through to v1.1 JSON
    /// `files[].mode`. Phase 5 task G adds submodule (`160000`); H/J
    /// extend the others (`spec-output-schema.md` § v1.1,
    /// `spec-domain-pitfalls.md`).
    pub mode: String,
    /// Blob byte size from the Trees response. `Some(n)` for `blob`
    /// entries; `None` for `commit` (submodule) entries which carry no
    /// size. Phase 7.2 task N pipes this into
    /// `commands::scan::hash_remote::try_remote_size_gate` so 50 MB+ blobs
    /// short-circuit to `Status::Failed` before any blob fetch
    /// (`spec-hash-and-normalize.md` § Phase 7 — 큰 파일 처리).
    pub size: Option<u64>,
}

/// Map one [`TreeEntry`] to a [`RemoteFile`]; `None` when the entry is
/// not a supported v0.1 file.
///
/// Routing:
/// - `blob` + `100644` / `100755`: regular vs executable. `compare.rs`
///   classifies on content; the mode bit rides along to v1.1 JSON.
/// - `blob` + `120000`: symlink. `compare.rs` promotes to
///   `Status::Failed` + `failed_reason: "symlink"`. The blob `sha` points
///   to the link target path — not followed.
/// - `commit` + `160000`: submodule. `compare.rs` promotes to
///   `Status::Failed` + `failed_reason: "submodule"`. The `sha` is the
///   submodule pointer commit, useful info for the caller.
///
/// Anything else drops. A `blob` with an unsupported mode emits a stderr
/// warning so the caller can audit the skip (G-010).
///
/// Takes `entry` by value so `make_remote` can move `sha` and `mode`
/// into the [`RemoteFile`] without cloning. `path` is borrowed only long
/// enough for `to_nfc` to canonicalize it. The `matches!` arm runs the
/// scrutinee borrow to completion before the move, then we re-inspect
/// `entry.entry_type` for the `blob`-with-unsupported-mode warning.
pub(super) fn classify_tree_entry(entry: TreeEntry) -> Option<RemoteFile> {
    if matches!(
        (entry.entry_type.as_str(), entry.mode.as_str()),
        ("blob", "100644" | "100755" | "120000") | ("commit", "160000")
    ) {
        return Some(make_remote(entry));
    }
    if entry.entry_type == "blob" {
        eprintln!(
            "warning: skipping {} (mode {} unsupported in v0.1)",
            entry.path, entry.mode
        );
    }
    None
}

fn make_remote(entry: TreeEntry) -> RemoteFile {
    RemoteFile {
        path: to_nfc(&entry.path),
        sha: entry.sha,
        mode: entry.mode,
        size: entry.size,
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
mod tests {
    use super::*;

    fn entry(path: &str, mode: &str, entry_type: &str, sha: &str) -> TreeEntry {
        TreeEntry {
            path: path.to_string(),
            mode: mode.to_string(),
            entry_type: entry_type.to_string(),
            sha: sha.to_string(),
            size: None,
        }
    }

    fn entry_with_size(
        path: &str,
        mode: &str,
        entry_type: &str,
        sha: &str,
        size: u64,
    ) -> TreeEntry {
        TreeEntry {
            path: path.to_string(),
            mode: mode.to_string(),
            entry_type: entry_type.to_string(),
            sha: sha.to_string(),
            size: Some(size),
        }
    }

    #[test]
    fn classifies_blob_regular_carries_mode_and_sha() {
        let rf = classify_tree_entry(entry("README.md", "100644", "blob", "sha1")).unwrap();
        assert_eq!(rf.path, "README.md");
        assert_eq!(rf.mode, "100644");
        assert_eq!(rf.sha, "sha1");
        assert_eq!(rf.size, None, "entry built with size: None propagates");
    }

    #[test]
    fn propagates_blob_size_into_remote_file_for_phase7_size_gate() {
        // Phase 7.2 task N — Trees response size field flows through
        // `make_remote` into `RemoteFile.size` so
        // `commands::scan::hash_remote::try_remote_size_gate` can pre-flight
        // 50 MB+ blobs without a fetch.
        let rf = classify_tree_entry(entry_with_size(
            "big.bin", "100644", "blob", "sha1", 60_000_000,
        ))
        .unwrap();
        assert_eq!(rf.size, Some(60_000_000));
    }

    #[test]
    fn submodule_size_remains_none_even_when_present_on_entry() {
        // `type=commit` (submodule) entries carry no size in real Trees
        // responses, but the gate stays defensive — even if a malformed
        // response sets one, the submodule arm of `short_circuit` promotes
        // first. Here we just lock the field-propagation contract: whatever
        // `entry.size` was, that's what `RemoteFile.size` becomes.
        let rf = classify_tree_entry(entry_with_size(
            "vendor/lib",
            "160000",
            "commit",
            "deadbeef",
            12_345,
        ))
        .unwrap();
        assert_eq!(rf.size, Some(12_345));
    }

    #[test]
    fn classifies_blob_executable_carries_mode_and_sha() {
        // Phase 5 task J: executables flow through the normal hash path;
        // `compare.rs` decides Identical/Drift on content, the mode bit
        // is reported in v1.1 JSON.
        let rf = classify_tree_entry(entry("exec.sh", "100755", "blob", "s3")).unwrap();
        assert_eq!(rf.path, "exec.sh");
        assert_eq!(rf.mode, "100755");
        assert_eq!(rf.sha, "s3");
    }

    #[test]
    fn classifies_blob_symlink_carries_target_blob_sha() {
        // Phase 5 task H: symlinks (`type=blob`, `mode=120000`) carry
        // through with the blob `sha` so `compare.rs` can promote to
        // `Status::Failed` + `failed_reason: "symlink"`. We do not follow
        // the link — the blob contents are the target path.
        let rf =
            classify_tree_entry(entry("link/to/elsewhere", "120000", "blob", "feedface")).unwrap();
        assert_eq!(rf.path, "link/to/elsewhere");
        assert_eq!(rf.mode, "120000");
        assert_eq!(rf.sha, "feedface");
    }

    #[test]
    fn classifies_commit_submodule_carries_pointer_sha() {
        // Phase 5 task G: submodules (`type=commit`, `mode=160000`)
        // surface their pointer commit `sha` so `compare.rs` can promote
        // to `Status::Failed` + `failed_reason: "submodule"`.
        let rf =
            classify_tree_entry(entry("vendor/lib", "160000", "commit", "deadbeefcafe")).unwrap();
        assert_eq!(rf.path, "vendor/lib");
        assert_eq!(rf.mode, "160000");
        assert_eq!(rf.sha, "deadbeefcafe");
    }

    #[test]
    fn drops_blob_with_unsupported_mode() {
        // Belt-and-suspenders: a `blob` with a mode git itself does not
        // emit (e.g. `100664`) routes through the unsupported-mode warn
        // branch and is dropped.
        assert!(classify_tree_entry(entry("weird", "100664", "blob", "s2")).is_none());
    }

    #[test]
    fn drops_commit_with_non_submodule_mode() {
        // A `type: "commit"` entry that lacks the `160000` mode bit is
        // ignored — only the canonical submodule shape promotes through.
        // Defends against malformed responses.
        assert!(classify_tree_entry(entry("weird", "100644", "commit", "s2")).is_none());
    }

    #[test]
    fn drops_tree_type_directory_entry() {
        // Recursive=1 still includes one `tree` row per sub-directory.
        // We only want files, so directories drop without a warning.
        assert!(classify_tree_entry(entry("src", "040000", "tree", "tsha")).is_none());
    }

    #[test]
    fn normalizes_remote_path_to_nfc() {
        // GitHub returns paths exactly as committed. If a file was
        // committed from a macOS shell that emitted NFD bytes, the
        // response carries NFD. We canonicalize to NFC so the comparison
        // key aligns with the walker's NFC output.
        let nfd_path = "\u{1100}\u{1161}.txt";
        let rf = classify_tree_entry(entry(nfd_path, "100644", "blob", "s1")).unwrap();
        assert_ne!(rf.path, nfd_path);
        assert_eq!(rf.path, "\u{AC00}.txt");
    }
}
