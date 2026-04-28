# Spec: GitHub API Integration

## 목적
GitHub Trees / Blobs / Commits API를 blocking `ureq`로 호출. 인증·rate limit·truncation을 구조화 에러로 매핑.

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
- `[HUMAN]` 실제 GitHub repo + 실제 PAT(Fine-grained `Contents: Read`)로 `fetch_tree` 1회 통합 검증. `docs/roadmap.md` Open Question 해소용.
