# Spec: GitHub API Integration

> **2026-05-06 (M0)**: ADR 0001 + ADR 0002 정합 통째 재작성. v0.1 ureq baseline 표현(직접 HTTP 호출 / mockito / Agent thread-safety / HTTP 헤더 송신 검증) 제거. 모든 GitHub API 호출은 `gh api` subprocess 단일 통로.

## 목적

GitHub Trees / Blobs / Commits API를 `gh api` subprocess로 호출 (ADR 0001 + ADR 0002). 인증·rate limit·truncation 등 운영 책임은 `gh`에 위임하고, 본 도구는 종료 코드 + stderr를 좁은 substring 매칭으로 `GitlessError`에 매핑하는 책임만 진다.

## 현재 상태

- **결정 박힘**:
  - ADR 0001: `gh` subprocess 단일 통로 + read-only 영구.
  - ADR 0002: v0.1 ureq baseline 일괄 마이그레이션. ureq + mockito 의존성 제거.
- **코드 baseline**: ureq 함수 잔존 (마이그레이션 task M2b1/M2b2가 본체 재작성, M2c가 의존성 정리). 본 spec은 마이그레이션 후 단일 baseline.
- **마이그레이션 task 매핑**: M2a (trait/Real/Mock 골격) → M2b1 (`fetch_tree` + `run_with_client` entry) → M2b2 (`fetch_blob` + `fetch_last_commit_at` + `run_with_base` 정리) → M2c (의존성 + guardrail 정리).

## 작업 범위

### `GhClient` trait + `GhResponse`

```rust
pub(crate) struct GhResponse {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub exit_code: i32,
}

pub(crate) trait GhClient {
    fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError>;
}
```

설계 근거:

- `GhResponse`에 `headers` / `duration` 등 추가 필드는 yagni. v0.1 매핑은 `exit_code` + `stderr` substring + `stdout` JSON으로 충분.
- `&[&str]`은 lifetime juggling, `IntoIterator<Item = impl AsRef<str>>` generic은 `dyn GhClient` trait object를 깬다. `&[String]`이 호출 측 `format!` 결과를 `vec![...]`에 박기 가장 자연.
- `api()`는 raw `GhResponse`를 transparent 반환한다. `exit_code`/`stderr` → `GitlessError` 매핑은 호출 측(`fetch_*`) 책임. 매핑 표는 `spec-error-contracts.md` (M1) 한 곳에만 박는다.

### `RealGhClient` (production)

- `pub(crate) fn new() -> Self` — 인자 0개. PATH lookup으로 `gh` 찾는다.
- `binary_path: Option<PathBuf>` 같은 inject 옵션은 yagni 적용으로 빠짐.
- 내부 호출: `std::process::Command::new("gh").args(args).output()`.
- `gh` 미존재 시 첫 호출에서 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환. (`Command::new` IO 에러를 본 variant로 매핑.)

### `MockGhClient` (테스트)

- 인자별 응답을 HashMap 또는 클로저로 stub.
- 단위 테스트 + 통합 테스트 모두 `MockGhClient` inject. mockito 호출 0회. v0.1 ureq baseline 시기에 박혀 있던 mockito 시나리오는 모두 `MockGhClient` stub 응답으로 재작성.

### `main.rs` entry pattern

- production 분기에서 `RealGhClient::new()`를 1회 inject:
  ```rust
  let client = RealGhClient::new();
  commands::scan::run_with_client(&args, &client)
  ```
- 통합 테스트는 library entry `commands::scan::run_with_client(args: &ScanArgs, client: &impl GhClient)`를 직접 호출 + `MockGhClient` inject. 테스트가 production CLI 진입(`main`)을 거치지 않는다.

### `fetch_*` 인터페이스

v0.1 ureq baseline 시그니처에서 `token` 인자 제거 + `client: &impl GhClient` 추가:

- `fn fetch_tree(client: &impl GhClient, repo: &str, branch: &str) -> Result<Vec<RemoteFile>, GitlessError>`
- `fn fetch_blob(client: &impl GhClient, repo: &str, sha: &str) -> Result<Vec<u8>, GitlessError>`
- `fn fetch_last_commit_at(client: &impl GhClient, repo: &str, branch: &str, path: &str) -> Result<DateTime<Utc>, GitlessError>`

### `gh api` 호출 인자 패턴

**`--paginate` flag 사용 금지.** paging이 필요한 경우 `per_page`를 인자에 명시 (Commits API). `--paginate`는 다중 페이지 stdout concat 동작이 본 도구 단일 응답 파싱 가정과 충돌.

#### `fetch_tree`

- 호출: `gh api repos/{owner}/{repo}/git/trees/{branch}?recursive=1`
- args 빌드 예: `vec!["api".to_string(), format!("repos/{owner}/{repo}/git/trees/{branch}?recursive=1")]`
- 응답 처리 (stdout JSON):
  - `tree` 배열에서 `type == "blob"`만 추출. `type == "tree"`(디렉토리)는 무시.
  - mode `100755` / `120000` / `160000` 등 v0.1 비목표 entry는 skip + warning(stderr) (G-010).
  - `truncated == true` → `GitlessError::TreesTruncated` 즉시 반환, exit 5 (G-002). 부분 결과 사용 금지.

#### `fetch_blob`

- 호출: `gh api repos/{owner}/{repo}/git/blobs/{sha}`
- args 빌드 예: `vec!["api".to_string(), format!("repos/{owner}/{repo}/git/blobs/{sha}")]`
- 응답 처리 (stdout JSON):
  - `{"content": "<base64>", "encoding": "base64", ...}`.
  - base64 디코딩 후 raw bytes 반환.

#### `fetch_last_commit_at`

- 호출: `gh api -X GET repos/{owner}/{repo}/commits -F sha={branch} -F path={path} -F per_page=1`
- `-X GET` prepend은 필수 (G-017): `gh`는 `-F` 플래그가 하나라도 있으면 method를 POST로 자동 전환한다. commits endpoint는 GET 전용이라 POST 시 404 반환. path 인자 앞에 `-X GET`를 박아 method를 명시적으로 GET으로 고정.
- args 빌드 예:
  ```rust
  vec![
      "api".to_string(),
      "-X".to_string(), "GET".to_string(),
      format!("repos/{owner}/{repo}/commits"),
      "-F".to_string(), format!("sha={branch}"),
      "-F".to_string(), format!("path={path}"),
      "-F".to_string(), "per_page=1".to_string(),
  ]
  ```
- 응답 처리 (stdout JSON 배열의 첫 번째 commit):
  - `commit.committer.date` (ISO-8601) → `DateTime<Utc>`.
- **호출 측(`scan::run_with_client`) 책임**: 차이 있는 파일에 한해서만 호출 (G-003은 ADR 0002로 도구 책임 종료 표시 예정이지만 호출 빈도 자체는 그대로 절약).

### 에러 매핑 (위임)

매핑 표는 `spec-error-contracts.md` (M1)에 한 곳에만 박는다. 본 spec은 매핑 종류만 명시:

- 인증 실패 → `GitlessError::AuthFailed` (exit 2)
- Rate Limit → `GitlessError::RateLimitExceeded { reset_at }` (exit 3)
- Trees truncated → `GitlessError::TreesTruncated` (exit 5)
- 5xx / 기타 비정상 → `GitlessError::Http(String)` (exit 1)
- gh 미설치 → `GitlessError::Config(String)` (exit 1)

매칭 신호는 좁은 stderr substring + exit_code 조합. **정규식 사용 금지** (M1 룰).

### Backend 선택 (v0.1 stub + Phase 4 활성화)

- v0.1는 **REST backend만 활성**. GraphQL backend는 인터페이스만 박고 본체는 stub.
- `--backend rest` (기본): 본 spec § fetch_tree / fetch_blob / fetch_last_commit_at + § 병렬 호출 정책 그대로 동작.
- `--backend graphql`: `GitlessError::Config("GraphQL backend not implemented in v0.1; use --backend rest. Phase 4 ETA.")` 즉시 반환, exit code 1. (orchestrator `scan::run` 진입부에서 분기.)
- forward-compatibility 의도: 호출자(LLM)가 v0.1부터 `--backend graphql`을 명시 가능. Phase 4에서 GraphQL backend 본체 채울 때 호출자 코드 변경 0.
- Phase 4 활성화 시 본 섹션을 갱신: GraphQL endpoint (`/graphql`), alias batching 패턴, GraphQL 응답 → `Vec<RemoteFile>` / `DateTime<Utc>` 매핑.

### 병렬 호출 정책 (Latency)

> **확정 (ADR 0003, 2026-05-07).** M5a 측정(commit `5e95312`)에서 rayon 8c vs sequential 4.86x speedup 입증 → ADR 0003에서 rayon 유지 결정. 본 섹션은 baseline이 아닌 확정 정책.

- `fetch_last_commit_at`은 차이 있는 파일 N개에 대해 직렬 호출 시 N × subprocess spawn + GitHub round-trip latency 누적 → 큰 vault에서 사용자 인내심 한계.
- 해결안: rayon으로 병렬 호출, default **8 concurrent**. M5a 측정에서 13 path 기준 sequential 6.56s → rayon 8c 1.35s (4.86x speedup).
- 패턴: `paths.par_iter().map(|p| github::fetch_last_commit_at(client, repo, branch, p)).collect::<Result<Vec<_>, _>>()` 를 `rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap().install(...)`로 thread pool 명시 제어.
- 동시 요청 수 상한 = 8 (G-011, GitHub abuse detection 회피). 변경 시 G-011 + 본 섹션 + ADR 0003 동시 갱신.
- burst 시 gh stderr `429` 또는 abuse detection 신호 → `GitlessError::Http(...)`로 매핑 후 즉시 종료. exponential backoff은 v0.1 비목표 (Phase 4).
- `fetch_tree`(scan에서 1회) / `fetch_blob`(diff 명령에서만) 병렬화 대상 아님.

## Acceptance Criteria

마이그레이션 task M2a~M2c가 본 spec을 충족한다. 단위 테스트는 모두 `MockGhClient` stub 기반.

- `[AUTO]` `fetch_tree`가 MockGhClient stub 정상 응답에서 `Vec<RemoteFile>` 반환 (blob entry만 필터, `tree`/`160000`/`120000`/`100755` skip).
- `[AUTO]` `fetch_tree`가 MockGhClient stub 응답 `truncated: true` → `GitlessError::TreesTruncated` (PRD 검증 시나리오 12).
- `[AUTO]` `fetch_tree`가 MockGhClient stub 인증 실패 stderr 패턴 → `GitlessError::AuthFailed`.
- `[AUTO]` `fetch_tree`가 MockGhClient stub rate limit stderr 패턴 → `GitlessError::RateLimitExceeded { reset_at }` (PRD 검증 시나리오 11).
- `[AUTO]` `fetch_tree`가 MockGhClient stub 5xx stderr 패턴 → `GitlessError::Http(...)`.
- `[AUTO]` `fetch_blob`가 MockGhClient stub 200 base64 응답을 raw bytes로 디코딩.
- `[AUTO]` `fetch_blob`가 잘못된 base64 응답 → `GitlessError::Http(...)` 또는 적절한 매핑.
- `[AUTO]` `fetch_last_commit_at`가 MockGhClient stub 응답에서 첫 commit의 date를 `DateTime<Utc>`로 파싱.
- `[AUTO]` `fetch_last_commit_at`가 빈 commits 배열 응답 → `GitlessError::Http(...)` (예상 외 케이스).
- `[AUTO]` `RealGhClient::new()` 호출 후 `gh` 미존재 환경에서 첫 `api()` 호출이 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환.
- ~~`[HUMAN]` 실제 GitHub repo + 실제 PAT(Fine-grained `Contents: Read`)로 `fetch_tree` 1회 통합 검증.~~ **OBSOLETE (ADR 0001).** vault 실전 검증 (2026-04-29, OAuth via `gh auth token`, 356 파일)으로 입증. PAT 권한 가이드는 gh subprocess 채택으로 도구 책임 밖.
