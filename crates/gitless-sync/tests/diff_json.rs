#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Phase 8 task Q — integration regression for the F3 evidence case
//! (`docs/research/llm-as-caller-usability-eval.md` § F3).
//!
//! The eval recorded `.obsidian/app.json` (status `local_only_changed`,
//! remote missing) returning a non-JSON shape under bare `diff`: stdout =
//! raw file dump, stderr = `(local only)\n`. The `--json` opt-in (task N+O+P)
//! collapses both channels into a single stdout line + 0-byte stderr per
//! `spec-cli-interface.md` § diff --json 출력 형식 + `spec-output-schema.md`
//! § diff sub-schema.
//!
//! Pinning four observables — exact stdout JSON, 0-byte stderr, ok exit,
//! parsed field shape — discriminates from regressions on any single
//! channel. The unit tests in `render.rs` cover JSON construction in
//! isolation; this test exercises the full pipeline (`compute_diff` →
//! `render::one_sided_json` → `run_with_client` write).

mod common;

use std::fs;

use tempfile::TempDir;

use gitless_sync::commands::diff::{DiffArgs, run_with_client};

use common::{TestGhClient, ok_resp, tree_args};

#[test]
fn f3_local_only_with_json_emits_single_line_json_and_silent_stderr() {
    // Local-only fixture, mirroring the eval F3 case shape (text content,
    // remote missing). Trees stub returns an empty tree so `compute_diff`
    // routes to the one-sided local path; `args.json = true` then triggers
    // `one_sided_json` instead of the default raw-dump-with-stderr-marker.
    //
    // Content uses LF only (no inner double-quotes) so the byte-exact
    // assertion reads cleanly — JSON escaping for `"` would obscure the
    // pinning shape rather than add coverage. The render unit test
    // `one_sided_json_local_only_text_populates_raw_with_content` already
    // covers serialization by `serde_json::to_vec` on richer payloads.
    let dir = TempDir::new().unwrap();
    let content = "hello\n";
    fs::write(dir.path().join("app.json"), content).unwrap();

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(br#"{"sha":"x","tree":[],"truncated":false}"#),
    );

    let args = DiffArgs {
        repo: Some("o/r".to_string()),
        branch: "main".to_string(),
        local: dir.path().to_str().unwrap().to_string(),
        keep_bom: false,
        path: "app.json".to_string(),
        json: true,
    };

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_with_client(&args, &mock, &mut stdout, &mut stderr)
        .expect("run_with_client should succeed for local-only --json");

    // Stderr must be silent under `--json` — spec-cli-interface.md § diff --json
    // 출력 형식 ("--json 명시 시 stderr side marker 미출력").
    assert!(
        stderr.is_empty(),
        "stderr must be 0 bytes under --json, got: {:?}",
        String::from_utf8_lossy(&stderr)
    );

    // Stdout must be exactly the one-line JSON the spec pins. The LF inside
    // `raw` is JSON-escaped to `\n` (two literal chars), and a trailing LF
    // closes the line per `render::json_outcome`.
    let stdout_str = String::from_utf8(stdout).expect("stdout is utf-8");
    assert_eq!(
        stdout_str,
        "{\"side\":\"local_only\",\"unified\":null,\"raw\":\"hello\\n\",\"binary\":false}\n",
        "stdout payload must match spec-output-schema.md § diff sub-schema verbatim"
    );

    // Parsed shape — discriminates from the binary case (which would null `raw`)
    // and from the both-side case (which would null `raw` + populate `unified`).
    let trimmed = stdout_str.trim_end_matches('\n');
    let parsed: serde_json::Value = serde_json::from_str(trimmed).expect("stdout is valid JSON");
    assert_eq!(parsed["side"], "local_only");
    assert_eq!(parsed["unified"], serde_json::Value::Null);
    assert_eq!(parsed["raw"], content);
    assert_eq!(parsed["binary"], false);
}
