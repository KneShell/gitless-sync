# Spec: GitHub API Integration

> **2026-05-06 Note (ADR 0001 + ADR 0002)**: 본 spec은 v0.1 ureq baseline 정합 상태. ADR 0002로 gh subprocess 일괄 마이그레이션 결정 종료 → `docs/ralph/implementation-plan.md` M0에서 본 spec 통째 재작성 예정. 마이그레이션 완료 전까지 본 spec(ureq + mockito 표현)은 코드 baseline과 정합.

## 목적
GitHub Trees / Blobs / Commits API를 blocking `ureq`로 호출 (v0.1). 인증·rate limit·truncation을 구조화 에러로 매핑.

## 현재 상태
- `crates/gitless-sync/src/commands/scan/github.rs`에 3개 함수 시그니처 박힘:
  - `fetch_tree(repo, branch, token) -> Result<Vec<RemoteFile>, GitlessError>`
  - `fetch_blob(repo, sha, token) -> Result<Vec<u8>, GitlessError>`
  - `fetch_last_commit_at(repo, branch, path, token) -> Result<DateTime<Utc>, GitlessError>`
- 모두 `todo!()`. 구현 필요.
- `RemoteFile` 구조체는 정의 완료.
- 의존성: `ureq` (json feature), `mockito` (dev) — Cargo.toml에 박힘.

## 작업 범위

### `fetch_tree`
- 엔드포인트: `GET /repos/{owner}/{repo}/git/trees/{branch}?recursive=1`
- 헤더: `Authorization: Bearer <token>`, `User-Agent: gitless-sync/0.1`, `Accept: application/vnd.github+json`.
- 응답: `tree` 배열에서 `type == "blob"`만 추출. `type == "tree"`(디렉토리)는 무시. mode `100755` / `120000` / `160000` 등은 v0.1에서 skip + warning(stderr) (G-010).
- `truncated == true` → `GitlessError::TreesTruncated`, exit 5 (G-002).
- 401 → `AuthFailed`. 403 + rate limit 헤더 → `RateLimitExceeded`. 5xx → `Http`.

### `fetch_blob`
- 엔드포인트: `GET /repos/{owner}/{repo}/git/blobs/{sha}`
- 응답: JSON `{"content": "<base64>", "encoding": "base64", ...}`.
- base64 디코딩 후 raw bytes 반환.
- 위 인증 / rate limit 매핑 동일.

### `fetch_last_commit_at`
- 엔드포인트: `GET /repos/{owner}/{repo}/commits?sha={branch}&path={path}&per_page=1`
- 응답: 배열의 첫 번째 commit의 `commit.committer.date` (ISO-8601).
- **주의**: 이 호출은 비싸므로 **차이가 있는 파일에 한해서만** 호출 (G-003). identical 파일에는 호출 금지. 호출 측(`scan::run`) 책임.

### Rate Limit 감지
응답 status 403 + `X-RateLimit-Remaining: 0` 헤더 → `RateLimitExceeded { reset_at: <X-RateLimit-Reset 헤더 ISO-8601 변환> }`.

### Truncation 감지
Trees API 응답 JSON에 `truncated: true` → 즉시 `TreesTruncated` 반환. 부분 결과 사용 금지.

### Backend 선택 (v0.1 stub + Phase 4 활성화)
- v0.1는 **REST backend만 활성**. GraphQL backend는 인터페이스만 박고 본체는 stub.
- `--backend rest` (기본): 본 spec § fetch_tree / fetch_blob / fetch_last_commit_at + § 병렬 호출 정책 그대로 동작.
- `--backend graphql`: `GitlessError::Config("GraphQL backend not implemented in v0.1; use --backend rest. Phase 4 ETA.")` 즉시 반환, exit code 1. (orchestrator `scan::run` 진입부에서 분기.)
- forward-compatibility 의도: 호출자(LLM)가 v0.1부터 `--backend graphql`을 명시 가능. Phase 4에서 GraphQL backend 본체 채울 때 호출자 코드 변경 0.
- Phase 4 활성화 시 본 섹션을 갱신: GraphQL endpoint (`/graphql`), alias batching 패턴, GraphQL 응답 → `Vec<RemoteFile>` / `DateTime<Utc>` 매핑.

### 병렬 호출 정책 (Latency)
- `fetch_last_commit_at`은 차이 있는 파일 N개에 대해 직렬로 호출하면 N × 100~300ms latency 누적 → 1000 drift 파일이면 분 단위 대기. rate limit(5,000/h) 한참 전에 사용자 인내심 한계.
- 해결: T09 (`scan::run` orchestrator)에서 **rayon으로 병렬 호출**, default **8 concurrent**.
- 패턴: `paths.par_iter().map(|p| github::fetch_last_commit_at(repo, branch, p, token)).collect::<Result<Vec<_>, _>>()`.
- ureq의 `Agent`는 thread-safe — caller가 한 번 생성하여 여러 thread에서 공유 가능 (또는 stateless 호출).
- 동시 요청 수 상한 = 8 (G-011, GitHub abuse detection 회피). 변경 시 G-011 갱신.
- burst 시 429 응답 가능성 → `GitlessError::Http(...)`로 매핑 후 즉시 종료. exponential backoff은 v0.1 비목표 (Phase 4).
- `fetch_tree`와 `fetch_blob`은 호출 횟수 자체가 적으므로 (Trees는 1회, Blob은 diff 명령에서만) 병렬화 대상 아님.

## Acceptance Criteria
- `[AUTO]` `fetch_tree`가 mockito 200 응답에서 `Vec<RemoteFile>` 반환 (blob entry만 필터). 단위 테스트.
- `[AUTO]` `fetch_tree`가 mockito 응답 `truncated: true` → `GitlessError::TreesTruncated` (PRD 검증 시나리오 12).
- `[AUTO]` `fetch_tree`가 mockito 401 응답 → `GitlessError::AuthFailed`.
- `[AUTO]` `fetch_tree`가 mockito 403 + `X-RateLimit-Remaining: 0` → `GitlessError::RateLimitExceeded`, `reset_at` 헤더 값 보존 (PRD 검증 시나리오 11).
- `[AUTO]` `fetch_tree`가 mockito 5xx → `GitlessError::Http(...)`.
- `[AUTO]` `fetch_blob`가 mockito 200 base64 응답을 raw bytes로 디코딩.
- `[AUTO]` `fetch_blob`가 잘못된 base64 응답 → `GitlessError::Http(...)` 또는 적절한 매핑.
- `[AUTO]` `fetch_last_commit_at`가 mockito 응답에서 첫 commit의 date를 `DateTime<Utc>`로 파싱.
- `[AUTO]` `fetch_last_commit_at`가 빈 commits 배열 응답 → `GitlessError::Http(...)` (예상 외 케이스).
- `[AUTO]` 모든 함수가 `User-Agent: gitless-sync/0.1` 헤더 송신 (mockito match로 검증).
- `[AUTO]` `fetch_last_commit_at`은 `Send + Sync` 호출 가능 (caller가 rayon `par_iter`로 동시 호출해도 안전). 단위 테스트에서 동일 함수를 여러 thread에서 동시 호출 → 모두 정상 결과.
- ~~`[HUMAN]` 실제 GitHub repo + 실제 PAT(Fine-grained `Contents: Read`)로 `fetch_tree` 1회 통합 검증.~~ **OBSOLETE (ADR 0001).** vault 실전 검증 (2026-04-29, OAuth via `gh auth token`, 356 파일)으로 입증. PAT 권한 가이드는 gh subprocess 채택으로 도구 책임 밖.
