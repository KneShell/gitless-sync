# Spec: Operational Error Contracts

> **2026-05-06 (M1)**: ADR 0001 + ADR 0002 정합 부분 갱신. mockito 시나리오 → MockGhClient stub 표현으로 재작성. gh subprocess 종료 코드 + stderr substring 매핑 표 신설. gh CLI floor 박제.

## 목적
read-only CLI라도 호출자(특히 AI)가 안정적으로 다룰 수 있도록 6개 contract를 박는다: custom error / exit code / stderr JSON / stdout-stderr 분리 / partial failure / 인증·rate limit 동작.

본 문서의 매핑 source는 **`gh api` subprocess의 종료 코드 + stderr substring**이다. v0.1 ureq baseline의 HTTP status code 직접 관찰 매핑은 ADR 0002로 obsolete. 매핑 표는 본 spec § gh 종료 코드 + stderr → GitlessError 매핑 한 곳에 박는다.

## 현재 상태
- `crates/gitless-sync/src/shared/error.rs`에 `GitlessError` enum + `exit_code()` + `error_code()` + `to_stderr_payload()` 모두 구현 완료.
- `main.rs`에서 에러 발생 시 stderr JSON 출력 + exit code 매핑 동작.
- 각 GitHub API / IO 호출 지점에서 `GitlessError` variant 매핑 + partial failure 누적 로직 모두 박힘 (ADR 0002 마이그레이션 완료, 2026-05-07).
- **gh CLI 최소 버전 floor**: `gh >= 2.40.0` (2023-12-07 릴리스). 근거: 멀티 계정 인증(`gh auth switch`/`gh auth status` 정비)이 박힌 시점으로 토큰/호스트 해석이 안정적. `gh api`의 `-F`/`-f`/recursive query는 그 전부터 안정이었으나 인증 측 안정성을 floor로 잡는다 [source: https://github.com/cli/cli/releases/tag/v2.40.0]. 현재 최신은 v2.92.0 (2026-04-28) 기준 [source: https://github.com/cli/cli/releases].

## 작업 범위

### Custom Error Types (이미 박힘)
```rust
pub enum GitlessError {
    Config(String),
    AuthFailed,
    RateLimitExceeded { reset_at: String },
    TreesTruncated,
    Http(String),
    Io(#[from] std::io::Error),
    PartialFailure { failed_count: usize },
}
```

variant 의미:
- `Config(String)`: CLI 인자 / 설정 / 환경 문제. **gh CLI 미설치(`Command::new("gh")` IO 에러)도 본 variant로 매핑** — 메시지: `"gh CLI not found in PATH; install from https://cli.github.com/"`.
- `AuthFailed`: `gh api` stderr substring `"Bad credentials"` 매칭 (HTTP 401). reset 가능 토큰 만료 / 잘못된 인증 / `gh auth login` 미수행.
- `RateLimitExceeded { reset_at }`: `gh api` stderr substring `"API rate limit exceeded"` (primary) 또는 `"secondary rate limit"` (secondary) 매칭. `reset_at`은 가능하면 stderr에서 추출, 부재 시 빈 문자열 (gh가 reset 시각을 항상 stderr로 노출하지 않음 — `[unverified]`).
- `TreesTruncated`: Trees API 응답 JSON의 `truncated: true` 필드 직접 검사. **gh subprocess는 이를 stderr로 알리지 않음** — stdout JSON 파싱 후 우리 도구가 검출 [source: https://docs.github.com/en/rest/git/trees].
- `Http(String)`: **gh subprocess 비정상 종료(인증/rate/truncated 외)**. 5xx, 404, JSON parse 실패, 또는 위 substring 어느 것에도 매칭하지 않는 fallthrough. 메시지에 stderr 원문 보존(JSON 한 줄 escape 안전 길이 내).
- `Io(std::io::Error)`: 로컬 디렉토리 walk / 파일 read 시 IO 실패. gh subprocess의 IO err은 `Config`로 매핑(위 항목).
- `PartialFailure { failed_count }`: 일부 파일 해시 실패 — 결과는 출력하되 카운트만 누적.

### Exit Code 매핑
| Code | 의미 | Variant |
|------|------|---------|
| 0 | 정상 (drift 존재 여부와 무관) | `Ok(())` |
| 1 | 사용자 입력 오류 / gh 미설치 / 5xx 등 기타 HTTP | `Config`, `Io`, `Http` |
| 2 | 인증 실패 | `AuthFailed` |
| 3 | GitHub API rate limit | `RateLimitExceeded` |
| 4 | 부분 성공 (결과는 출력되지만 일부 파일 누락) | `PartialFailure` |
| 5 | Trees truncated (repo 너무 큼, G-002) | `TreesTruncated` |

### gh 종료 코드 + stderr → GitlessError 매핑 (M1 신설)

**중요 사실**: `gh api`는 HTTP 4xx/5xx 모두 **exit code 1 단일**로 떨어진다. exit code만으로는 케이스 구분 불가 — stderr substring이 sole signal이다 [source: https://github.com/cli/cli/issues/9338, gh가 401→exit 4 분기를 자체 미구현]. stderr 포맷은 `gh: <serverError> (HTTP <code>)` 또는 본문 파싱 실패 시 `gh: HTTP <code>` 단일 패턴 [source: https://raw.githubusercontent.com/cli/cli/trunk/pkg/cmd/api/api.go, parseErrorResponse 함수].

**매칭 룰** (정규식 금지, substring contains 체크만):

| gh 종료 신호 | stderr substring (좁은 매칭) | 매핑 | 출처 |
|---|---|---|---|
| Command::new IO 에러 (`ErrorKind::NotFound` 등) | (해당 없음 — 호출 자체 실패) | `Config("gh CLI not found in PATH; install from https://cli.github.com/")` | std::process::Command 동작 |
| exit 0 + stdout JSON에 `truncated: true` (Trees API) | (해당 없음) | `TreesTruncated` | [source: https://docs.github.com/en/rest/git/trees] |
| exit 1 + stderr contains `"Bad credentials"` | `Bad credentials` (예: `gh: Bad credentials (HTTP 401)`) | `AuthFailed` | [source: https://github.com/cli/cli/issues/9338, 실제 출력 인용] |
| exit 1 + stderr contains `"secondary rate limit"` | `secondary rate limit` (예: `gh: You have exceeded a secondary rate limit ... (HTTP 403)`) | `RateLimitExceeded { reset_at: "" }` | [source: https://github.com/orgs/community/discussions/32120] |
| exit 1 + stderr contains `"API rate limit exceeded"` | `API rate limit exceeded` (예: `gh: API rate limit exceeded for user XXX. (HTTP 403)`) | `RateLimitExceeded { reset_at: "" }` | [source: https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api] |
| exit 1 + stderr contains `"HTTP 5"` (5xx 그룹) | `HTTP 5` (예: `gh: ... (HTTP 503)` 또는 `gh: HTTP 500`) | `Http(stderr 원문)` | [source: https://raw.githubusercontent.com/cli/cli/trunk/pkg/cmd/api/api.go, api.go의 `serverError = fmt.Sprintf("HTTP %d", ...)`] |
| exit 1 + 위 어느 것에도 미매칭 (404 / parse 에러 / 기타) | (fallthrough) | `Http(stderr 원문)` | parseErrorResponse fallback |
| exit 2 (사용자 Ctrl-C 같은 signal) | (해당 없음) | (gitless-sync는 gh를 동기 spawn — Ctrl-C는 본 도구가 직접 받음, gh 단독 신호 처리 케이스 미발생 [unverified]) | [source: https://raw.githubusercontent.com/cli/cli/trunk/internal/ghcmd/cmd.go, exitCancel = 2] |

**매칭 우선순위** (위에서 아래로):
1. IO 에러(`Command::new` 실패) → `Config`
2. exit 0 + stdout truncated 검사 → `TreesTruncated`
3. exit 1 + stderr `"Bad credentials"` → `AuthFailed`
4. exit 1 + stderr `"secondary rate limit"` → `RateLimitExceeded` (primary substring과 겹치지 않으므로 어느 순서든 안전 — but 명시적 우선순위로 secondary 먼저)
5. exit 1 + stderr `"API rate limit exceeded"` → `RateLimitExceeded`
6. exit 1 + stderr `"HTTP 5"` → `Http`
7. exit 1 + 그 외 → `Http(stderr 원문)`

**정규식 사용 금지.** substring contains 체크만 (`stderr.contains("Bad credentials")` 식). 매칭이 너무 좁아 미매칭 케이스가 fallthrough하면 `Http`로 떨어지는 게 안전 — `AuthFailed` / `RateLimit` 오분류는 사용자 액션 오도로 더 위험.

**reset_at 추출 한계**: `RateLimitExceeded { reset_at }`의 `reset_at` 필드는 v0.1에서 빈 문자열로 둔다. gh stderr가 X-RateLimit-Reset 헤더 값을 항상 노출하지 않음 [unverified]. Phase 4에서 `gh api -i`(헤더 포함) 옵션 도입 검토.

### stderr 출력 형식 (G-008)
- stdout: 결과 JSON 전용. 다른 출력 일체 금지.
- stderr: 진행 로그(verbose 레벨), 경고, 에러 JSON.
- 에러 JSON 한 줄 형식:
  ```json
  {"error_code": "AUTH_FAILED", "message": "gh: Bad credentials (HTTP 401)", "context": {}}
  ```
  `error_code`는 `GitlessError` enum과 1:1 매핑 (`error_code()` 메서드 결과). `message`는 가능하면 gh stderr 원문 보존(escape 처리).
- verbose: 기본 warning 이상. `-v` info, `-vv` debug.

### Partial Failure 표현
일부 파일 해시 실패 시:
- 전체 결과는 출력 (stdout 정상 JSON).
- `summary.failed` 카운트 증가.
- `files[]`에 해당 항목 `status: "failed"`로 포함 (별도 `failed[]` 배열은 두지 않음 — 단일 배열 + `Status::Failed`).
- exit code 4.

### 인증 실패 / Rate Limit / Trees Truncated 동작
매핑 source는 위 § gh 종료 코드 + stderr → GitlessError 매핑 표. 기존 v0.1 ureq HTTP status 직접 관찰은 ADR 0002로 obsolete.

- **AuthFailed**: gh stderr `"Bad credentials"` 검출 → 즉시 종료, exit 2, stdout 출력 안 함. 원인 안내(stderr JSON `message` 필드): `gh auth login` 미수행 또는 토큰 만료. 본 도구는 PAT를 직접 보지 않으므로 권한 가이드(Contents:Read 등)는 도구 책임 밖 — gh 측에 위임.
- **RateLimitExceeded**: gh stderr `"API rate limit exceeded"` 또는 `"secondary rate limit"` 검출 → 즉시 종료, exit 3, stderr에 원문 메시지(reset 시각이 stderr에 노출되면 포함, 부재 시 빈 문자열). 부분 결과 출력 안 함 (재시도 가능).
- **TreesTruncated**: `gh api repos/{owner}/{repo}/git/trees/{branch}?recursive=1` 응답 JSON `truncated: true` 검출 → exit 5, stderr에 안내. v0.1 큰 repo(7MB / 100k entry 초과) 미지원. 부분 결과 사용 금지(G-002).

## Acceptance Criteria
단위 테스트는 모두 `MockGhClient` stub 응답 기반 (M2a~M2c, ADR 0002). v0.1 ureq baseline 시기에 박혀 있던 mockito 시나리오는 모두 stub 응답으로 재작성한다.

- `[AUTO]` `GitlessError::AuthFailed.exit_code()` == `2`.
- `[AUTO]` `GitlessError::TreesTruncated.exit_code()` == `5`.
- `[AUTO]` `GitlessError::PartialFailure { failed_count: 3 }.exit_code()` == `4`.
- `[AUTO]` `to_stderr_payload(&AuthFailed).error_code` == `"AUTH_FAILED"`.
- `[AUTO]` `to_stderr_payload(&RateLimitExceeded { reset_at: "..." }).context` 가 JSON object `{"reset_at": "..."}` 포함.
- `[AUTO]` `to_stderr_payload(&PartialFailure { failed_count: 5 }).context` 가 `{"failed_count": 5}` 포함.
- `[AUTO]` PRD 검증 시나리오 10 (인증 실패): `MockGhClient` stub 응답 stderr `"gh: Bad credentials (HTTP 401)"` + exit 1 → 도구 exit code 2 + stderr에 `error_code: "AUTH_FAILED"` JSON.
- `[AUTO]` PRD 검증 시나리오 11 (rate limit): `MockGhClient` stub 응답 stderr `"gh: API rate limit exceeded for user XXX. (HTTP 403)"` + exit 1 → 도구 exit code 3 + stderr `RATE_LIMIT_EXCEEDED`. 추가로 secondary 케이스(`"secondary rate limit"` substring) stub 응답도 동일 매핑 검증.
- `[AUTO]` PRD 검증 시나리오 12 (truncated): `MockGhClient` stub 응답 stdout JSON에 `"truncated": true` + exit 0 → 도구 exit code 5 + stderr `TREES_TRUNCATED`.
- `[AUTO]` PRD 검증 시나리오 15 (partial failure): 일부 파일 해시 실패 (예: 권한 없는 파일) → 도구 exit code 4, stdout JSON에 `summary.failed > 0`, 해당 파일 `status: "failed"`.
- `[AUTO]` 정상 실행 (drift 있어도) → 도구 exit code 0.
- `[AUTO]` stdout이 결과 JSON 한 덩어리만 포함하고 추가 텍스트 없음 (`serde_json::from_str` 가능).
- `[AUTO]` gh 미설치 환경: `RealGhClient::new().api(&["api".to_string(), "...".to_string()])` 첫 호출이 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환 → 도구 exit code 1 + stderr `error_code: "CONFIG"`.
- `[AUTO]` 5xx fallthrough: `MockGhClient` stub 응답 stderr `"gh: ... (HTTP 503)"` + exit 1 → `GitlessError::Http(...)` → 도구 exit code 1 + stderr `error_code: "HTTP"`.
