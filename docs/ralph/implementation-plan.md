# Implementation Plan

## Status
- Last updated: 2026-05-06T00:00:00Z (ADR 0002 마이그레이션 task 박힘)
- Total tasks: 8 (M0~M7)
- Completed: 0 / 8

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵된 상태.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 해당 spec 파일과 정확히 매핑되어야 함. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).

## Dependency Graph (요약)

```
M0 (trait 인터페이스 + spec-github-api 재작성)
   ↓
M1 (에러 매핑 spec 갱신)
   ↓
M2 (gh wrapper 구현 + 단위 테스트, ureq+mockito 제거)
   ├──→ M3 (--token 제거 + spec-cli/spec-config 슬림화)
   │       ↓
   │      M4 (통합 테스트 재작성)
   │       ↓
   ├──→ M5 (rayon 유지/제거 측정·결정)──┐
   ├──→ M6 (README + 의존성 안내)──────┤
   │                                    ↓
   └────────────────────────────────→ M7 (빌드 게이트 검증)
```

## Tasks

### M0. gh subprocess 호출 trait 인터페이스 + spec-github-api 통째 재작성
- **Spec reference**: `docs/specs/spec-github-api.md` (재작성 대상), `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md`
- **Files**: `docs/specs/spec-github-api.md` (갱신). 코드 변경 없음 (spec-only task).
- **Depends on**: none
- **Acceptance criteria**:
  - `[AUTO]` `pub(crate) trait GhClient` 정의 박제: `fn api(&self, args: &[&str]) -> Result<GhResponse, GitlessError>`. `GhResponse`는 최소 `{ stdout: Vec<u8>, stderr: String, exit_code: i32 }` 캡처.
  - `[AUTO]` 두 구현 명시: `RealGhClient` (production, `std::process::Command::new("gh")`)와 `MockGhClient` (테스트, 인자별 응답을 미리 등록한 HashMap 또는 클로저). 위치는 `commands/scan/github.rs` (또는 별도 `shared/gh.rs` — M2에서 결정).
  - `[AUTO]` `gh` PATH 미존재 시 첫 호출에서 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환. 검증 시점(`new()` vs 첫 호출) 명시.
  - `[AUTO]` 호출 인자 패턴 박제. Trees: `gh api repos/{owner}/{repo}/git/trees/{branch}?recursive=1`. Blobs: `gh api repos/{owner}/{repo}/git/blobs/{sha}`. Commits: `gh api repos/{owner}/{repo}/commits -F sha={branch} -F path={path} -F per_page=1`.
  - `[AUTO]` Binary entry point 시그니처 결정: `commands/scan/mod.rs::run_with_client(args: &ScanArgs, client: &impl GhClient) -> Result<...>` 형태. main.rs는 `RealGhClient`를 inject. M4 통합 테스트는 `MockGhClient` inject. 시그니처를 spec에 박음 (구현은 M2).
  - `[AUTO]` `spec-github-api.md` 통째 갱신: ureq/mockito/Agent thread-safety/HTTP 헤더 송신 표현 제거. `GhClient` trait 기준으로 § 목적 / § 현재 상태 / § 작업 범위 / § Acceptance Criteria 재작성. § 병렬 호출 정책은 M5 결과 미정 상태로 "rayon 유지 여부 미정 (M5에서 결정)" 박스 유지.
  - `[AUTO]` § Backend 선택 섹션은 그대로 유지 — `--backend rest` 의미가 "REST 단건 N×" 그대로, `--backend graphql`은 Phase 4 stub 그대로 (호출 통로만 ureq → gh로 변경).
  - `[AUTO]` 사람 검토 후 다음 task 진입.
- **Status**: `[ ]`

### M1. 에러 매핑 표 박제 (gh 종료 코드 + stderr → GitlessError)
- **Spec reference**: `docs/specs/spec-error-contracts.md` (부분 갱신), `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 에러 매핑
- **Files**: `docs/specs/spec-error-contracts.md` (갱신). 코드 변경 없음.
- **Depends on**: M0
- **Acceptance criteria**:
  - `[AUTO]` gh exit code + stderr 패턴 표 조사 박제. 출처 명시 (gh 공식 docs URL + 직접 호출 관찰). 최소 케이스: 정상(0), gh 미설치(`Command::new` IO err), 인증 실패(gh stderr `HTTP 401` 또는 `gh auth status` fail), rate limit(gh stderr `API rate limit exceeded`), Trees truncated(stdout JSON `truncated: true` 파싱), 5xx(gh stderr `HTTP 5xx`), 기타.
  - `[AUTO]` `spec-error-contracts.md` § 인증 실패 / Rate Limit / Trees Truncated 동작 섹션의 매핑 source를 "ureq 응답" → "gh stderr/stdout 패턴"으로 갱신. exit code 매핑 표(0~5)는 그대로 유지.
  - `[AUTO]` Acceptance Criteria 섹션의 mockito 시나리오(시나리오 11/12/15)를 "MockGhClient stub 응답" 표현으로 재작성. exit code + stderr `error_code` 검증은 그대로.
  - `[AUTO]` § Custom Error Types에 `Http(String)` variant의 의미를 "gh subprocess 비정상 종료(인증/rate/truncated 외)" 로 보강.
- **Status**: `[ ]`

### M2. gh wrapper 구현 + 단위 테스트 (commands/scan/github.rs 재작성)
- **Spec reference**: `docs/specs/spec-github-api.md` (M0 갱신본), `docs/specs/spec-error-contracts.md` (M1 갱신본)
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs` (전면 재작성), `crates/gitless-sync/Cargo.toml` (`ureq` + `mockito` 의존성 삭제).
- **Depends on**: M0, M1
- **Acceptance criteria**:
  - `[AUTO]` `GhClient` trait + `RealGhClient` + `MockGhClient` 구현 (M0 spec 따라).
  - `[AUTO]` `fetch_tree(client: &impl GhClient, repo, branch) -> Result<Vec<RemoteFile>, GitlessError>` 재작성. blob entry만 필터, mode `100755`/`120000`/`160000` skip + stderr warning. `truncated: true` 감지 → `TreesTruncated`.
  - `[AUTO]` `fetch_blob(client: &impl GhClient, repo, sha) -> Result<Vec<u8>, GitlessError>` 재작성. base64 디코딩.
  - `[AUTO]` `fetch_last_commit_at(client: &impl GhClient, repo, branch, path) -> Result<DateTime<Utc>, GitlessError>` 재작성. 첫 commit의 `commit.committer.date` 파싱.
  - `[AUTO]` 단위 테스트는 모두 `MockGhClient` 사용. mockito 호출 0회. PRD 시나리오 11/12 단위 케이스도 mock 응답으로 재현.
  - `[AUTO]` 에러 매핑은 M1 spec의 표 그대로. 인증/rate/truncated/5xx/parse 케이스 모두 단위 테스트 커버.
  - `[AUTO]` `Cargo.toml`에서 `ureq`, `mockito` 의존성 삭제. `Cargo.lock` 갱신.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` 통과.
  - `[AUTO]` `cargo deny check` + `cargo audit` 통과.
- **Status**: `[ ]`

### M3. CLI 인자 + config 토큰 경로 제거 (spec-cli/spec-config 슬림화)
- **Spec reference**: `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md` (둘 다 부분 갱신)
- **Files**: `crates/gitless-sync/src/main.rs`, `crates/gitless-sync/src/shared/config.rs`, `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md`.
- **Depends on**: M2
- **Acceptance criteria**:
  - `[AUTO]` clap 정의에서 `--token` 인자 필드 + `clap(env = "GITHUB_TOKEN")` 자동 처리 제거.
  - `[AUTO]` `shared/config.rs::resolve_token` 함수 + 관련 단위 테스트 삭제. `Config` 구조체에 token 필드 있으면 삭제.
  - `[AUTO]` `GITLESS_API_BASE` env 처리 코드 삭제 (M0 결정에서 새 testability env가 정의됐으면 그것으로 교체, 아니면 단순 삭제).
  - `[AUTO]` `spec-cli-interface.md`: 글로벌 플래그 표에서 `--token` 행 삭제. § 인자 우선순위에서 토큰 라인 제거. Acceptance Criteria의 `--token env:...`/`--token literal:...` 항목 삭제.
  - `[AUTO]` `spec-config.md`: § `--token` 형식 섹션 통째 삭제. § 우선순위 표에서 토큰 라인 삭제. § 비밀 정보 정책은 그대로 유지(여전히 valid). Acceptance Criteria의 토큰 관련 5개 항목(`--token env:`, `--token literal:`, 토큰 미설정 → AuthFailed) 삭제.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings` 통과.
- **Status**: `[ ]`

### M4. 통합 테스트 재작성 (tests/integration.rs)
- **Spec reference**: PRD 검증 시나리오 1~4, 9~15. `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 마이그레이션 작업 범위.
- **Files**: `crates/gitless-sync/tests/integration.rs` (전면 재작성). `crates/gitless-sync/Cargo.toml` dev-deps에서 `mockito` 제거 확인 (M2에서 이미 처리).
- **Depends on**: M3
- **Acceptance criteria**:
  - `[AUTO]` 모든 시나리오를 M0에서 정의한 `run_with_client(args, &MockGhClient)` 진입점 기반으로 재작성. `assert_cmd`로 binary 통째 실행하던 패턴은 (a) library 함수 직접 호출로 전환, 또는 (b) MockGhClient를 main에 inject할 수 있는 testability hook 추가 — 어느 쪽이든 M0 spec에 결정 박힘.
  - `[AUTO]` PRD 시나리오 1~4 (4상태 분류) end-to-end: tempfile 디렉토리 + `MockGhClient` stub 응답 → 4상태 카운트 검증.
  - `[AUTO]` 시나리오 9 (.gitignore + --ignore 합집합), 13 (`--summary-only`), 14 (`--status drift`), 15 (partial failure) 재현.
  - `[AUTO]` 시나리오 10 (인증 실패): `MockGhClient`가 gh stderr `HTTP 401` 흉내 → exit 2 + stderr `AUTH_FAILED` JSON.
  - `[AUTO]` 시나리오 11 (rate limit): `MockGhClient`가 gh stderr `API rate limit exceeded` 흉내 → exit 3 + stderr `RATE_LIMIT_EXCEEDED`.
  - `[AUTO]` 시나리오 12 (truncated): `MockGhClient`의 stdout JSON에 `truncated: true` → exit 5.
  - `[AUTO]` 시나리오에서 빠진 케이스: gh 미설치 시뮬레이션 1건 추가 (PATH에서 gh 제거 또는 `RealGhClient`를 가짜 PATH로 호출) — exit 1 + stderr `gh CLI not found...`.
  - `[AUTO]` `cargo test --test integration` 통과.
- **Status**: `[ ]`

### M5. rayon 유지 여부 측정 + guardrail 갱신
- **Spec reference**: `docs/ralph/guardrails.md` § G-011 (갱신/obsolete 대상), `docs/specs/spec-github-api.md` § 병렬 호출 정책 (M0에서 미정 상태로 박힘 → M5에서 확정)
- **Files**: `docs/ralph/guardrails.md`, `docs/specs/spec-github-api.md`, (제거 결정 시) `crates/gitless-sync/Cargo.toml` + `commands/scan/mod.rs`
- **Depends on**: M2
- **Acceptance criteria**:
  - `[AUTO]` 측정: 1000-path 규모 repo(또는 vault scale 100~300 path) 기준으로 (a) rayon 8 concurrent + gh subprocess vs (b) 순차 gh subprocess 시간 측정. 환경(Windows + 실제 vault) + 명령어 + 결과를 본 spec 또는 별도 메모에 박제.
  - `[AUTO]` 결정: ① rayon 유지(subprocess spawn 비용 << 순차 latency) 또는 ② rayon 제거(병렬 spawn 비용이 무시 못 함, gh 자체 retry/backoff로 충분). 결정 근거 한 단락.
  - `[AUTO]` 결정에 따라 처리:
    - 유지 시: G-011 갱신 — "rayon 8 concurrent는 gh subprocess 환경에서도 유지. abuse detection 회피 책임은 gh가 처리." `spec-github-api.md` § 병렬 호출 정책 확정 박제.
    - 제거 시: G-011 obsolete 마크. `Cargo.toml`에서 `rayon` 의존성 삭제. `commands/scan/mod.rs::run_with_client`의 `par_iter` → `iter`/`for` 변경. `spec-github-api.md` § 병렬 호출 정책 섹션 삭제.
  - `[AUTO]` `cargo test --workspace`, `cargo deny check`, `cargo audit` 통과.
- **Status**: `[ ]`

### M6. README + 의존성 안내
- **Spec reference**: `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 의존성 안내
- **Files**: `README.md` (현재 존재 여부 확인 후 신규 작성 또는 갱신).
- **Depends on**: M2
- **Acceptance criteria**:
  - `[AUTO]` README.md에 "Prerequisites" 섹션: `gh` CLI(>= 2.x) 설치 안내, `gh auth login` 한 줄 인증 안내. Windows(`winget install GitHub.cli`)/macOS(`brew install gh`)/Linux 설치 명령 박제.
  - `[AUTO]` 사용 예시 섹션의 `--token env:GITHUB_TOKEN` 등 토큰 인자 표현이 있으면 모두 제거. `gh auth login` 사전 실행 가정으로 단순화.
  - `[AUTO]` gh 미설치 시 에러 메시지 동작 검증: 임시 PATH(원래 PATH에서 `gh` 디렉토리 제외)로 `gitless-sync scan` 호출 → `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` + exit 1 확인. 검증 결과 한 줄 박제.
- **Status**: `[ ]`

### M7. 빌드 게이트 통과 검증
- **Spec reference**: `docs/ralph/project-ops.md` § Coverage, `CLAUDE.md` § Test coverage, G-007
- **Files**: 미달 모듈에 unit test 추가 (필요 시).
- **Depends on**: M2, M3, M4, M5, M6
- **Acceptance criteria**:
  - `[AUTO]` `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo audit` 모두 통과.
  - `[AUTO]` `cargo tarpaulin --engine llvm --workspace --out Stdout` 라인 커버리지 ≥ 80%. (이전 95.87%에서 mockito 제거 + gh wrapper 전환으로 변동 가능 → 측정 후 미달 시 보강.)
  - `[AUTO]` `cargo tree`로 `ureq`, `mockito` 의존성 부재 확인. (M5에서 rayon 제거 결정됐으면 rayon도 부재 확인.)
  - `[AUTO]` Windows 환경에서 LLVM 백엔드 false positive/negative 의심 시 G-007 참조.
- **Status**: `[ ]`
