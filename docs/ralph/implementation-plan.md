# Implementation Plan

## Status
- Last updated: 2026-05-06T00:00:00Z (옵션 2− 보정 — 6 페르소나 tribunal 검증 반영)
- Total tasks: 10 (M0, M1, M2, M3, M4, M5a, M5b, M6, M7, M8)
- Completed: 0 / 10

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵된 상태.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 해당 spec 파일과 정확히 매핑되어야 함. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- **`[HUMAN→AUTO]` hybrid 마크**: ralph가 `[AUTO]` acceptance를 spec/문서에 박은 후 `[~ awaiting human]`으로 commit + exit. 사람이 `[HUMAN]` acceptance(후보 선택, 검토 등)를 처리하고 `[x]` 마크.
- **`[HUMAN]` 마크 (task 단위)**: ralph는 task 통째 skip. 사람이 수행 후 `[x]` 마크.

## Dependency Graph

```
M0 (hybrid) → M1 → M2
                    ├──→ M3 → M4
                    ├──→ M5a (HUMAN) → M5b
                    └──→ M6
                            ↓
                          M7 → M8 (HUMAN, release gate)
```

## Tasks

### M0. GhClient trait 인터페이스 + spec-github-api 통째 재작성 `[HUMAN→AUTO hybrid]`
- **Spec reference**: `docs/specs/spec-github-api.md` (재작성), `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` (L31 정렬)
- **Files**: `docs/specs/spec-github-api.md`, `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md`
- **Depends on**: none
- **Acceptance criteria**:
  - `[AUTO]` `pub(crate) trait GhClient` 정의 박제: `fn api(&self, args: &[&str]) -> Result<GhResponse, GitlessError>`. `GhResponse`는 최소 `{ stdout: Vec<u8>, stderr: String, exit_code: i32 }` 캡처.
  - `[AUTO]` 두 구현 명시: `RealGhClient` (production, `std::process::Command::new("gh")`)와 `MockGhClient` (테스트, 인자별 응답 HashMap 또는 클로저). `gh` PATH 미존재 시 첫 호출에서 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환.
  - `[AUTO]` 호출 인자 패턴 박제. Trees: `gh api repos/{owner}/{repo}/git/trees/{branch}?recursive=1`. Blobs: `gh api repos/{owner}/{repo}/git/blobs/{sha}`. Commits: `gh api repos/{owner}/{repo}/commits -F sha={branch} -F path={path} -F per_page=1`. **`--paginate` flag 사용 금지** (가독성 명목으로 추가 시 무한 페이지 → rate limit).
  - `[AUTO]` Binary entry point 시그니처 박제: `commands/scan/mod.rs::run_with_client(args: &ScanArgs, client: &impl GhClient) -> Result<...>`. main.rs는 production 분기에서 `RealGhClient::new()` 1회 inject. 통합 테스트(M4)는 library entry `run_with_client`를 직접 호출, `MockGhClient` inject.
  - `[AUTO]` testability seam 결정: trait inject로 단일화. **`GITLESS_API_BASE` env 처리는 M3에서 단순 삭제** (테스트가 trait inject로 옮기므로 env 불필요). 본 spec 본문에 명시 박제.
  - `[AUTO]` `spec-github-api.md` 통째 갱신: ureq/mockito/Agent thread-safety/HTTP 헤더 송신 표현 제거. § 목적 / § 현재 상태 / § 작업 범위 / § Acceptance Criteria 재작성. § 병렬 호출 정책은 M5b 결과 미정 박스 유지. **L3 stale ADR note (`v0.1 ureq 코드 자체의 마이그레이션 시점은 ADR 0001 follow-up open question #1로 별도 결정`)는 ADR 0002로 종료됨을 반영해 갱신/삭제.**
  - `[AUTO]` `spec-github-api.md` § Backend 선택은 그대로 유지 — `--backend rest`/`--backend graphql` 의미 유지, 호출 통로만 ureq → gh로 변경.
  - `[AUTO]` `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 마이그레이션 작업 범위 L31 ("가짜 `gh` 바이너리 PATH 주입 등") 표현을 trait inject 채택으로 정렬 갱신.
  - `[HUMAN]` ralph가 위 acceptance를 후보 1~2개 spec에 박은 후 사람이 trait shape (특히 `GhResponse` 필드 구성, `args: &[&str]` vs `args: &[String]`, `RealGhClient::new()` 시그니처 등) + main.rs inject 패턴을 검토하고 단일안 선택. 선택 후 `[x]` 마크.
- **Status**: `[ ]`

### M1. 에러 매핑 표 박제 (gh 종료 코드 + stderr → GitlessError) `[spec-only]`
- **Spec reference**: `docs/specs/spec-error-contracts.md` (부분 갱신), `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 에러 매핑
- **Files**: `docs/specs/spec-error-contracts.md`
- **Depends on**: M0
- **Acceptance criteria**:
  - `[AUTO]` gh exit code + stderr 패턴 표 조사 박제. 출처 명시 (gh 공식 docs URL + 직접 호출 관찰). 최소 케이스: 정상(0), gh 미설치(`Command::new` IO err), 인증 실패(gh stderr `HTTP 401` 또는 `gh auth status` fail), rate limit(gh stderr `API rate limit exceeded`), Trees truncated(stdout JSON `truncated: true` 파싱), 5xx(gh stderr `HTTP 5xx`), 기타.
  - `[AUTO]` **stderr 매칭은 좁은 substring(예: `rate limit exceeded`) 한정. 정규식 사용 금지** — gh 버전 minor 업데이트로 깨질 risk 회피.
  - `[AUTO]` 본 spec 또는 README에 **gh CLI 최소 버전 floor 명시** (예: `gh >= 2.40`). minor floor 변경 시 본 spec 갱신 룰 박제.
  - `[AUTO]` `spec-error-contracts.md` § 인증 실패 / Rate Limit / Trees Truncated 동작 섹션의 매핑 source를 "ureq 응답" → "gh stderr/stdout 패턴"으로 갱신. exit code 매핑 표(0~5)는 그대로 유지.
  - `[AUTO]` Acceptance Criteria 섹션의 mockito 시나리오(시나리오 11/12/15)를 "MockGhClient stub 응답" 표현으로 재작성.
  - `[AUTO]` § Custom Error Types에 `Http(String)` variant의 의미를 "gh subprocess 비정상 종료(인증/rate/truncated 외)"로 보강.
- **Status**: `[ ]`

### M2. gh wrapper 구현 + 단위 테스트 + 의존성 정리 `[코드]`
- **Spec reference**: `docs/specs/spec-github-api.md` (M0 갱신본), `docs/specs/spec-error-contracts.md` (M1 갱신본)
- **Files**: `crates/gitless-sync/src/commands/scan/github.rs` (전면 재작성), `crates/gitless-sync/Cargo.toml` (`ureq` + `mockito` 의존성 삭제), `docs/ralph/guardrails.md` (G-009 통째 삭제, G-003 obsolete 마크)
- **Depends on**: M0, M1
- **Acceptance criteria**:
  - `[AUTO]` task 시작 직전 ralph 환경 자체 점검: `gh --version` + `gh auth status`가 0 종료. 미통과 시 task 시작 전 BLOCKED + ralph 환경 안내 (mock-only 검증으로 마이그레이션 전체가 빌드되는 것 방어).
  - `[AUTO]` `GhClient` trait + `RealGhClient` + `MockGhClient` 구현 (M0 spec 따라).
  - `[AUTO]` `fetch_tree(client: &impl GhClient, repo, branch) -> Result<Vec<RemoteFile>, GitlessError>` 재작성. blob entry만 필터. `truncated: true` 감지 → `TreesTruncated`.
  - `[AUTO]` `fetch_blob(client: &impl GhClient, repo, sha) -> Result<Vec<u8>, GitlessError>` 재작성. base64 디코딩.
  - `[AUTO]` `fetch_last_commit_at(client: &impl GhClient, repo, branch, path) -> Result<DateTime<Utc>, GitlessError>` 재작성.
  - `[AUTO]` 단위 테스트는 모두 `MockGhClient` 사용. mockito 호출 0회. PRD 시나리오 11/12 단위 케이스도 mock 응답으로 재현.
  - `[AUTO]` `Cargo.toml`에서 `ureq`, `mockito` 의존성 삭제. `Cargo.lock` 갱신. `cargo tree`로 `ureq`/`mockito`/관련 transitive(rustls/webpki/hyper/tokio 등) 부재 확인.
  - `[AUTO]` `guardrails.md` G-009 통째 삭제 (M2 완료 = mockito 의존성 0). G-003도 "**2026-05-06 obsolete (gh가 rate limit 처리)**" 마크 추가. (G-011은 M5b에서 처리.)
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo deny check`, `cargo audit` 통과.
- **Status**: `[ ]`

### M3. CLI 인자 + config 토큰 경로 제거 `[코드]`
- **Spec reference**: `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md` (둘 다 부분 갱신)
- **Files**: `crates/gitless-sync/src/main.rs`, `crates/gitless-sync/src/shared/config.rs`, `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md`
- **Depends on**: M2
- **Acceptance criteria**:
  - `[AUTO]` clap 정의에서 `--token` 인자 + `clap(env = "GITHUB_TOKEN")` 제거.
  - `[AUTO]` `shared/config.rs::resolve_token` 함수 + 관련 단위 테스트 삭제. `Config` 구조체에 token 필드 있으면 삭제.
  - `[AUTO]` **`GITLESS_API_BASE` env 처리 코드 단순 삭제** (M0 결정). 관련 main.rs 분기 + 환경 변수 reference 모두 제거.
  - `[AUTO]` `spec-cli-interface.md`: 글로벌 플래그 표에서 `--token` 행 삭제. § 인자 우선순위에서 토큰 라인 제거. Acceptance Criteria의 토큰 항목 삭제.
  - `[AUTO]` `spec-config.md`: § `--token` 형식 섹션 삭제. § 우선순위 표에서 토큰 라인 삭제. § 비밀 정보 정책은 그대로 유지. Acceptance Criteria의 토큰 관련 5개 항목 삭제.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings` 통과.
- **Status**: `[ ]`

### M4. 통합 테스트 재작성 `[코드]`
- **Spec reference**: PRD 검증 시나리오 1~4, 9~15. `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 마이그레이션 작업 범위
- **Files**: `crates/gitless-sync/tests/integration.rs` (전면 재작성)
- **Depends on**: M3
- **Acceptance criteria**:
  - `[AUTO]` 모든 시나리오를 M0에서 정의한 `run_with_client(args, &MockGhClient)` 진입점 기반으로 재작성.
  - `[AUTO]` PRD 시나리오 1~4 (4상태), 9 (.gitignore), 10 (인증 실패: MockGhClient stderr `HTTP 401`), 11 (rate limit), 12 (truncated), 13 (`--summary-only`), 14 (`--status`), 15 (partial failure) 모두 통과.
  - `[AUTO]` `cargo test --test integration` 통과.
- **Status**: `[ ]`

### M5a. rayon 측정 `[HUMAN]`
- **Spec reference**: `docs/ralph/guardrails.md` § G-011, `docs/specs/spec-github-api.md` § 병렬 호출 정책
- **Files**: 측정 결과를 별도 메모(`docs/ralph/m5-measurement.md` 등) 또는 본 acceptance 본문에 박제.
- **Depends on**: M2
- **Acceptance criteria**:
  - `[HUMAN]` 1000-path 또는 vault scale (100~300 path) 환경에서 측정. (a) rayon 8 concurrent + gh subprocess vs (b) 순차 gh subprocess 시간 측정. 환경(Windows + 실제 vault 또는 KneShell repo) + 명령어 + 결과 박제.
  - `[HUMAN]` 측정 결과 박제 형식: 각 경로 N≥3회 실행 후 평균/p50 (정량 noise 회피, 비용 균형).
  - `[HUMAN]` 사람이 박제 완료 후 `[x]` 마크 + commit.
- **Status**: `[ ]`

### M5b. rayon 유지/제거 결정 + guardrail/spec 갱신 `[AUTO]`
- **Spec reference**: `docs/ralph/guardrails.md` § G-011, `docs/specs/spec-github-api.md` § 병렬 호출 정책
- **Files**: `docs/ralph/guardrails.md`, `docs/specs/spec-github-api.md`, (제거 결정 시) `crates/gitless-sync/Cargo.toml` + `crates/gitless-sync/src/commands/scan/mod.rs`
- **Depends on**: M5a
- **Acceptance criteria**:
  - `[AUTO]` M5a 측정 결과를 read해서 결정: ① rayon 유지(spawn 비용 << 순차 latency) 또는 ② rayon 제거(spawn 비용이 무시 못 함).
  - `[AUTO]` 결정에 따라 처리:
    - 유지 시: G-011 갱신 (gh subprocess 환경에서도 유지 + abuse detection 책임 gh로 외부화). `spec-github-api.md` § 병렬 호출 정책 확정 박제.
    - 제거 시: G-011 obsolete 마크. `Cargo.toml`에서 `rayon` 의존성 삭제. `commands/scan/mod.rs::run_with_client`의 `par_iter` → `iter`/`for` 변경. `spec-github-api.md` § 병렬 호출 정책 섹션 삭제.
  - `[AUTO]` `cargo test --workspace`, `cargo deny check`, `cargo audit` 통과 (의존성 변경 시).
- **Status**: `[ ]`

### M6. README + 의존성 안내 `[문서]`
- **Spec reference**: `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` § 의존성 안내
- **Files**: `README.md` (현재 존재 여부 확인 후 신규 작성 또는 갱신)
- **Depends on**: M2
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

### M8. Release contract step `[HUMAN]`
- **Spec reference**: ADR 0002 § Consequences (호출자 인터페이스), tribunal P3/P4 sema gap risk (#12)
- **Files**: 검증 결과를 별도 메모(`docs/ralph/m8-contract.md` 등) 또는 본 acceptance 본문에 박제
- **Depends on**: M7
- **Acceptance criteria**:
  - `[HUMAN]` `RealGhClient`로 실제 gh + 실제 GitHub repo (vault `KneShell/obsidian-vault` 또는 임의 small repo) 1회 `gitless-sync scan` 실행.
  - `[HUMAN]` 4분류 카운트가 v0.1 baseline (vault 2026-04-29 기준: 284 identical / 55 local_only_changed / 17 remote_only_changed / 0 drift / 0 failed) 또는 비교 가능 baseline과 일치/근사 확인.
  - `[HUMAN]` `fetch_tree`/`fetch_blob`/`fetch_last_commit_at` 모든 경로가 happy path로 통과 박제.
  - `[HUMAN]` 사람이 박제 완료 후 `[x]` 마크 + commit. v0.2 release tag 가능 시점.
- **Status**: `[ ]`
