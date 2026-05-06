# Implementation Plan

## Status
- Last updated: 2026-05-06T19:00:00Z (M4a [~] 시작 — 통합 테스트 정상 경로 시나리오 1~4 + 9 재작성. lib target 도입 + 가시성 cascade.)
- Total tasks: 14 (M0, M1, M2a, M2b1, M2b2, M2c, M3, M4a, M4b, M5a, M5b, M6, M7, M8)
- Completed: 7 / 14

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵된 상태.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 해당 spec 파일과 정확히 매핑되어야 함. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- **모든 task는 `[AUTO]`** — 사람 개입 0으로 자율 루프 진행. 결정 항목은 `docs/specs/spec-github-api.md` § GhClient trait 사전 결정 등 사전 박음.
- **transient 실패 자동 회복**: G-015 (외부 명령 transient retry policy)로 박힌 [!] task는 `prompt-build.md` § 1 [!] auto-recovery 룰에 따라 다음 iteration 자동 [!]→[ ] reset. 사람 개입 0.

## Dependency Graph

```
M0 → M1 → M2a → M2b1 → M2b2 → M2c
                              ├──→ M3 → M4a → M4b
                              ├──→ M5a → M5b
                              └──→ M6
                                     ↓
                                   M7 → M8
```

## Tasks

### M0. spec-github-api 통째 재작성 `[AUTO, spec-only]`
- **Spec reference**: `docs/specs/spec-github-api.md` § GhClient trait 사전 결정 (baseline), `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` (L31 정렬)
- **Files**: `docs/specs/spec-github-api.md`, `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md`
- **Depends on**: none
- **Acceptance criteria**:
  - `[AUTO]` `spec-github-api.md` 통째 갱신: § 목적 / § 현재 상태 / § 작업 범위 / § Acceptance Criteria 재작성. ureq/mockito/Agent thread-safety/HTTP 헤더 송신 표현 제거.
  - `[AUTO]` § GhClient trait 사전 결정 섹션의 6개 결정을 본문 § 작업 범위로 옮긴 뒤, **사전 결정 섹션은 통째 제거** (historical mark도 남기지 않음 — 양자택일 fixpoint 단조화 / P1 권고).
  - `[AUTO]` 호출 인자 패턴 박제. Trees: `gh api repos/{owner}/{repo}/git/trees/{branch}?recursive=1`. Blobs: `gh api repos/{owner}/{repo}/git/blobs/{sha}`. Commits: `gh api repos/{owner}/{repo}/commits -F sha={branch} -F path={path} -F per_page=1`. **`--paginate` flag 사용 금지**.
  - `[AUTO]` § 병렬 호출 정책은 M5b 결과 미정 박스 유지.
  - `[AUTO]` § Backend 선택 그대로 유지.
  - `[AUTO]` `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 마이그레이션 작업 범위 L31 ("가짜 `gh` 바이너리 PATH 주입 등") 표현을 trait inject 채택으로 정렬 갱신.
- **Status**: `[x]`

### M1. 에러 매핑 표 박제 (gh 종료 코드 + stderr → GitlessError) `[AUTO, spec-only]`
- **Spec reference**: `docs/specs/spec-error-contracts.md` (부분 갱신), `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 에러 매핑
- **Files**: `docs/specs/spec-error-contracts.md`
- **Depends on**: M0
- **Acceptance criteria**:
  - `[AUTO]` gh exit code + stderr 패턴 표 조사 박제. 출처 명시 (gh 공식 docs URL + 직접 호출 관찰). 최소 케이스: 정상(0), gh 미설치, 인증 실패, rate limit, Trees truncated, 5xx, 기타.
  - `[AUTO]` **stderr 매칭은 좁은 substring 한정. 정규식 사용 금지**.
  - `[AUTO]` 본 spec 또는 README에 **gh CLI 최소 버전 floor 명시** (예: `gh >= 2.40`).
  - `[AUTO]` § 인증 실패 / Rate Limit / Trees Truncated 동작 섹션의 매핑 source 갱신. exit code 매핑 표(0~5) 그대로 유지.
  - `[AUTO]` Acceptance Criteria 섹션의 mockito 시나리오를 "MockGhClient stub 응답" 표현으로 재작성.
  - `[AUTO]` § Custom Error Types에 `Http(String)` variant의 의미를 "gh subprocess 비정상 종료(인증/rate/truncated 외)"로 보강.
- **Status**: `[x]`

### M2a. GhClient trait + RealGhClient + MockGhClient 골격 `[AUTO, 코드]`
- **Spec reference**: `docs/specs/spec-github-api.md` (M0 갱신본)
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs` (또는 `crates/gitless-sync/src/shared/gh.rs`), `crates/gitless-sync/Cargo.toml` (의존성 변경 0)
- **Depends on**: M0
- **Acceptance criteria**:
  - `[AUTO]` task 시작 직전 ralph 환경 자체 점검: `gh --version` + `gh auth status`가 0 종료. 미통과 시 G-015 transient retry 적용 (N=3 + 30s backoff). 3회 실패 시 [!] BLOCKED + G-015 reference (auto-recovery 가능). 영구 실패(gh 미설치) 신호 명확하면 즉시 [!] + G-016 신규(영구 사유, 사람 대기).
  - `[AUTO]` `pub(crate) trait GhClient { fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError>; }` 정의. `GhResponse`는 `{ stdout: Vec<u8>, stderr: String, exit_code: i32 }`.
  - `[AUTO]` `RealGhClient` 구현 (production, `std::process::Command::new("gh")`). `RealGhClient::new() -> Self`. PATH 미존재 시 첫 호출에서 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환.
  - `[AUTO]` `MockGhClient` 구현 (테스트 — 인자별 응답 HashMap 또는 클로저).
  - `[AUTO]` 단위 테스트로 trait 동작 검증.
  - `[AUTO]` 기존 ureq 함수는 잔존 (M2b1/M2b2에서 본체 재작성).
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` 통과.
- **Status**: `[x]`

### M2b1. fetch_tree gh subprocess 재작성 + run_with_client entry point `[AUTO, 코드]`
- **Spec reference**: `docs/specs/spec-github-api.md` (M0), `docs/specs/spec-error-contracts.md` (M1)
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs`, `crates/gitless-sync/src/commands/scan/mod.rs` (entry point 시그니처), `crates/gitless-sync/src/main.rs` (production inject)
- **Depends on**: M2a
- **Acceptance criteria**:
  - `[AUTO]` `fetch_tree(client: &impl GhClient, repo, branch) -> Result<Vec<RemoteFile>, GitlessError>` 재작성. blob entry만 필터. `truncated: true` 감지 → `TreesTruncated`.
  - `[AUTO]` `commands::scan::run_with_client(args: &ScanArgs, client: &impl GhClient) -> Result<...>` 시그니처 도입. main.rs는 production 분기에서 `RealGhClient::new()` 1회 inject.
  - `[AUTO]` 기존 `run_with_base` 함수는 잔존 (M2b2에서 정리). `fetch_tree`는 새 시그니처 + 기존 시그니처 둘 다 잠시 공존 또는 본체만 새로.
  - `[AUTO]` `fetch_tree` 단위 테스트는 `MockGhClient` 사용. mockito 호출 0회. 인증/rate/truncated/5xx/parse 케이스 모두 cover.
  - `[AUTO]` 에러 매핑은 M1 spec 따라.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings` 통과.
- **Status**: `[x]`

### M2b2. fetch_blob + fetch_last_commit_at + run_with_base 정리 `[AUTO, 코드]`
- **Spec reference**: `docs/specs/spec-github-api.md` (M0), `docs/specs/spec-error-contracts.md` (M1)
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs`, `crates/gitless-sync/src/commands/scan/mod.rs`, `crates/gitless-sync/src/commands/diff/mod.rs`, `crates/gitless-sync/src/main.rs`, `crates/gitless-sync/tests/integration.rs`
- **Depends on**: M2b1
- **Acceptance criteria**:
  - `[AUTO]` `fetch_blob(client: &impl GhClient, repo, sha) -> Result<Vec<u8>, GitlessError>` 재작성. base64 디코딩.
  - `[AUTO]` `fetch_last_commit_at(client: &impl GhClient, repo, branch, path) -> Result<DateTime<Utc>, GitlessError>` 재작성.
  - `[AUTO]` 기존 `run_with_base` 함수 + 관련 ureq `fetch_*_with_base` 잔존 코드 모두 삭제. scan은 `run_with_client` 단일화.
  - `[AUTO]` `commands::diff::run_with_base` / `compute_diff` → `run_with_client` / `compute_diff_with_client` 시그니처로 이행 (`client: &impl GhClient` 인자 추가). diff 단위 테스트도 `MockGhClient` 기반으로 재작성. (`fetch_*_with_base` 호출 모두 제거를 위한 동반 변경 — Files 확장 근거.)
  - `[AUTO]` `main.rs`의 `GITLESS_API_BASE` dual-mode 분기 + `resolve_api_base*` 함수 + 단위 테스트 + `GITHUB_API_BASE` 상수 모두 제거 (scan/github.rs에서 `GITHUB_API_BASE`가 사라지므로 main.rs의 import도 동반 정리 — 잔존 시 컴파일 fail). scan/diff 양쪽 모두 `RealGhClient::new()` 1회 inject + `run_with_client`로 단일화. (M3는 `--token` clap 인자 + `shared/config.rs::resolve_token` 정리만 남음.)
  - `[AUTO]` 단위 테스트는 모두 `MockGhClient`. mockito 호출 0회 (production + unit tests 기준).
  - `[AUTO]` **이 시점에 production + unit tests의 ureq import 0, mockito import 0** (Cargo.toml 의존성은 미변경 — M2c).
  - `[AUTO]` `tests/integration.rs`는 mockito + `GITLESS_API_BASE` 의존이라 본 시점에 동작 불가 — ADR 0002 § 마이그레이션 작업 범위 명시(`testability는 GhClient trait + MockGhClient inject 패턴으로 해결한다 (M2a~M2c)`). 본 task에서는 모든 통합 테스트에 `#[ignore]` + 본 task reference comment 박제. M4a/M4b가 `MockGhClient` + library-entry 호출 방식으로 통째 재작성하며 이 ignore를 해제한다.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` 통과 (ignored 테스트는 무시 — `cargo test`가 ignored 테스트를 실패로 보지 않음).
- **Status**: `[x]`

### M2c. Cargo.toml ureq+mockito 삭제 + Cargo.lock 정리 + guardrails obsolete 처리 `[AUTO, 코드+guardrail]`
- **Spec reference**: ADR 0002 § 마이그레이션 작업 범위
- **Files**: `crates/gitless-sync/Cargo.toml`, `Cargo.lock`, `crates/gitless-sync/tests/integration.rs` (mockito imports 정리 — M2b2 누락분 보정), `docs/ralph/guardrails.md` (G-009 통째 삭제, G-003 obsolete 마크)
- **Depends on**: M2b2
- **Acceptance criteria**:
  - `[AUTO]` `Cargo.toml`에서 `ureq`, `mockito` 의존성 삭제. `Cargo.lock` 갱신.
  - `[AUTO]` `cargo tree`로 `ureq`/`mockito`/관련 transitive 부재 확인.
  - `[AUTO]` `guardrails.md` G-009 통째 삭제. G-003에 "**2026-05-06 obsolete (gh가 rate limit 처리)**" 마크 추가. (G-011은 M5b가 처리.)
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo deny check`, `cargo audit` 통과.
- **Status**: `[x]`

### M3. CLI 인자 + config 토큰 경로 제거 `[AUTO, 코드]`
- **Spec reference**: `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md`
- **Files**: `crates/gitless-sync/src/main.rs`, `crates/gitless-sync/src/shared/config.rs`, `crates/gitless-sync/src/commands/scan/mod.rs`, `crates/gitless-sync/src/commands/diff/mod.rs`, `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md`
- **Depends on**: M2c
- **Acceptance criteria**:
  - `[AUTO]` clap 정의에서 `--token` 인자 + `clap(env = "GITHUB_TOKEN")` 제거.
  - `[AUTO]` `shared/config.rs::resolve_token` 함수 + 관련 단위 테스트 삭제. `Config` 구조체에 token 필드 있으면 삭제.
  - `[AUTO]` `ScanArgs.token` / `DiffArgs.token` 필드 + `build_report` / `compute_diff` 안의 token resolve 게이트 + 토큰 의존 단위 테스트 모두 삭제. (M2b2가 의도적으로 contract로 남긴 잔존분 — clap 인자 제거 cascade로 동반 정리. M2b2 `run_with_base→run_with_client` Files 확장 선례.)
  - ~~`[AUTO]` **`GITLESS_API_BASE` env 처리 잔존 코드 단순 삭제** (M2b1/M2b2에서 trait inject로 옮김).~~ **OBSOLETE** — M2b2가 `GITHUB_API_BASE` 상수와 함께 `resolve_api_base*` 통째 제거 완료.
  - `[AUTO]` `spec-cli-interface.md`: 글로벌 플래그 표에서 `--token` 행 삭제. § 인자 우선순위에서 토큰 라인 제거. Acceptance Criteria의 토큰 항목 삭제.
  - `[AUTO]` `spec-config.md`: § `--token` 형식 섹션 삭제. § 우선순위 표에서 토큰 라인 삭제. § 비밀 정보 정책은 그대로 유지. Acceptance Criteria의 토큰 관련 5개 항목 삭제.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings` 통과.
- **Status**: `[x]`

### M4a. 통합 테스트 정상 경로 시나리오 `[AUTO, 코드]`
- **Spec reference**: PRD 검증 시나리오 1~4 + 9
- **Files**: `crates/gitless-sync/tests/integration.rs` (정상 경로 부분 재작성), `crates/gitless-sync/src/lib.rs` (신규 — 통합 테스트가 라이브러리 진입점을 호출하기 위한 lib target), `crates/gitless-sync/src/main.rs` (lib import 정렬), `crates/gitless-sync/src/shared/gh.rs` (`GhClient` / `GhResponse` / `RealGhClient` 가시성 `pub`), `crates/gitless-sync/src/commands/scan/mod.rs` (`run_with_client` / `build_report` `pub`), `crates/gitless-sync/src/commands/diff/mod.rs` (`run_with_client` `pub`)
- **Depends on**: M3
- **Acceptance criteria**:
  - `[AUTO]` PRD 시나리오 1~4 (4상태 분류) end-to-end: tempfile + `MockGhClient` stub → `run_with_client(args, &MockGhClient)` 호출 → stdout JSON 파싱 → 4상태 카운트 검증.
  - `[AUTO]` PRD 시나리오 9 (.gitignore + --ignore 합집합) end-to-end.
  - `[AUTO]` `cargo test --test integration` 정상 경로 부분 통과.
- **Status**: `[~]`

### M4b. 통합 테스트 에러 + partial failure 시나리오 `[AUTO, 코드]`
- **Spec reference**: PRD 검증 시나리오 10~15
- **Files**: `crates/gitless-sync/tests/integration.rs` (에러 부분)
- **Depends on**: M4a
- **Acceptance criteria**:
  - `[AUTO]` 시나리오 10 (인증 실패): `MockGhClient` stderr `HTTP 401` 흉내 → exit 2 + stderr `AUTH_FAILED` JSON.
  - `[AUTO]` 시나리오 11 (rate limit): `MockGhClient` stderr `API rate limit exceeded` 흉내 → exit 3 + stderr `RATE_LIMIT_EXCEEDED`.
  - `[AUTO]` 시나리오 12 (truncated): `MockGhClient` stdout JSON에 `truncated: true` → exit 5.
  - `[AUTO]` 시나리오 13 (`--summary-only`), 14 (`--status`), 15 (partial failure) 재현.
  - `[AUTO]` `cargo test --test integration` 전체 통과.
- **Status**: `[ ]`

### M5a. rayon 측정 (자율) `[AUTO]`
- **Spec reference**: `docs/ralph/guardrails.md` § G-011, `docs/specs/spec-github-api.md` § 병렬 호출 정책
- **Files**: 측정 raw data를 task `[~]` commit message + acceptance 본문에 박제 (M5b가 ADR 0003에 옮김).
- **Depends on**: M2c
- **Acceptance criteria**:
  - `[AUTO]` ralph 환경에서 `KneShell/gitless-sync` repo 또는 vault scale repo 대상 자율 측정. (a) rayon 8 concurrent + gh subprocess vs (b) 순차 gh subprocess 시간 측정.
  - `[AUTO]` **측정 신뢰성 룰**:
    - warm-up 1회 폐기 (PATH cache + AV scan 영향 회피).
    - N≥3 본 측정 후 평균 + p50 박제.
    - 본 측정 N=3 결과의 variance가 30% 초과 시 N=5로 확장 후 재계산.
    - 측정 도중 gh exit≠0 발생 시 G-015 transient retry policy 적용 (N=3 + 30s backoff). 3회 모두 실패 시 [!] + G-015 reference (auto-recovery 가능).
  - `[AUTO]` 환경(Windows + 실제 repo + 시점 timestamp) + 명령어 + raw timing(각 측정의 전체 시간 박제 — outlier 추적 가능).
- **Status**: `[ ]`

### M5b. rayon 유지/제거 결정 + ADR 0003 박제 + spec/guardrail 갱신 `[AUTO]`
- **Spec reference**: `docs/ralph/guardrails.md` § G-011, `docs/specs/spec-github-api.md` § 병렬 호출 정책
- **Files**: `docs/adr/0003-rayon-keep-or-drop.md` (신규), `docs/specs/spec-github-api.md`, `docs/ralph/guardrails.md`, (제거 결정 시) `crates/gitless-sync/Cargo.toml` + `crates/gitless-sync/src/commands/scan/mod.rs`
- **Depends on**: M5a
- **Acceptance criteria**:
  - `[AUTO]` M5a 측정 raw data를 git log + acceptance 본문에서 read해서 결정: ① rayon 유지 또는 ② rayon 제거.
  - `[AUTO]` **`docs/adr/0003-rayon-keep-or-drop.md` 신규 작성** (Status: Accepted, Date: 2026-05-06): § Context (M5a raw data + 측정 환경) / § Decision (① 또는 ②) / § Consequences (G-011 처분, Cargo.toml 변경, spec § 병렬 호출 정책 변경) / § References (ADR 0001 + ADR 0002 + M5a measurement raw).
  - `[AUTO]` 결정에 따라 처리:
    - 유지 시: G-011 갱신. `spec-github-api.md` § 병렬 호출 정책 확정 박제.
    - 제거 시: G-011 obsolete 마크. `Cargo.toml`에서 `rayon` 의존성 삭제. `commands/scan/mod.rs::run_with_client`의 `par_iter` → `iter`/`for` 변경. `spec-github-api.md` § 병렬 호출 정책 섹션 삭제.
  - `[AUTO]` `cargo test --workspace`, `cargo deny check`, `cargo audit` 통과 (의존성 변경 시).
- **Status**: `[ ]`

### M6. README + 의존성 안내 `[AUTO, 문서]`
- **Spec reference**: `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 의존성 안내
- **Files**: `README.md`
- **Depends on**: M2c
- **Acceptance criteria**:
  - `[AUTO]` README.md에 "Prerequisites" 섹션: `gh` CLI(M1에 박힌 floor 버전 이상) 설치 안내 + `gh auth login` 한 줄 인증. Windows/macOS/Linux 설치 명령 박제.
  - `[AUTO]` 사용 예시 섹션의 `--token env:GITHUB_TOKEN` 등 토큰 인자 표현 모두 제거.
  - `[AUTO]` gh 미설치 시 에러 메시지 동작 검증 결과 박제.
- **Status**: `[ ]`

### M7. 빌드 게이트 통과 검증 `[AUTO]`
- **Spec reference**: `docs/ralph/project-ops.md` § Coverage, `CLAUDE.md` § Test coverage, G-007
- **Files**: 미달 모듈에 unit test 추가 (필요 시)
- **Depends on**: M3, M4b, M5b, M6
- **Acceptance criteria**:
  - `[AUTO]` `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo audit` 모두 통과.
  - `[AUTO]` `cargo tarpaulin --engine llvm --workspace --out Stdout` 라인 커버리지 ≥ 80%.
  - `[AUTO]` `cargo tree`로 `ureq`, `mockito` 의존성 부재 확인. (M5b에서 rayon 제거 결정 시 rayon도 부재.)
- **Status**: `[ ]`

### M8. Self dogfooding contract step `[AUTO]`
- **Spec reference**: ADR 0002 § Consequences, tribunal P3/P4 sema gap risk (#12)
- **Files**: 박제 0. 실행 결과는 task `[x]` commit message에 카운트만 인라인.
- **Depends on**: M7
- **Acceptance criteria**:
  - `[AUTO]` ralph 환경에서 `cargo run --release -- scan --repo KneShell/gitless-sync` 실행 (gh 인증 사전 OK 가정).
  - `[AUTO]` **단조 통과 게이트** (P1 권고):
    - exit code ∈ {0, 4} (정상 또는 partial failure 정상 출력)
    - stdout JSON `serde_json::from_str` 파싱 가능
    - `summary` 객체에 `identical`/`local_only_changed`/`remote_only_changed`/`drift`/`failed` 5개 카운트 모두 음수 아닌 정수
    - `total = identical + local_only_changed + remote_only_changed + drift + failed` invariant 일치
  - `[AUTO]` 위 4개 조건 충족 시 [x]. failed 비율은 commit message에 기록만 (BLOCKED 게이트 아님). 동일 코드 + 동일 repo 상태에서 결과 단조 보장.
  - `[AUTO]` external command transient(network 5xx, gh exit≠0)는 G-015 retry policy 적용. 3회 실패 시 [!] + G-015 reference (auto-recovery 가능).
  - `[AUTO]` 별도 release evidence 파일 박제 0. git log + commit message가 evidence trail.
- **Status**: `[ ]`
