# Spec: Operational Error Contracts

## 목적
read-only CLI라도 호출자(특히 AI)가 안정적으로 다룰 수 있도록 6개 contract를 박는다: custom error / exit code / stderr JSON / stdout-stderr 분리 / partial failure / 인증·rate limit 동작.

## 현재 상태
- `crates/gitless-sync/src/shared/error.rs`에 `GitlessError` enum + `exit_code()` + `error_code()` + `to_stderr_payload()` 모두 구현 완료.
- `main.rs`에서 에러 발생 시 stderr JSON 출력 + exit code 매핑 동작.
- 빠진 것: 각 GitHub API / IO 호출 지점에서 적절한 `GitlessError` variant 매핑, partial failure 누적 로직.

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

### Exit Code 매핑
| Code | 의미 | Variant |
|------|------|---------|
| 0 | 정상 (drift 존재 여부와 무관) | `Ok(())` |
| 1 | 사용자 입력 오류 | `Config`, `Io` |
| 2 | 인증 실패 | `AuthFailed` |
| 3 | GitHub API 오류 (rate limit, 5xx 등) | `RateLimitExceeded`, `Http` |
| 4 | 부분 성공 (결과는 출력되지만 일부 파일 누락) | `PartialFailure` |
| 5 | Trees truncated (repo 너무 큼, G-002) | `TreesTruncated` |

### stderr 출력 형식 (G-008)
- stdout: 결과 JSON 전용. 다른 출력 일체 금지.
- stderr: 진행 로그(verbose 레벨), 경고, 에러 JSON.
- 에러 JSON 한 줄 형식:
  ```json
  {"error_code": "AUTH_FAILED", "message": "GITHUB_TOKEN unauthorized", "context": {"status": 401}}
  ```
  `error_code`는 `GitlessError` enum과 1:1 매핑 (`error_code()` 메서드 결과).
- verbose: 기본 warning 이상. `-v` info, `-vv` debug.

### Partial Failure 표현
일부 파일 해시 실패 시:
- 전체 결과는 출력 (stdout 정상 JSON).
- `summary.failed` 카운트 증가.
- `files[]`에 해당 항목 `status: "failed"`로 포함 (별도 `failed[]` 배열은 두지 않음 — 단일 배열 + `Status::Failed`).
- exit code 4.

### 인증 실패 / Rate Limit / Trees Truncated 동작
- **AuthFailed**: 즉시 종료, exit 2, stdout 출력 안 함.
- **RateLimitExceeded**: 즉시 종료, exit 3, stderr에 `reset_at` 시각 명시. 부분 결과 출력 안 함 (재시도 가능).
- **TreesTruncated**: exit 5, stderr에 안내. v0.1 큰 repo 미지원.

## Acceptance Criteria
- `[AUTO]` `GitlessError::AuthFailed.exit_code()` == `2`.
- `[AUTO]` `GitlessError::TreesTruncated.exit_code()` == `5`.
- `[AUTO]` `GitlessError::PartialFailure { failed_count: 3 }.exit_code()` == `4`.
- `[AUTO]` `to_stderr_payload(&AuthFailed).error_code` == `"AUTH_FAILED"`.
- `[AUTO]` `to_stderr_payload(&RateLimitExceeded { reset_at: "..." }).context` 가 JSON object `{"reset_at": "..."}` 포함.
- `[AUTO]` `to_stderr_payload(&PartialFailure { failed_count: 5 }).context` 가 `{"failed_count": 5}` 포함.
- `[AUTO]` PRD 검증 시나리오 10: 토큰 미설정 → exit code 2 + stderr에 `error_code: "AUTH_FAILED"` JSON (통합 테스트, env 변수 클리어).
- `[AUTO]` PRD 검증 시나리오 11: rate limit 시뮬레이션 (mockito 403 + `X-RateLimit-Remaining: 0`) → exit code 3 + stderr `RATE_LIMIT_EXCEEDED`.
- `[AUTO]` PRD 검증 시나리오 12: Trees truncated 시뮬레이션 (mockito 응답 `truncated: true`) → exit code 5 + stderr `TREES_TRUNCATED`.
- `[AUTO]` PRD 검증 시나리오 15: 일부 파일 해시 실패 (예: 권한 없는 파일) → exit code 4, stdout JSON에 `summary.failed > 0`, 해당 파일 `status: "failed"`.
- `[AUTO]` 정상 실행 (drift 있어도) → exit code 0.
- `[AUTO]` stdout이 결과 JSON 한 덩어리만 포함하고 추가 텍스트 없음 (`serde_json::from_str` 가능).
