# Implementation Plan: gitless-sync v0.1

## Status
- Last updated: 2026-04-28T00:00:00Z (initial draft, hand-written to skip ralph plan mode)
- Total tasks: 13
- Completed: 2 / 13

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵된 상태.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 해당 spec 파일과 정확히 매핑됨. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).

## Dependency Graph (요약)
```
L1 (leaf):     T01  T02  T03  T04
                |    |          |
L2 (L1 deps):  T05  T06  T07  T08
                            |
                          (T02 deps)
                            ↓
L3 (orchestr): T09 ──────── T10
                |            |
L4 (validate): T11 ── T12   |
                |            |
L5 (human):    T13          |
```

## Tasks

### T01. IgnoreMatcher 구현 + unit tests
- **Spec reference**: `docs/specs/spec-ignore-policy.md`
- **Files**: `crates/gitless-sync/src/shared/ignore.rs`
- **Depends on**: none
- **Acceptance criteria**:
  - `[AUTO]` `IgnoreMatcher::new(root, &[])`가 root에 `.gitignore` 없어도 성공.
  - `[AUTO]` builtin 패턴 매치: `is_ignored(Path::new(".git/HEAD")) == true`, `is_ignored(Path::new("node_modules/foo")) == true`.
  - `[AUTO]` `--ignore "*.log"` 인자 → `is_ignored(Path::new("debug.log")) == true`.
  - `[AUTO]` `.gitignore`에 `dist/` → `is_ignored(Path::new("dist/bundle.js")) == true`.
  - `[AUTO]` `.gitignore` + `--ignore` 합집합 동작 (tempfile 통합 테스트, PRD 시나리오 9).
  - `[AUTO]` 매칭 키는 forward slash (Windows 백슬래시 입력도 정규화).
  - `[AUTO]` `cargo test ignore` 통과.
- **Status**: `[x]`

### T02. config::load + Token 파싱 헬퍼
- **Spec reference**: `docs/specs/spec-config.md`
- **Files**: `crates/gitless-sync/src/shared/config.rs`
- **Depends on**: none
- **Acceptance criteria**:
  - `[AUTO]` `config::load(Some(toml_path))` 정상 TOML 파싱 → `Config`.
  - `[AUTO]` `config::load(None)` 또는 파일 없는 경로 → `Config::default()`.
  - `[AUTO]` 잘못된 TOML → `GitlessError::Config(...)`, exit code 1.
  - `[AUTO]` Token 파싱 헬퍼 (예: `resolve_token(&str) -> Result<String, GitlessError>`): `env:NAME` → env 변수 읽음, `literal:VAL` → 그대로.
  - `[AUTO]` env 변수 미설정 시 `GitlessError::AuthFailed` (exit 2). (PRD 시나리오 10 일부)
  - `[AUTO]` 우선순위 검증 헬퍼 또는 `scan::run`에서 처리 — CLI > env > toml > 기본값.
  - `[AUTO]` `cargo test config` 통과.
- **Status**: `[x]`

### T03. shared 단위 테스트 보강 (hash + normalize)
- **Spec reference**: `docs/specs/spec-hash-and-normalize.md`
- **Files**: `crates/gitless-sync/src/shared/hash.rs`, `crates/gitless-sync/src/shared/normalize.rs`
- **Depends on**: none
- **Acceptance criteria**:
  - `[AUTO]` `hash::tests::crlf_normalizes_to_lf` — `blob_hash(normalize(b"hello\r\n", false))` == `blob_hash(b"hello\n")`. (PRD 시나리오 5)
  - `[AUTO]` `normalize::tests::strips_bom_when_keep_bom_false` — BOM 시작 입력에 false → BOM 제거. (PRD 시나리오 6)
  - `[AUTO]` `normalize::tests::keeps_bom_when_keep_bom_true` — BOM 시작 입력에 true → BOM 보존. (PRD 시나리오 7)
  - `[AUTO]` `normalize::tests::detects_binary_with_nul_byte` — NUL 포함 입력 → `is_binary == true`.
  - `[AUTO]` `normalize::tests::prepare_for_hash_returns_correct_flag` — binary 입력 `(_, true)`, text 입력 `(_, false)`.
  - `[AUTO]` `hash::tests::same_binary_same_sha` — 동일 raw bytes → 동일 SHA. (PRD 시나리오 8)
  - `[AUTO]` `cargo test --workspace` 통과. PRD 시나리오 5~8 단위 테스트로 모두 검증.
- **Status**: `[ ]`

### T04. compare::classify 4상태 판정
- **Spec reference**: `docs/specs/spec-classification.md`
- **Files**: `crates/gitless-sync/src/commands/scan/compare.rs`
- **Depends on**: none
- **Acceptance criteria**:
  - `[AUTO]` 양쪽 SHA 동일 → `Identical`. (PRD 시나리오 1)
  - `[AUTO]` `local_sha == Some + remote_sha == None` → `LocalOnlyChanged`. (PRD 시나리오 2 일부)
  - `[AUTO]` `local_sha == None + remote_sha == Some` → `RemoteOnlyChanged`. (PRD 시나리오 3 일부)
  - `[AUTO]` 양쪽 다른 SHA + `remote_last_commit_at < local_mtime` → `LocalOnlyChanged`. (PRD 시나리오 2)
  - `[AUTO]` 양쪽 다른 SHA + `local_mtime < remote_last_commit_at` → `RemoteOnlyChanged`. (PRD 시나리오 3)
  - `[AUTO]` 양쪽 다른 SHA + `local_mtime == remote_last_commit_at` → `Drift` (G-005). (PRD 시나리오 4)
  - `[AUTO]` 양쪽 다른 SHA + 한쪽 시간 None → `Drift`.
  - `[AUTO]` `compare::tests::*` 모든 케이스 + edge case 커버.
- **Status**: `[ ]`

### T05. walker::walk 디렉토리 순회
- **Spec reference**: `docs/specs/spec-ignore-policy.md` § 매칭 동작 + G-004
- **Files**: `crates/gitless-sync/src/commands/scan/walker.rs`
- **Depends on**: T01
- **Acceptance criteria**:
  - `[AUTO]` `walker::walk(root, &matcher)`가 ignored 파일을 결과에서 제외.
  - `[AUTO]` `LocalFile.relative_path`가 forward slash로 통일 (Windows 백슬래시 → 슬래시 변환). (G-004)
  - `[AUTO]` `LocalFile.mtime`이 `DateTime<Utc>` (chrono 변환 포함).
  - `[AUTO]` 빈 디렉토리는 결과에 포함 안 됨 (파일만 반환).
  - `[AUTO]` 심볼릭 링크는 v0.1에서 skip (G-010, Phase 5에서 처리).
  - `[AUTO]` `walker::tests::*` (tempfile 기반) 통과.
- **Status**: `[ ]`

### T06. github::fetch_tree + mockito tests
- **Spec reference**: `docs/specs/spec-github-api.md` § fetch_tree
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs`
- **Depends on**: T02
- **Acceptance criteria**:
  - `[AUTO]` mockito 200 응답 → `Vec<RemoteFile>` (blob entry만 필터, mode `100755`/`120000`/`160000` 제외 + stderr warning).
  - `[AUTO]` mockito `truncated: true` → `GitlessError::TreesTruncated` (G-002, PRD 시나리오 12).
  - `[AUTO]` mockito 401 → `GitlessError::AuthFailed`.
  - `[AUTO]` mockito 403 + `X-RateLimit-Remaining: 0` → `GitlessError::RateLimitExceeded { reset_at }` (PRD 시나리오 11).
  - `[AUTO]` mockito 5xx → `GitlessError::Http(...)`.
  - `[AUTO]` `User-Agent: gitless-sync/0.1` 헤더 송신 (mockito match로 검증).
  - `[AUTO]` `Authorization: Bearer <token>` 헤더 송신.
- **Status**: `[ ]`

### T07. github::fetch_blob + mockito tests
- **Spec reference**: `docs/specs/spec-github-api.md` § fetch_blob
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs`
- **Depends on**: T02
- **Acceptance criteria**:
  - `[AUTO]` mockito 200 base64 응답 → raw bytes 디코딩.
  - `[AUTO]` 잘못된 base64 응답 → `GitlessError::Http(...)`.
  - `[AUTO]` 인증·rate limit 매핑 T06과 동일 룰.
  - `[AUTO]` `User-Agent` + `Authorization` 헤더 송신.
- **Status**: `[ ]`

### T08. github::fetch_last_commit_at + mockito tests
- **Spec reference**: `docs/specs/spec-github-api.md` § fetch_last_commit_at
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs`
- **Depends on**: T02
- **Acceptance criteria**:
  - `[AUTO]` mockito 응답 첫 commit의 `commit.committer.date` → `DateTime<Utc>` 파싱.
  - `[AUTO]` 빈 commits 배열 응답 → `GitlessError::Http(...)`.
  - `[AUTO]` 인증·rate limit 매핑 T06과 동일.
- **Status**: `[ ]`

### T09. scan::run 오케스트레이터 + 필터 + verbose 플래그
- **Spec reference**: `docs/specs/spec-output-schema.md`, `docs/specs/spec-error-contracts.md`, `docs/specs/spec-cli-interface.md`
- **Files**: `crates/gitless-sync/src/commands/scan/mod.rs`, `crates/gitless-sync/src/main.rs`
- **Depends on**: T01, T02, T03, T04, T05, T06, T07, T08
- **Acceptance criteria**:
  - `[AUTO]` 흐름: `config::load` → token 결정 → `fetch_tree` → `walker::walk` → 각 파일 SHA 계산 → 차이 있는 파일에만 `fetch_last_commit_at` (G-003) → `classify` → `ScanReport` 조립 → `serialize` → stdout.
  - `[AUTO]` Commits API는 SHA 다른 파일에만 호출. identical에는 호출 금지.
  - `[AUTO]` `--summary-only` 시 `ScanReport.files = None` → 출력 JSON에 `files` 키 omit. (PRD 시나리오 13)
  - `[AUTO]` `--status drift,local_only_changed` 시 해당 status만 `files[]`. summary는 전체 카운트 유지. (PRD 시나리오 14)
  - `[AUTO]` `-v` (info) / `-vv` (debug) flag를 `main.rs`에 추가 + scan::run에서 stderr 로그 분기.
  - `[AUTO]` Partial failure: 해시 실패 파일은 `Status::Failed` + `summary.failed` 증가 + exit code 4. 전체 결과는 정상 출력.
  - `[AUTO]` 정상 종료 → exit 0. stdout JSON 한 덩어리 (`serde_json::from_str` 가능, G-008).
  - `[AUTO]` `cargo test scan` 통과.
- **Status**: `[ ]`

### T10. diff::run unified diff 출력
- **Spec reference**: `docs/specs/spec-cli-interface.md` § diff
- **Files**: `crates/gitless-sync/src/commands/diff/mod.rs`, (필요 시 Cargo.toml에 `similar` crate 추가)
- **Depends on**: T03, T07
- **Acceptance criteria**:
  - `[AUTO]` `<path>` 인자로 받은 파일을 로컬에서 읽고, 원격 blob을 `fetch_blob`로 받음 (Trees 조회로 SHA 먼저 알아내거나 `fetch_tree` 캐시 활용).
  - `[AUTO]` 양쪽 normalize 후 unified diff 형식으로 stdout 출력 (`similar` crate 권장 — 의존성 추가 시 `cargo deny check` 통과 확인).
  - `[AUTO]` 파일이 한쪽에만 있으면 stderr 메시지 ("(remote only)" / "(local only)") + 빈 stdout 또는 한쪽 내용 그대로.
  - `[AUTO]` 바이너리 파일이면 diff 안 하고 stderr 메시지 ("binary file, diff skipped"). exit 0.
  - `[AUTO]` `cargo test diff` 통과.
- **Status**: `[ ]`

### T11. End-to-end 통합 테스트 (PRD 검증 시나리오 자동화)
- **Spec reference**: 전 spec의 acceptance criteria 통합. 자동화 가능한 PRD 검증 시나리오 14항목 중 unit test로 안 잡히는 부분.
- **Files**: `crates/gitless-sync/tests/integration.rs` (새 파일), 필요 시 Cargo.toml dev-deps에 `assert_cmd` + `predicates` 추가.
- **Depends on**: T09, T10
- **Acceptance criteria**:
  - `[AUTO]` PRD 시나리오 1~4 (4상태 분류) end-to-end: tempfile 디렉토리 + mockito GitHub API → 실제 binary 실행 → stdout JSON 파싱 → 4상태 카운트 검증.
  - `[AUTO]` PRD 시나리오 9 (.gitignore + --ignore 합집합) end-to-end.
  - `[AUTO]` PRD 시나리오 10 (토큰 미설정 → exit 2 + stderr `AUTH_FAILED` JSON) — `assert_cmd`로 실제 binary 실행.
  - `[AUTO]` PRD 시나리오 11 (rate limit, exit 3 + stderr `RATE_LIMIT_EXCEEDED`) end-to-end.
  - `[AUTO]` PRD 시나리오 12 (truncated, exit 5) end-to-end.
  - `[AUTO]` PRD 시나리오 13 (`--summary-only` → stdout 출력에 `"files"` 문자열 부재).
  - `[AUTO]` PRD 시나리오 14 (`--status drift` → drift 항목만 files[]에 + summary는 전체).
  - `[AUTO]` PRD 시나리오 15 (partial failure → exit 4 + summary.failed > 0).
  - `[AUTO]` `cargo test --test integration` 통과.
- **Status**: `[ ]`

### T12. tarpaulin 80% 커버리지 게이트 통과
- **Spec reference**: `CLAUDE.md` § Test coverage, `docs/ralph/project-ops.md` § Coverage, G-007
- **Files**: 부족한 모듈에 unit test 추가 (T01~T11 완료 후 측정해서 결정)
- **Depends on**: T11
- **Acceptance criteria**:
  - `[AUTO]` `cargo tarpaulin --engine llvm --workspace --out Stdout` 라인 커버리지 ≥ 80%.
  - `[AUTO]` 미달 시 부족한 모듈에 unit test 추가하여 80% 도달. 어느 모듈이 부족한지는 tarpaulin 출력에서 확인.
  - `[AUTO]` 통합 테스트는 별도 카운트, 80% 게이트엔 미반영 (project-ops.md 정책).
  - `[AUTO]` Windows 환경에서 LLVM 백엔드 false positive/negative 의심 시 G-007 참조 + guardrails 갱신.
  - `[AUTO]` Baseline cleanup: `crates/gitless-sync/src/main.rs` 첫 두 줄의 TODO 주석 + `#![allow(dead_code, clippy::needless_pass_by_value)]` 제거 후 `cargo clippy --all-targets -- -D warnings` 재통과. 만약 잔존 dead_code가 잡히면 해당 함수가 진짜 unwired된 것이므로 별도 fix task를 plan에 추가 후 `[!]` BLOCKED 처리.
- **Status**: `[ ]`

### T13. [HUMAN] 실제 GitHub repo + Fine-grained PAT 통합 검증
- **Spec reference**: `docs/specs/spec-github-api.md` § Open Question, `docs/roadmap.md` § Open Questions
- **Files**: 코드 변경 없음. 사람이 수동 실행 + 결과를 `docs/roadmap.md`에 반영.
- **Depends on**: T09
- **Acceptance criteria**:
  - `[HUMAN]` Fine-grained PAT (`Contents: Read` 권한만)로 실제 GitHub repo에 `gitless-sync scan` 실행 → 정상 JSON 출력.
  - `[HUMAN]` Trees + Commits API 모두 작동 확인. 실패 시 더 넓은 권한 필요한지 확인.
  - `[HUMAN]` 결과를 `docs/roadmap.md` Open Questions 섹션에서 제거. 추가로 발견된 함정은 `guardrails.md`에 추가.
- **Status**: `[ ]`
