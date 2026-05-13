# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2026-05-13

First binary distribution release. No source-code or behavior changes vs v0.6.0; release infrastructure only.

### Added

- Prebuilt portable binaries via GitHub Releases for Windows (`x86_64-pc-windows-msvc`), Linux (`x86_64-unknown-linux-musl`, static), and macOS (`aarch64-apple-darwin`).
- SHA256 checksums (`sha256sums.txt` + per-asset `.sha256`) and SLSA build provenance attestation. Verify with `gh attestation verify <binary> --repo KneShell/gitless-sync`.
- README "Install (prebuilt binary)" section.

### Infrastructure

- `.github/workflows/release.yml` — tag-push trigger + `workflow_dispatch` dry-run mode.
- `docs/specs/spec-release-distribution.md` — distribution spec.

## [0.6.0] - 2026-05-12

> Phase 10 (post-v0.5.0 verification finding 해소, Finding 1/2/3 해소) 누적. post-v0.5.0 clean-context audit (2026-05-12, 메모리 차단 fresh context) 결과 발견된 3 finding 해소 + v0.6.0 minor release. Phase 10.1 (spec/CHANGELOG 사전 확정) + Phase 10.2 (Finding 2 impl wire shape + schema bump) + Phase 10.3 (test 갱신 + spec 마감) + Phase 10.4 (release) + Phase 10.5 (plan close). 결정 trail은 post-v0.5.0 clean-context audit finding + `docs/specs/spec-output-schema.md` § v1.5 → v1.6 변경. 상세 task별 결과는 git history (`git log --grep="Phase 10"`) + `docs/ralph/implementation-plan.md`.

### Added

- Phase 10.1 — Finding 1 SemVer 라벨 모순 해소 (`spec-output-schema.md` § v1.4 → v1.5 변경 § 본문에 면제 근거 한 줄 명문화, task A). v1.4 caller `files == null` / key 부재로 summary-only mode 판단하는 분기는 v1.4 시점 도입된 신규 가정이라 SemVer 보호 대상 아님 — 라벨 ("minor bump") 유지 + 면제 근거 명문화. doc-only.
- Phase 10.1 — Finding 2 hash_io explicit emit 사전 spec 확정 (`spec-output-schema.md` § v1.5 → v1.6 변경 § 신규, task B). 이전 v1.5 까지: hash_io 는 `failed_reason` 필드 부재 (`null`) sentinel 로 의미 표현 (key 부재 = hash_io). v1.6 부터: hash_io 도 explicit `failed_reason: "hash_io"` wire emit. summary-only `files[]` entry 도 동일 3 field shape (path + presence + failed_reason) — 이전 v1.5 의 hash_io 2 field special case 제거. v1.5 caller `failed_reason` field absent 가정 분기는 v1.5 시점 도입된 신규 가정이라 SemVer 보호 대상 아님 (Finding 1 면제 logic 일반화 — 공통 면제 표 spec § v1.5 → v1.6 변경 § 본문).
- Phase 10.1 — Finding 3 minimal entry shape 발산 강조 한 줄 (`spec-output-schema.md` § `--summary-only` 출력 본문, task C). summary-only `files[]` entry 는 일반 mode entry 와 shape 발산 — caller 는 응답 shape 추론 금지, 자신의 `--summary-only` argument 기준 mode 분기. doc-only.
- Phase 10.1 — v1.6 신규 Acceptance Criteria 5 시나리오 (`spec-output-schema.md` § v1.6 신규, task D): (1) `schema_version == "1.6"`, (2) summary-only + hash_io → 3 field (이전 v1.5 의 2 field 에서 변경), (3) summary-only + 그 외 reason → 동일 3 field 유지, (4) v1.5 caller × v1.6 hash_io entry parse 정상, (5) status omit 정책 유지.
- Phase 10.2 — Finding 2 impl 본진 (task F~J 5종). `commands/scan/output.rs::SCHEMA_VERSION` `"1.5"` → `"1.6"` (task F) + `compare/types.rs::FailedReason` enum 변경 (task G, `None` 특수 케이스 제거 + 명시 `HashIo` variant 추가 + serde rename `"hash_io"`) + caller 전수 갱신 (task H, 10~11 호출처 의미 검토 후 `FailedReason::HashIo` 변환, `Option<FailedReason>` 시그니처 유지 — Failed 외 entry 는 여전히 None) + `commands/scan/summary_view.rs::strip_to_summary_failed` 갱신 (task I, hash_io entry 도 다른 reason 과 동일 3 field emit, v1.5 의 2 field special case 제거) + v1.6 backward-compat lock (task J, `tests/scan_output_backward_compat.rs` `V16` client struct 신규 + `v1_6_sample_json` fixture + V10/V11/V12/V15 client × v1.6 sample 4 client lock + V15 client × v1.6 hash_io entry `Some("hash_io")` deserialize 정상 검증).
- Phase 10.4 — README `### scan` `--summary-only` 섹션 wording 정밀화 (task N). `failed_reason` 필드가 v1.6 부터 hash_io 포함 모든 failed reason 에 대해 명시 emit 명시 + JSON 예시 본문 hash_io 케이스 3 field shape 정확. doc-only.
- Schema v1.5 → v1.6 minor bump — Finding 2 wire shape change 동반. v1.0 / v1.1 / v1.2 / v1.3 / v1.4 / v1.5 backward-compat lock test 갱신 (task J).

### Changed

- `schema_version` `"1.5"` → `"1.6"`.
- `compare/types.rs::FailedReason` enum — v1.5 의 `None` special case (`hash_io` signal, key 부재로 의미 표현) 제거 + 명시 `HashIo` variant 추가 + serde rename `"hash_io"`. `Option<FailedReason>` 시그니처는 유지 (Failed 외 entry 는 여전히 None).
- release: v0.5.0 → **v0.6.0** minor (Finding 2 wire shape change + schema bump 동반). v0.5.1 patch 회피 — schema 버전이 호출자 contract 일급 signal이라 SemVer minor가 정직 (Phase 9 v0.4.2 → v0.5.0 동일 정책).

### Spec

- `spec-output-schema.md` § v1.4 → v1.5 변경 § 본문 — Finding 1 면제 근거 한 줄 (task A).
- `spec-output-schema.md` § v1.5 → v1.6 변경 § 신규 — Finding 2 본진 (wire shape change + backward-compat 표 + migration lock + 공통 면제 표 — task A 의 v1.4→v1.5 면제 근거와 동일 패턴 mirror 일반화) (task B).
- `spec-output-schema.md` § `--summary-only` 출력 § 본문 — Finding 3 강조 한 줄 (task C).
- `spec-output-schema.md` § v1.6 신규 Acceptance Criteria 5 시나리오 (task D).
- `spec-error-contracts.md` § Per-file Pitfall Reasons hash_io row + `failed_reason` 부재 정합 — v1.5 의 `None` special case → v1.6 의 `HashIo` variant 명시 (task E).
- `spec-output-schema.md` + `spec-error-contracts.md` + `spec-classification.md` § 마감 점검 정합 (task M) — task K~L 결과 vs spec 본문 cross-check + § minimal entry shape 발산 강조 위치 정합 + acceptance 시나리오 wording 정확 + `spec-error-contracts.md` FailedReason enum 11 cover 갱신 + `spec-classification.md` § Failed status § FailedReason 인용 dead reference 0 검증.

### Verified

- 신규 / 갱신 test 시나리오 — `commands/scan/output.rs::tests` (task F, `schema_version_field_serializes_as_1_6` 갱신) + `compare/types.rs::tests` (task G, `failed_reason_hash_io_serializes_snake_case`) + `commands/scan/summary_view.rs::tests` (task I + K, hash_io entry strip 3 field shape + encoding entry rich payload 3 field shape projection lock) + `tests/scan_output_backward_compat.rs` v1.6 lock (task J, `V16` client struct + `v1_6_sample_json` fixture + V10/V11/V12/V15 × v1.6 sample 4 client lock + V15 × hash_io entry `Some("hash_io")` deserialize 정상 검증) + `tests/scan_summary_only_hash_io.rs` 신규 (task L, 104 LOC, 권한 0 file fixture 시뮬레이션 + `--summary-only` JSON parse + `files[]` entry `failed_reason: "hash_io"` 명시 검증).
- Hard gate full pipeline PASS — `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo xtask check-line-limits` / `cargo xtask check-cycles` / `cargo machete` / `cargo test --workspace` / `cargo tarpaulin --engine llvm --workspace --out Stdout`.
- 492 + 88 unit + integration test pass / 0 failed (gitless-sync workspace + xtask).
- tarpaulin 88.99% (1325/1489 lines).
- post-v0.5.0 clean-context audit (2026-05-12, 메모리 차단 fresh context) Finding 1/2/3 모두 해소.

## [0.5.0] - 2026-05-12

> Phase 9 (CLI UX Feedback Fix, vault 도그푸딩 F1/F2/F3 해소) 누적. F1 `scan` / `diff` 서브커맨드 `--help` description + F2 `init` wording 정밀화 + F3 `--summary-only` 모드 failed status entry minimal emit (path + presence + failed_reason 3 field). schema_version 1.4 → 1.5 (F3 한정 caller-visible behavior change). Phase 9.1 (spec/CHANGELOG 사전 확정) + Phase 9.2 (F1+F2 clap surface) + Phase 9.3 (F3 summary-only failed visibility) + Phase 9.4 (release) + Phase 9.5 (plan close). 결정 trail은 post-v0.4.2 vault dogfood feedback (Phase 9 source, 본문은 Phase 10 진입 직전 삭제) + `docs/specs/spec-output-schema.md` § v1.4 → v1.5 변경. 상세 task별 결과는 git history (`git log --grep="Phase 9"`) + `docs/ralph/implementation-plan.md`.

### Added

- Phase 9.2 — `scan` / `diff` 서브커맨드 `#[command(about = "...")]` derive 추가 (F1). `cargo run -- --help` 최상위 listing에 두 서브커맨드 description 한 줄 노출 — 기존 빈 공란 해소. clap surface only — output schema / runtime behavior 영향 0.
- Phase 9.2 — `init` 서브커맨드 `about` wording 정밀화 (F2). 기존 `"Print a gitless-sync.toml template to stdout (you redirect to a file)"` → `"Emit gitless-sync.toml body from input args (stdout)"`. post-v0.4.2 vault dogfood feedback F2 "template printer" 인지부조화 해소. clap surface only.
- Phase 9.3 — `--summary-only` 모드 failed status entry 한정 emit (F3). 기존 v1.4까지 summary-only 시 `files` 필드 자체를 omit했으나, `summary.failed > 0` 발생 시 minimal entry list (`path` + `presence` + `failed_reason` 세 field만, sha / size / mode / diff_meaningful / lfs_pointer / size_bytes 등 detail field 모두 omit) 포함. failed 0건 시 기존 동작 유지 (`files` 필드 omit). `--summary-only --status <filter>` 동시 명시 시 summary-only 정체성 우선, status filter 무시. AI 호출자가 한 호출로 "무엇이 실패했나" 명단 확인 가능 — Trees + 추가 scan 호출 2회 부담 해소.
- Schema v1.4 → v1.5 minor bump — F3 한정 caller-visible behavior change. 전체 모드 (`--summary-only` 미지정 시) wire shape 변경 0 — v1.4와 byte-identical (`schema_version` 값만 다름). summary-only 응답에서 `files == null` 또는 key 부재를 가정한 caller v1.4 분기는 failed N건 케이스에서 깨질 수 있음 (migration guide는 `spec-output-schema.md` § v1.4 → v1.5 변경 § backward-compat 표 참조). v1.0 / v1.1 / v1.2 / v1.3 / v1.4 backward-compat lock test 갱신 (task M).
- `commands/scan/summary_view.rs` helper 모듈 신규 — `strip_to_summary_failed` 함수 (`FileEntry` → `path` + `presence` + `failed_reason` 3 field minimal). build_report inline 분기 시 LOC 300 cap 위험 회피 + 단일 책임 helper 분리.

### Changed

- `schema_version` `"1.4"` → `"1.5"`.
- release: v0.4.2 → **v0.5.0** minor (schema bump 동반). v0.4.3 patch 회피 — schema 버전이 호출자 contract 일급 signal이라 SemVer minor가 정직.

### Spec

- `spec-cli-interface.md` § Acceptance Criteria — `cargo run -- --help` 에 scan / diff 서브커맨드 description 한 줄 노출 acceptance 추가 (F1) + § init subcommand § --help description 첫 줄 byte-identical 정합 acceptance 추가 (F2, "Emit gitless-sync.toml body from input args (stdout)").
- `spec-output-schema.md` § `--summary-only` 출력 + § v1.4 → v1.5 변경 + § v1.5 신규 Acceptance Criteria (5 시나리오) — F3 본진 결정 trail. summary-only mode contract 확장 + minimal entry shape lock + backward-compat 표.

### Verified

- 신규 / 갱신 test 시나리오 — `tests/cli_help_about.rs` 신규 (F1/F2 about description 한 줄 정합) + `commands/scan/output.rs::tests` + `commands/scan/status_filter.rs::tests` F3 4 시나리오 (summary-only + failed 0 → files omit / failed 1 → 3-field row / failed N + identical M → len == N / `--status drift` override 시 failed 등장) + `commands/scan/summary_view.rs::tests` helper strip contract (sha/size/mode/diff_meaningful/lfs_pointer/size_bytes 모두 None 확인) + `tests/scan_output_backward_compat.rs` v1.5 lock (`V15` client struct + `v1_5_sample_json` fixture + v1.0 ~ v1.4 client × v1.5 sample unknown field 0 보장) + `tests/scan_summary_only_failed.rs` F3 integration (long_path fixture, `--summary-only` 시 summary.failed > 0 + files[] entry 3 field 검증).
- Hard gate full pipeline PASS — `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo xtask check-line-limits` / `cargo xtask check-cycles` / `cargo machete` / `cargo test --workspace` / `cargo tarpaulin --engine llvm --workspace --out Stdout`.
- 485 + 88 unit + integration test pass / 0 failed (gitless-sync workspace + xtask).
- tarpaulin 88.98% (1324/1488 lines).
- post-v0.4.2 vault dogfood feedback F1 / F2 / F3 Improvement 후보 3건 friction 모두 해소 (vault 측 도그푸딩 motivation 정합).

## [0.4.2] - 2026-05-11

> Issue #1 hotfix. byte-identical files (UTF-8 BOM / LF-CRLF cosmetic SHA drift) 가 `LocalOnlyChanged` 로 잘못 분류되던 spec/code drift fix. `classify` 함수에 `normalize_equal: Option<bool>` 인자 추가 + sha-differ + `Some(true)` → `Status::Identical` arm. schema_version 1.3 → **1.4** (additive 의미 정확화, backward compat 보장). 결정 trail은 `docs/adr/0015-cosmetic-identical-classification.md`.

### Fixed

- **Issue #1**: byte 동일 파일이 `status: local_only_changed` + `presence: both` + `diff_meaningful: false` 로 잘못 분류되던 bug fix. `pipeline::normalize_pass` 가 sha-differ Hashed entry 한정 fetch_blob + 자체 hash 재계산 결과를 `compare()` 의 `diff_meaningful` 결정에만 박았고 `classify()` 의 status 결정에는 안 박혔던 비대칭 처리 정정. 이제 `normalize_equal == Some(true)` 시 `Status::Identical` 분류.

### Changed

- `compare/decisions.rs::classify` signature — `normalize_equal: Option<bool>` 5번째 인자 추가. caller 1건 (`pipeline/finalize/pre_entry.rs::hashed_to_file_entry`) 갱신 + 기존 unit test 11건 갱신 (`None` 추가).
- `output.rs::SCHEMA_VERSION` 1.3 → 1.4.
- `hash_remote.rs` 모듈 코멘트 정정 — outdated ("scan never calls fetch_blob") → 정확 ("normalize_pass 에서 sha-mismatch 시 fetch_blob 호출").
- `Cargo.toml` workspace.package — 본 release 는 crate version 만 (0.4.1 → 0.4.2).

### Spec

- `spec-classification.md` § Status 정의 + § classify 시그니처 + § 판정 로직 + § Acceptance Criteria 갱신.
- `spec-output-schema.md` § 안정성 보장 version history + § v1.4 신규 acceptance section 추가.
- `spec-hash-and-normalize.md` § 원격 측 비교 정확화 (1차 raw SHA + mismatch 시 자체 hash 재계산 흐름 명시).
- `docs/adr/0015-cosmetic-identical-classification.md` 신규 — 결정 trail + 4 alternative 검토.

### Verified

- 신규 regression test 3건:
  - `compare/decisions.rs::tests::identical_when_normalize_equal_despite_sha_differ` — F1 unit test.
  - `compare/decisions.rs::tests::normalize_equal_some_false_falls_through_to_timestamp_arm` — `Some(false)` 시 기존 동작 유지.
  - `pipeline/finalize/pre_entry.rs::tests::scenario_byte_identical_with_cosmetic_sha_differ_classifies_as_identical` — 통합 시나리오.
- backward compat — v1.0 / v1.1 / v1.2 / v1.3 caller 가 v1.4 JSON 파싱 정상 (status enum 그대로, 의미 정확화 additive).

## [0.4.1] - 2026-05-10

> Phase 8 post-eval minor fix. v0.4.0 가 도입한 clap `value_enum` (F5) 부산물 회복 + F4 백틱 노이즈 정리. 외부 surface 변경: 잘못된 인자 시 사람용 multi-line text + exit 2 → JSON 한 줄 + exit 1 (CONFIG_ERROR) 로 복귀 — v0.3 이전 한 줄 JSON contract 와 의미 일관.

### Changed

- **F5 contract 회복** — `Cli::parse()` → `Cli::try_parse()` + `main::map_clap_parse_error` helper. clap argument-parse 실패 (예: `--status drif`) 가 `GitlessError::Config(_)` 로 wrap 되어 기존 `to_stderr_payload()` 경로로 한 줄 JSON (`{"error_code":"CONFIG_ERROR","message":"..."}`) + exit 1 contract 회복. `message` 안에 clap multi-line 출력 (escape 처리, valid 후보 5개 + did-you-mean hint) 그대로 보존 — 정보 손실 0. `--help` / `--version` / `DisplayHelpOnMissingArgumentOrSubcommand` (bare invocation) 은 `None` 분기로 clap 기본 출력 (stdout + exit 0) 통과.
- **F4 백틱 노이즈 제거** — `main.rs` `#[arg]` doc comment 7곳에서 markdown 백틱 제거. clap 이 백틱을 raw 출력해 `--help` 에 `` `GitHub` `` / `` `JSON` `` 등이 노출되던 cosmetic noise 해소. lib internal doc 은 유지.
- **main.rs 분해** — `dispatch` + `emit_error` helper 분리 (clippy `too_many_lines` 60 cap 회복). `handle_clap_parse_error` signature `&clap::Error` (`needless_pass_by_value`).

### Spec

- `spec-error-contracts.md` § Custom Error Types Config(String) 의미에 clap argument-parse 실패 매핑 박음. § Acceptance Criteria 새 `[AUTO]` 시나리오 (잘못된 `--status` / `--help` / `--version` / bare invocation 분기 검증) 추가.

### Verified

- Hard gate full pipeline PASS — `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo xtask check-line-limits` / `cargo xtask check-cycles` / `cargo machete` / `cargo test --workspace` / `cargo xtask check-readme-examples` / `cargo tarpaulin --engine llvm --workspace --fail-under 80`.
- 484 + 88 unit + integration test pass / 0 failed (gitless-sync workspace + xtask).
- 새 unit test 4건 (`map_clap_parse_error` Display kind None × 3 + invalid status `Some(Config)` + 5 후보 + did-you-mean + exit/error_code 검증).
- tarpaulin 88.47% (1305/1475 lines).
- vault dogfood (KneShell/obsidian-vault, 363 files) — 4-state 281/60/22/0/0 + 0 drift + 0 failed v0.4.0 baseline 유지 (regression 0).

## [0.4.0] - 2026-05-10

> Phase 8 (LLM-as-Caller Usability Fix, eval 7 friction 해소) 누적. Phase 8.1 (spec/ADR 사전 확정) + Phase 8.2 (F1+F2 schema) + Phase 8.3 (F3 diff --json) + Phase 8.4 (F4/F5/F6 clap surface) + Phase 8.5 (F7 CI README sanity) + Phase 8.6 (release tag). 상세 task별 결과는 git history (`git log --grep="<task ID>"`) + `docs/ralph/implementation-plan.md`.

### Added

- Phase 8.2 — `files[].diff_meaningful: Option<bool>` + `files[].presence: "local_only"|"both"|"remote_only"` field 추가 (F1+F2). F1: sha differ but normalize-equal (e.g., CRLF vs LF) 케이스를 AI 호출자가 1 scan 호출로 판별 가능 — `diff_meaningful=Some(false)`로 불필요한 `diff` 호출 생략. F2: 4-state status 유지 + `presence` field로 방향 즉시 확인. Failed / local-or-remote-only entry는 `diff_meaningful=None`.
- Phase 8.3 — `diff --json` opt-in flag (F3). 기존 unified text output 기본 유지 + `--json` 분기 시 `{"side": "...", "unified": "..." | null, "raw": "..." | null, "binary": bool}` 직렬화. AI 호출자가 파싱 없이 구조화 diff 수신 가능.
- Phase 8.4 — clap surface 개선 3종 (F4/F5/F6). F4: 각 flag `///` doc comment 1줄 (--help 가독성). F5: `--status` `value_enum` derive + `value_delimiter = ','` (valid 후보 자동 노출 + 에러 친화). F6: `--branch` `default_value = "main"` (--help에 기본값 노출).
- Phase 8.5 — `cargo xtask check-readme-examples` sub-command 신규 (F7). README Quick Start 코드블록 실제 실행 + exit 0 검증. CI hard gate 합류.
- Schema v1.2 → v1.3 minor bump — `files[].diff_meaningful: Option<bool>` + `files[].presence: enum` 신규 field. v1.0/v1.1/v1.2 backward-compat 유지 (lock test 갱신).
- ADR 0014 'scan-diff metadata contract' 신규 — F1+F2 묶음 결정 trail.

### Changed

- schema_version `"1.2"` → `"1.3"`.

### Verified

- eval 7 friction (P0~P3) 해소 + Phase 8.5 regression baseline (`cargo xtask synth-vault --count 1000 --seed 42` — 추가 field가 4-state breakdown 유지 검증). 상세 결과: `docs/research/phase8-regression.md`.

### Known limitations (v0.5+ 해소 예정)

- sub-tree fallback real public repo dogfood 부재 — git/git는 truncation 영역 외, linux/torvalds는 budget 1000 cap 위반. 현 baseline은 unit/integration test (Phase 7.1 task F/G) cover. budget 정책 진화 task 도입 시 재검토.
- hash phase instrumentation 부재 — ADR 0008 § Phase 7.3 재검토 mtime cache 재도입 트리거 (a)/(b) 정량 verify에 필요. yagni 일관 deferred — 별도 instrumentation work 도입 시점에 검토.

## [0.3.0] - 2026-05-10

> Phase 7 (vault scale + Trees sub-tree + 큰 파일 임계치) 누적. Phase 7.1 (Trees sub-tree fallback) + Phase 7.2 (큰 파일 임계치) + Phase 7.3 (vault scale dogfood, T main bench + U public repo cross-check + V mtime cache keep-drop confirm + W 종합) + Phase 7.4 (release tag). 상세 task별 결과는 git history (`git log --grep="<task ID>"`) + `docs/ralph/implementation-plan.md`.

### Added

- Phase 7.1 — Trees sub-tree 재귀 fallback (G-002 해소). `truncated:true` 응답 시 ref → commit → root tree 1회 resolve 후 sub-tree non-recursive fan-out으로 내려가며 entries 합산 (`shared/github/trees/fallback.rs`). Cap: call budget 1000 + entries 500_000. Cap 초과 또는 fallback 실패 시 `GitlessError::TreesTruncated` + exit code 5 (정책 일관, 부분 결과 사용 금지). 정상 path는 v0.2.x 동작 유지.
- Phase 7.2 — 큰 파일 임계치 2 reason (`file_too_large` ≥ 100 MB GitHub Blobs API hard limit / `memory_exceeded` ≥ 50 MB tool 메모리 안전 임계). `commands/scan/hash_local.rs::try_hash_local`가 `fs::metadata().len()` pre-flight + 분기, `shared/github/blobs.rs::fetch_blob_with_size_gate`가 Trees response size field로 fetch 전 분기 (skip 시 fetch_blob 호출 0회). cascade는 short_circuit.rs LFS 다음 우선순위 9 reason로 갱신.
- Schema v1.1 → v1.2 (minor bump) — `failed_reason` enum 9 → 11 (`file_too_large` + `memory_exceeded` 추가) + 신규 field `files[].size_bytes` (`Option<u64>`, `#[serde(skip_serializing_if = "Option::is_none")]`). `file_too_large` / `memory_exceeded` entry는 size_bytes 포함 + `is_binary: false` (size pre-flight short-circuit, local read 전 격하). 그 외 entry는 size_bytes omit. v1.0 / v1.1 backward-compat 유지 (lock test `output.rs::tests` 갱신 — 이전 envelope/entry 필수 field invariant 박힘).
- `shared/github/trees/parse.rs::TreeEntry::size` — `Option<u64>` field 추가 (Trees response size field 활용, sub-tree fallback + remote-side size pre-flight 양쪽 사용).
- Specs — spec-github-api.md § Trees truncation handling + spec-hash-and-normalize.md § Phase 7 — 큰 파일 처리 + spec-output-schema.md § v1.2 + 신규 Acceptance Criteria 7 시나리오.
- ADR — 0011 (Trees sub-tree fallback) + 0012 (큰 파일 임계치 100/50 MB) + 0013 (자율 chain hard cap depth 3 + token 200k + wall-clock 6h). ADR 0008 § Phase 7.3 재검토 추가 — 1000+ path scale에서 mtime cache 재도입 트리거 keep-drop 유지 (path 20× scale에도 walltime 1324.8 ms → 829/1109 ms로 hash 비중 폭증 신호 없음, 향후 trigger (a)/(b) 명시).
- guardrails — G-019 (자율 chain hard cap, Phase 7 결정).
- Phase 7.3 — `xtask synth-vault` sub-command (1000+ markdown 합성 vault generator, seed/UTF-8 NFC/LF/mtime epoch 정책 정합, `xtask/src/synth_vault.rs`). dogfood + scale 측정용 — 도구 본체 contract 영향 0.
- Research — `docs/research/phase7-vault-scale-bench.md` (T main bench 1000 local × 129 remote 3 runs + U public repo cross-check 1000 local × 4964 remote git/git@94f0577 single run + W 종합 cross-comparison + ADR 0008 cross-link).

### Changed

- G-002 (Trees API truncation) — v0.2.x "즉시 fail" 정책 → v0.3 sub-tree fallback 도입. cap 초과 또는 fallback 실패 시에만 `GitlessError::TreesTruncated` + exit code 5.
- `compare.rs::FailedReason` enum 8 variant → 10 variant (`FileTooLarge` + `MemoryExceeded` 추가). `None` special case (`hash_io`)는 그대로 — 11 wire reason 모두 cover.

### Verified

- Phase 7.1 unit test 2 시나리오 (call budget 1001 / entries 500_001 cap trip) + integration test (multi-layer truncated descent → 합산 ScanReport).
- Phase 7.2 unit test 4 시나리오 (49MB local hash 정상 / 51MB memory_exceeded / 101MB file_too_large / 30MB LFS pointer 우선순위).
- Schema v1.2 acceptance 7 시나리오 unit test (`output.rs::tests`) — schema_version "1.2" + `FailedReason` 11 wire snake_case + size_bytes 정확 직렬화/omit + size_gate entry `is_binary: false` + v1.0/v1.1 envelope+entry 필수 field 박힘 invariant.
- Phase 7.3 vault dogfood — T main bench (1000 local markdown × 129 remote KneShell/gitless-sync, 3 runs cold+2warm, mean 829 ms, exit 0, failed 0, schema v1.2) + U public repo cross-check (1000 local × 4964 remote git/git@94f0577, 1 manual sanity run 1109 ms, exit 4 PARTIAL_FAILURE, failed 4 = 3 symlink `120000` + 1 submodule `160000` git/git remote-side, schema v1.2). Cross-comparison — remote 38× scale-up (129 → 4964)에 walltime ~+35% 증가 그침 (sub-linear, GraphQL batching + rayon 8c local hash 흡수). ADR 0008 § Phase 7.3 재검토 — mtime cache keep-drop 유지 (path 20× scale에도 walltime 폭증 신호 없음, hash phase instrumentation 부재로 정량 verify 불가 → yagni 일관). 종합 분석 + open items: `docs/research/phase7-vault-scale-bench.md` § 종합 (task W).

### Known limitations (v0.4+ 해소 예정)

- sub-tree fallback real public repo dogfood 부재 — git/git는 truncation 영역 외, linux/torvalds는 budget 1000 cap 위반. 현 baseline은 unit/integration test (Phase 7.1 task F/G) cover. budget 정책 진화 task 도입 시 재검토.
- hash phase instrumentation 부재 — ADR 0008 § Phase 7.3 재검토 mtime cache 재도입 트리거 (a)/(b) 정량 verify에 필요. yagni 일관 deferred — 별도 instrumentation work 도입 시점에 검토.

## [0.2.1] - 2026-05-10

> Phase 5 (도메인 함정 정리) + Phase 5.13/5.13.1/5.14 (follow-up) + Phase 6.1 (v0.2.x cleanup) 누적. v0.2.0은 ADR 0002 (gh subprocess migration) tag로 이미 사용. 본 release는 v0.2.x sub-phase patch increment. 상세 task별 결과는 git history (`git log --grep="<task ID>"`) + `docs/ralph/implementation-plan.md`.

### Added

- 8 핵심 함정 detect/handle — NFC normalize / case_collision (3 시나리오) / `encoding_rs` sniff (UTF-8 + Shift_JIS/EUC_KR/GBK/Windows-1252 + binary fallback, hash 입력은 항상 raw bytes — b-policy) / submodule (160000) detect-only / symlink (120000) lstat-only / 빈 파일 실파일 검증 (G-010) / executable (100755) mode bit / `.gitattributes` 파서 (project root + 하위 1회 로드 + glob + 가장 깊은 winner).
- 추가 함정 4건 — UTF-8 BOM strip + UTF-16 BOM (`FF FE`/`FE FF`) → `failed_reason: "encoding"` / git LFS pointer (`filter=lfs` path 자동 Failed + diff first-line signature 검증) / Windows long path 260+ + 예약 파일명 (`CON`/`PRN`/`NUL`/`AUX`/`COM1-9`/`LPT1-9`) / `.gitignore` 무시 정책 spec 명시.
- `.gitattributes` 화이트리스트 5 entry — `text=auto` / `binary` / `eol=lf` / `eol=crlf` / `filter=lfs`. 그 외 (`working-tree-encoding`, `ident`, `filter=*` (lfs 외), macro, legacy `crlf`) → `Unsupported` + `Status::Failed`.
- Schema v1.1 (minor bump) — 신규 field `mode` (4-digit octal: `100644`/`100755`/`120000`/`160000`) + `failed_reason` (9 enum, skip_serializing on `None`) + `lfs_pointer` (skip on `None`). v1.0 backward-compat lock test (`output.rs::tests` 5 lock).
- Dependencies — `unicode-normalization = "0.1"` (NFC) + `encoding_rs = "0.8"` (Apache-2.0/MIT). cargo-bloat `.text` attribution 0 KiB (LTO + strip + dead code elim).
- Specs — `docs/specs/spec-domain-pitfalls.md` (Phase 5 함정 spec hub).
- Research — `docs/research/phase5-{vault-baseline, vault-after, regression, gitattributes-bench, scan-scale-bench}.md` + `encoding-library-eval.md` + `phase4-measurements.md` (ADR 0003/0006/0007/0008 raw data hub).
- Phase 5.13/5.13.1 follow-up — `failed_reason` 3건 (`encoding`/`nfd_collision`/`gitattributes_unsupported`) caller plumbing 완성 + `shared/github/trees/` module 폴더 분할 + `commands/scan/pipeline/` module 폴더 분할 + sibling test 5 file 제거 + tmp/ + 외부 worktree 정리 + `.gitignore`에 `tmp/` 추가.
- Phase 5.14 (md 자료 audit) — CLAUDE.md "### 메모리 환경" section 제거 (privacy critical, vault path/admin username 노출 제거) + CLAUDE.md slim 142→45 LOC + CHANGELOG.md slim 159→76 LOC + ADR (0001~0009) audit + research/specs/ralph privacy 일반화 (vault path/`<project root>` placeholder).
- Phase 6.1 (v0.2.x cleanup) — ADR 0010 (cognitive_complexity vs LOC 300 orthogonal proxy 둘 다 유지 결정) + CI runner Linux 전환 (`windows-latest` → `ubuntu-latest`, 비용 + cold-start 둘 다 우위, G-018 cross-platform cfg gate 신규 발견 + fix).
- guardrails — G-016 (`cargo fmt --check` 무조건) + G-017 (gh `-F` GET→POST 자동 전환) + G-018 (cross-platform cfg gate — Windows-only `use`/test).
- CI gate Linux runner — `.github/workflows/ci.yml` 4 게이트 (`fmt --check` / `clippy -D warnings` / `test --workspace` / `tarpaulin --engine llvm --fail-under 80`). public-api diff PR-only.

### Changed

- `prepare_for_hash` 시그니처 — `gitattr: &Arc<GitAttributes>` + `path: &str` 추가. caller 모두 갱신.
- `status="failed"` 의미 확장 — v0.1 = "hash IO 실패"만, v0.2 = 9 reasons (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported`). caller가 `failed_reason` 분기 필요. 9 reason 모두 caller plumbing 완성 (Phase 5.13.1 EE/FF/GG).
- default LF normalize — v0.1 = 항상, v0.2 = `.gitattributes` conditional + 화이트리스트 5 entry. `Unspecified` (default) branch는 v0.1 정책 그대로 유지.
- implementation-plan.md skeleton inline slim — 440→246 LOC (completed task `결과` paragraph + sub-bullet 제거, header만 retain).

### Fixed

- G-005 mtime 휴리스틱 정합 — `local_mtime == remote_last_commit_at` 동률은 `Status::Drift`로 격하 (각 함정 처리 후 정합 재검증).
- hash_pass.rs PreState doc comment 3줄 중복 (외부 시각 AUTO-FIX).
- CLAUDE.md G-017 stale cross-ref + research 3 file 절대 경로 (`D:\00.Projects\02.Personal\05.gitless-sync` → `<project root>`).
- scan_errors.rs:13 `use std::fs;` cfg(windows) gate (Linux CI unused import) + compute.rs:267 backslash test cfg(windows) gate (Linux invalid path).

### Verified

- vault dogfood (T) — KneShell/gitless-sync@main 117 files / 0 drift / 0 failed.
- v0.1 baseline regression diff (W) — REGRESSION 0건 (envelope schema_version + mode field 정확화만, 121/121 path binary delta 0).
- 410+ tests pass + tarpaulin 90.57% (Phase 5 baseline 90.73% 대비 -0.16% 자연 변동).
- Linux CI runner 1차 검증 통과 (commit `87d2cf4`, 19m7s).

### Known limitations (v0.3+에서 해소 검토)

- vault scale 1000+ path dogfood 미측정 — Phase 5/6.1까지는 13~117 path scale 한정. Phase 7 진입 시 측정 + mtime cache 재도입 트리거 검토 (ADR 0008 § Future work).
- Trees API truncated repo 미지원 (G-002, 7MB or 100K entry 한도) — Phase 7+ sub-tree 재귀 fallback 도입 검토.
- 큰 파일 임계치 미정 (10MB+ 메모리 + Phase 4 cache 연결) — Phase 7+ 도입 검토.

## [0.1.0] - Phase 6 완료 시점 (2026-05-09)

> Phase 1 ~ Phase 6 누적 (v0.1 + Phase 4 GraphQL batching + Phase 6 Code Quality).

### Added

- CLI 명령어 — `scan` (디렉토리↔repo 4-state JSON 출력) + `diff` (단일 파일 차이) + `init` (`gitless-sync.toml` stdout TOML emit).
- 분류 — 4-state (`identical`/`local_only_changed`/`remote_only_changed`/`drift`/`failed`) + 시간 비교 휴리스틱 (G-005) + Schema v1.0 (호출자 backward-compat 보장).
- Hash & Normalize — 자체 정의 SHA-1 (`SHA-1("blob <size>\0<normalized content>")`, git 표준 blob SHA 아님 — G-001) + LF normalize + UTF-8 BOM strip (`--keep-bom` 시 보존) + binary 휴리스틱 (NUL byte 8000 안).
- GitHub API — `gh` CLI subprocess 단일 통로 (ADR 0001/0002) + REST + GraphQL (alias batching, default since Phase 4 — ADR 0006) + `MockGhClient` trait inject.
- Quality Gates (Phase 6) — clippy 60/15/5 deny + panic 검출 (`unwrap_used`/`expect_used`/`panic`, tests 면제) + LOC 300 게이트 + cycle/cross-slice 0건 + 외부 도구 (cargo-public-api / machete / tarpaulin / deny / audit) + CI gate.

### Architecture decisions

- Read-only 영구 (ADR 0001) + Vertical slice + cross-slice 직접 ref 금지 + slice 안 acyclic + directional discipline (orchestrator → domain → IO) + Windows 1차 (실행 환경) + MSRV 1.95.0 stable + `#![forbid(unsafe_code)]` + release `panic = "abort"` + `lto = "thin"` + `strip = true`.

### Verified

- vault dogfooding 356 files / 0 drift (2026-04-29 ureq baseline).
- Phase 6 baseline: 244 tests pass (174 lib + 21 integration + 49 xtask) + tarpaulin 88.31%.

### Known limitations (v0.2에서 해소)

- 도메인 함정 8건 + 추가 함정 4건 (UTF-16 BOM/LFS pointer/Windows long path/`.gitignore` 정책).

### Excluded (영구 비목표)

- 3-way merge / 양방향 동기화 / 인터랙티브 UI / GitHub 외 호스팅 (GitLab/Bitbucket) / LFS 추적 파일 직접 fetch (pointer detect-only) / Event 기반 layer 통신 (channel/observer/actor).
