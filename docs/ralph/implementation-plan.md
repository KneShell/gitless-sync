# Implementation Plan

## Status
- Last updated: 2026-05-06T00:00:00Z (사전 결정 박음 + M0/M2/M5a/M8 자율화 보정)
- Total tasks: 12 (M0, M1, M2a, M2b, M2c, M3, M4, M5a, M5b, M6, M7, M8)
- Completed: 0 / 12

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵된 상태.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 해당 spec 파일과 정확히 매핑되어야 함. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- **모든 task는 `[AUTO]`** — 사람 개입 0으로 자율 루프 진행. 결정 항목은 `docs/specs/spec-github-api.md` § GhClient trait 사전 결정 등 사전 박음.

## Dependency Graph

```
M0 → M1 → M2a → M2b → M2c
                       ├──→ M3 → M4
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
  - `[AUTO]` `spec-github-api.md` 통째 갱신: § 목적 / § 현재 상태 / § 작업 범위 / § Acceptance Criteria 재작성. ureq/mockito/Agent thread-safety/HTTP 헤더 송신 표현 제거. § GhClient trait 사전 결정 섹션의 6개 결정을 본문 § 작업 범위로 옮기고, 사전 결정 섹션은 historical mark만 남기거나 제거.
  - `[AUTO]` 호출 인자 패턴 박제. Trees: `gh api repos/{owner}/{repo}/git/trees/{branch}?recursive=1`. Blobs: `gh api repos/{owner}/{repo}/git/blobs/{sha}`. Commits: `gh api repos/{owner}/{repo}/commits -F sha={branch} -F path={path} -F per_page=1`. **`--paginate` flag 사용 금지** (가독성 명목으로 추가 시 무한 페이지 → rate limit).
  - `[AUTO]` § 병렬 호출 정책은 M5b 결과 미정 박스 유지 ("M5b 측정·결정 후 확정").
  - `[AUTO]` § Backend 선택 그대로 유지 — `--backend rest`/`--backend graphql` 의미 유지, 호출 통로만 ureq → gh로 변경.
  - `[AUTO]` `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 마이그레이션 작업 범위 L31 ("가짜 `gh` 바이너리 PATH 주입 등") 표현을 trait inject 채택으로 정렬 갱신.
- **Status**: `[ ]`

### M1. 에러 매핑 표 박제 (gh 종료 코드 + stderr → GitlessError) `[AUTO, spec-only]`
- **Spec reference**: `docs/specs/spec-error-contracts.md` (부분 갱신), `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 에러 매핑
- **Files**: `docs/specs/spec-error-contracts.md`
- **Depends on**: M0
- **Acceptance criteria**:
  - `[AUTO]` gh exit code + stderr 패턴 표 조사 박제. 출처 명시 (gh 공식 docs URL + 직접 호출 관찰). 최소 케이스: 정상(0), gh 미설치(`Command::new` IO err), 인증 실패(gh stderr `HTTP 401` 또는 `gh auth status` fail), rate limit(gh stderr `API rate limit exceeded`), Trees truncated(stdout JSON `truncated: true` 파싱), 5xx(gh stderr `HTTP 5xx`), 기타.
  - `[AUTO]` **stderr 매칭은 좁은 substring(예: `rate limit exceeded`) 한정. 정규식 사용 금지** — gh 버전 minor 업데이트로 깨질 risk 회피.
  - `[AUTO]` 본 spec 또는 README에 **gh CLI 최소 버전 floor 명시** (예: `gh >= 2.40`). minor floor 변경 시 본 spec 갱신 룰 박제.
  - `[AUTO]` § 인증 실패 / Rate Limit / Trees Truncated 동작 섹션의 매핑 source를 "ureq 응답" → "gh stderr/stdout 패턴"으로 갱신. exit code 매핑 표(0~5) 그대로 유지.
  - `[AUTO]` Acceptance Criteria 섹션의 mockito 시나리오(11/12/15)를 "MockGhClient stub 응답" 표현으로 재작성.
  - `[AUTO]` § Custom Error Types에 `Http(String)` variant의 의미를 "gh subprocess 비정상 종료(인증/rate/truncated 외)"로 보강.
- **Status**: `[ ]`

### M2a. GhClient trait + RealGhClient + MockGhClient 골격 `[AUTO, 코드]`
- **Spec reference**: `docs/specs/spec-github-api.md` (M0 갱신본)
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs` (또는 `crates/gitless-sync/src/shared/gh.rs`), `crates/gitless-sync/Cargo.toml` (의존성 변경 0)
- **Depends on**: M0
- **Acceptance criteria**:
  - `[AUTO]` task 시작 직전 ralph 환경 자체 점검: `gh --version` + `gh auth status`가 0 종료. 미통과 시 BLOCKED + ralph 환경 안내.
  - `[AUTO]` `pub(crate) trait GhClient { fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError>; }` 정의. `GhResponse`는 `{ stdout: Vec<u8>, stderr: String, exit_code: i32 }`.
  - `[AUTO]` `RealGhClient` 구현 (production, `std::process::Command::new("gh")`). `RealGhClient::new() -> Self`. PATH 미존재 시 첫 호출에서 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환.
  - `[AUTO]` `MockGhClient` 구현 (테스트 — 인자별 응답 HashMap 또는 클로저 등록).
  - `[AUTO]` 단위 테스트로 trait 동작 검증: `MockGhClient`가 인자 일치 시 미리 등록된 응답 반환. `RealGhClient`는 `gh --version` 정도의 가벼운 호출로 PATH lookup 검증 (실제 gh 호출).
  - `[AUTO]` 기존 ureq 함수 (`fetch_tree`/`fetch_blob`/`fetch_last_commit_at`)는 잔존 (M2b에서 본체 재작성).
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` 통과.
- **Status**: `[ ]`

### M2b. fetch_* 본체 gh subprocess 재작성 + entry point 시그니처 변경 `[AUTO, 코드]`
- **Spec reference**: `docs/specs/spec-github-api.md` (M0), `docs/specs/spec-error-contracts.md` (M1)
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs`, `crates/gitless-sync/src/commands/scan/mod.rs` (entry point 시그니처 변경), `crates/gitless-sync/src/main.rs` (production inject)
- **Depends on**: M2a
- **Acceptance criteria**:
  - `[AUTO]` `fetch_tree(client: &impl GhClient, repo, branch) -> Result<Vec<RemoteFile>, GitlessError>` 재작성. blob entry만 필터. `truncated: true` 감지 → `TreesTruncated`.
  - `[AUTO]` `fetch_blob(client: &impl GhClient, repo, sha) -> Result<Vec<u8>, GitlessError>` 재작성. base64 디코딩.
  - `[AUTO]` `fetch_last_commit_at(client: &impl GhClient, repo, branch, path) -> Result<DateTime<Utc>, GitlessError>` 재작성. 첫 commit의 `commit.committer.date` 파싱.
  - `[AUTO]` `commands::scan::run_with_client(args: &ScanArgs, client: &impl GhClient) -> Result<...>` 시그니처 도입. main.rs는 production 분기에서 `RealGhClient::new()` 1회 inject.
  - `[AUTO]` 기존 `run_with_base` 함수 + `GITLESS_API_BASE` env 처리 → 새 `run_with_client` + trait inject로 대체. (잔존 GITLESS_API_BASE 코드 정리는 M3.)
  - `[AUTO]` 단위 테스트는 모두 `MockGhClient` 사용. mockito 호출 0회 (이전 mockito 의존 테스트는 mock 응답으로 변환).
  - `[AUTO]` 에러 매핑은 M1 spec 따라. 인증/rate/truncated/5xx/parse 케이스 단위 테스트 커버.
  - `[AUTO]` **이 시점에 ureq import 0, mockito import 0** (Cargo.toml 의존성은 미변경 — M2c).
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings` 통과.
- **Status**: `[ ]`

### M2c. Cargo.toml ureq+mockito 삭제 + Cargo.lock 정리 `[AUTO, 코드]`
- **Spec reference**: ADR 0002 § 마이그레이션 작업 범위
- **Files**: `crates/gitless-sync/Cargo.toml`, `Cargo.lock`, `docs/ralph/guardrails.md` (G-009 통째 삭제, G-003 obsolete 마크)
- **Depends on**: M2b
- **Acceptance criteria**:
  - `[AUTO]` `Cargo.toml`에서 `ureq`, `mockito` 의존성 삭제. `Cargo.lock` 갱신.
  - `[AUTO]` `cargo tree`로 `ureq`/`mockito`/관련 transitive(rustls/webpki/hyper/tokio 등) 부재 확인.
  - `[AUTO]` `guardrails.md` G-009 통째 삭제. G-003에 "**2026-05-06 obsolete (gh가 rate limit 처리)**" 마크 추가. (G-011은 M5b가 처리.)
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo deny check`, `cargo audit` 통과.
- **Status**: `[ ]`

### M3. CLI 인자 + config 토큰 경로 제거 `[AUTO, 코드]`
- **Spec reference**: `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md` (둘 다 부분 갱신)
- **Files**: `crates/gitless-sync/src/main.rs`, `crates/gitless-sync/src/shared/config.rs`, `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md`
- **Depends on**: M2c
- **Acceptance criteria**:
  - `[AUTO]` clap 정의에서 `--token` 인자 + `clap(env = "GITHUB_TOKEN")` 제거.
  - `[AUTO]` `shared/config.rs::resolve_token` 함수 + 관련 단위 테스트 삭제. `Config` 구조체에 token 필드 있으면 삭제.
  - `[AUTO]` **`GITLESS_API_BASE` env 처리 잔존 코드 단순 삭제** (M2b에서 trait inject로 옮김). 관련 main.rs 분기 + 환경 변수 reference 모두 제거.
  - `[AUTO]` `spec-cli-interface.md`: 글로벌 플래그 표에서 `--token` 행 삭제. § 인자 우선순위에서 토큰 라인 제거. Acceptance Criteria의 토큰 항목 삭제.
  - `[AUTO]` `spec-config.md`: § `--token` 형식 섹션 삭제. § 우선순위 표에서 토큰 라인 삭제. § 비밀 정보 정책은 그대로 유지. Acceptance Criteria의 토큰 관련 5개 항목 삭제.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings` 통과.
- **Status**: `[ ]`

### M4. 통합 테스트 재작성 `[AUTO, 코드]`
- **Spec reference**: PRD 검증 시나리오 1~4, 9~15. `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 마이그레이션 작업 범위
- **Files**: `crates/gitless-sync/tests/integration.rs` (전면 재작성)
- **Depends on**: M3
- **Acceptance criteria**:
  - `[AUTO]` 모든 시나리오를 M0/M2b에서 정의한 `run_with_client(args, &MockGhClient)` 진입점 기반으로 재작성.
  - `[AUTO]` PRD 시나리오 1~4 (4상태), 9 (.gitignore), 10 (인증 실패: MockGhClient stderr `HTTP 401`), 11 (rate limit), 12 (truncated), 13 (`--summary-only`), 14 (`--status`), 15 (partial failure) 모두 통과.
  - `[AUTO]` `cargo test --test integration` 통과.
- **Status**: `[ ]`

### M5a. rayon 측정 `[AUTO]`
- **Spec reference**: `docs/ralph/guardrails.md` § G-011, `docs/specs/spec-github-api.md` § 병렬 호출 정책
- **Files**: 측정 결과는 본 task acceptance 본문 또는 `[~]` commit message에 raw data 박제 (M5b가 ADR 0003에 옮김).
- **Depends on**: M2c
- **Acceptance criteria**:
  - `[AUTO]` ralph 환경에서 `KneShell/gitless-sync` repo 또는 vault scale repo 대상 자율 측정. (a) rayon 8 concurrent + gh subprocess vs (b) 순차 gh subprocess 시간 측정.
  - `[AUTO]` 환경(Windows + 실제 repo) + 명령어 + raw timing 박제. 각 경로 N≥3회 실행 후 평균/p50 박제 (정량 noise 회피, 비용 균형).
  - `[AUTO]` 측정 raw data를 본 task `[~]` commit message + acceptance 본문에 기록. M5b가 ADR 0003에 영구 박제.
- **Status**: `[ ]`

### M5b. rayon 유지/제거 결정 + ADR 0003 박제 + spec/guardrail 갱신 `[AUTO]`
- **Spec reference**: `docs/ralph/guardrails.md` § G-011, `docs/specs/spec-github-api.md` § 병렬 호출 정책
- **Files**: `docs/adr/0003-rayon-keep-or-drop.md` (신규), `docs/specs/spec-github-api.md`, `docs/ralph/guardrails.md`, (제거 결정 시) `crates/gitless-sync/Cargo.toml` + `crates/gitless-sync/src/commands/scan/mod.rs`
- **Depends on**: M5a
- **Acceptance criteria**:
  - `[AUTO]` M5a 측정 raw data(commit message + acceptance 본문)를 read해서 결정: ① rayon 유지(spawn 비용 << 순차 latency) 또는 ② rayon 제거(spawn 비용 무시 못 함, gh 자체 retry/backoff로 충분).
  - `[AUTO]` **`docs/adr/0003-rayon-keep-or-drop.md` 신규 작성** (Status: Accepted, Date: 2026-05-06): § Context (M5a raw data + 측정 환경) / § Decision (① 또는 ②) / § Consequences (G-011 처분, Cargo.toml 변경, spec § 병렬 호출 정책 변경) / § References (ADR 0001 + ADR 0002 + M5a measurement raw).
  - `[AUTO]` 결정에 따라 처리:
    - 유지 시: G-011 갱신 (gh subprocess 환경에서도 유지 + abuse detection 책임 gh로 외부화). `spec-github-api.md` § 병렬 호출 정책 확정 박제.
    - 제거 시: G-011 obsolete 마크. `Cargo.toml`에서 `rayon` 의존성 삭제. `commands/scan/mod.rs::run_with_client`의 `par_iter` → `iter`/`for` 변경. `spec-github-api.md` § 병렬 호출 정책 섹션 삭제.
  - `[AUTO]` `cargo test --workspace`, `cargo deny check`, `cargo audit` 통과 (의존성 변경 시).
- **Status**: `[ ]`

### M6. README + 의존성 안내 `[AUTO, 문서]`
- **Spec reference**: `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 의존성 안내
- **Files**: `README.md` (현재 존재 여부 확인 후 신규 작성 또는 갱신)
- **Depends on**: M2c
- **Acceptance criteria**:
  - `[AUTO]` README.md에 "Prerequisites" 섹션: `gh` CLI(M1에 박힌 floor 버전 이상) 설치 안내 + `gh auth login` 한 줄 인증 안내. Windows(`winget install GitHub.cli`)/macOS(`brew install gh`)/Linux 설치 명령 박제.
  - `[AUTO]` 사용 예시 섹션의 `--token env:GITHUB_TOKEN` 등 토큰 인자 표현 모두 제거. `gh auth login` 사전 실행 가정으로 단순화.
  - `[AUTO]` gh 미설치 시 에러 메시지 동작 검증 결과 박제.
- **Status**: `[ ]`

### M7. 빌드 게이트 통과 검증 `[AUTO]`
- **Spec reference**: `docs/ralph/project-ops.md` § Coverage, `CLAUDE.md` § Test coverage, G-007
- **Files**: 미달 모듈에 unit test 추가 (필요 시)
- **Depends on**: M3, M4, M5b, M6
- **Acceptance criteria**:
  - `[AUTO]` `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo audit` 모두 통과.
  - `[AUTO]` `cargo tarpaulin --engine llvm --workspace --out Stdout` 라인 커버리지 ≥ 80%.
  - `[AUTO]` `cargo tree`로 `ureq`, `mockito` 의존성 부재 확인. (M5b에서 rayon 제거 결정 시 rayon도 부재.)
- **Status**: `[ ]`

### M8. Self dogfooding contract step `[AUTO]`
- **Spec reference**: ADR 0002 § Consequences (호출자 인터페이스), tribunal P3/P4 sema gap risk (#12)
- **Files**: 박제 0. 실행 결과는 task `[x]` commit message에 카운트만 인라인.
- **Depends on**: M7
- **Acceptance criteria**:
  - `[AUTO]` ralph 환경에서 `cargo run --release -- scan --repo KneShell/gitless-sync` 실행 (gh 인증 사전 OK 가정). gitless-sync 자체 source(local) vs origin/main 비교.
  - `[AUTO]` exit code = 0 (정상 종료) 또는 = 4 (partial failure이지만 정상 출력).
  - `[AUTO]` stdout JSON `serde_json::from_str` 파싱 가능. `summary` 객체에 `identical`/`local_only_changed`/`remote_only_changed`/`drift`/`failed` 5개 카운트 모두 음수 아닌 정수. `total = identical + local_only_changed + remote_only_changed + drift + failed` 일치.
  - `[AUTO]` `failed` 카운트가 비정상적으로 크지 않음 (예: 전체 파일의 50% 이상이면 BLOCKED — 무언가 깨짐).
  - `[AUTO]` 위 4개 조건 충족 시 task `[x]` 마크 + commit. commit message에 카운트 한 줄 박음 (예: "M8: 102 identical / 5 local_only_changed / 0 remote_only_changed / 1 drift / 0 failed").
  - `[AUTO]` 별도 release evidence 파일 박제 0. git log + commit message가 evidence trail.
- **Status**: `[ ]`
