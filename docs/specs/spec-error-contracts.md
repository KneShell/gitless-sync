# Spec: Operational Error Contracts

> **2026-05-06 (M1)**: ADR 0001 + ADR 0002 정합 부분 갱신. mockito 시나리오 → MockGhClient stub 표현으로 재작성. gh subprocess 종료 코드 + stderr substring 매핑 표 신설. gh CLI floor 박제.

## 목적
read-only CLI라도 호출자(특히 AI)가 안정적으로 다룰 수 있도록 6개 contract 정의: custom error / exit code / stderr JSON / stdout-stderr 분리 / partial failure / 인증·rate limit 동작.

본 문서의 매핑 source는 **`gh api` subprocess의 종료 코드 + stderr substring**이다. v0.1 ureq baseline의 HTTP status code 직접 관찰 매핑은 ADR 0002로 obsolete. 매핑 표는 본 spec § gh 종료 코드 + stderr → GitlessError 매핑 한 곳에 집중.

## 현재 상태
- `crates/gitless-sync/src/shared/error/{mod,core,network}.rs`에 `GitlessError` enum + `exit_code()` + `error_code()` + `to_stderr_payload()` 모두 구현 완료. 단일 file → 도메인 sub-module 분리(Phase 6 Q task) 적용.
- `main.rs`에서 에러 발생 시 stderr JSON 출력 + exit code 매핑 동작.
- 각 GitHub API / IO 호출 지점에서 `GitlessError` variant 매핑 + partial failure 누적 로직 모두 구현 (ADR 0002 마이그레이션 완료, 2026-05-07).
- **gh CLI 최소 버전 floor**: `gh >= 2.40.0` (2023-12-07 릴리스). 근거: 멀티 계정 인증(`gh auth switch`/`gh auth status` 정비)이 도입된 시점으로 토큰/호스트 해석이 안정적. `gh api`의 `-F`/`-f`/recursive query는 그 전부터 안정이었으나 인증 측 안정성을 floor로 잡는다 [source: https://github.com/cli/cli/releases/tag/v2.40.0]. 현재 최신은 v2.92.0 (2026-04-28) 기준 [source: https://github.com/cli/cli/releases].

## 작업 범위

### Module Layout (Q, 2026-05-08)

`shared/error/` 폴더 구조 (단일 파일 → 도메인 sub-module 분리, spec-architecture.md § 구조적 분리):

| 파일 | 책임 |
|---|---|
| `shared/error/mod.rs` | re-export hub. `pub mod core; pub mod network;` + `pub use core::{GitlessError, StderrPayload}; pub use network::{GraphqlError, GraphqlErrorExtensions, map_graphql_error};` |
| `shared/error/core.rs` | `GitlessError` enum 정의 + `exit_code()` / `error_code()` / `to_stderr_payload()` + `StderrPayload` |
| `shared/error/network.rs` | GraphQL 응답 매핑 — `GraphqlError`, `GraphqlErrorExtensions`, `map_graphql_error`, `format_graphql_errors` |

호출자 import는 `use crate::shared::error::{GitlessError, GraphqlError, map_graphql_error};` 형태로 호환 유지 (sub-module 위치 변경에도 path 동일). REST-side gh stderr substring 매칭은 본 모듈이 아닌 call site 인접 `shared/github/error_map.rs`에 위치 — gh 종료 코드 + stderr 스냅샷이 GitHub API 호출 모듈의 자연스러운 동반자라.

mod.rs를 re-export 전용으로 둔 이유: sub-module이 parent의 type을 import하면 cargo modules 그래프에서 양방향 edge가 만들어져 cycle 게이트(`cargo xtask check-cycles`) deny. github/mod.rs(2026-05-08, task G) 동일 패턴 — sub-module 간 sibling import만 허용 + parent는 re-export hub.

### Custom Error Types (이미 구현됨)
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

### GraphQL error mapping (Phase 4)

> **신설 (Phase 4 P2, 2026-05-07)**: GraphQL backend(`gh api graphql`)는 gh subprocess가 exit 0으로 떨어져도 응답 JSON 안의 `errors[]` 배열에 에러를 포함할 수 있다 (REST의 stderr substring 패턴과 다른 신호 경로). 매핑 source는 응답 JSON의 `data.errors[].extensions.code` enum 값.

GraphQL 응답 형식 (errors 동반 케이스):

```json
{
  "data": { ... },
  "errors": [
    {
      "message": "...",
      "extensions": { "code": "RATE_LIMITED" }
    }
  ]
}
```

**매핑 표** (response가 partial result 포함해도 통째 fail — partial errors 정책):

| `errors[].extensions.code` | 매핑 | exit | 출처 |
|---|---|---|---|
| `RATE_LIMITED` | `RateLimitExceeded { reset_at: "" }` | 3 | [source: https://docs.github.com/en/graphql/overview/resource-limitations] |
| `UNAUTHENTICATED` | `AuthFailed` | 2 | GitHub GraphQL standard error code (token 만료 / 미인증) |
| `NOT_FOUND` | `Http(errors[] 원문)` | 1 | GraphQL standard error code (repo / branch / object 미존재) |
| 그 외 (예: `INTERNAL_SERVER_ERROR`) | `Http(errors[] 원문)` | 1 | fallthrough — 알려지지 않은 enum 코드 |

**매칭 우선순위** (위에서 아래로):
1. gh subprocess exit ≠ 0이면 REST과 동일 § gh 종료 코드 + stderr 매핑 우선 적용 (gh 자체가 5xx / auth 등을 stderr에 노출).
2. gh subprocess exit == 0 + stdout JSON `errors[]` 비어 있지 않음 → 본 § GraphQL error mapping 표 적용.
3. `errors[0].extensions.code` 값으로 표 매핑. 첫 항목 우선 (다중 errors 시 첫 항목만 매핑, 나머지 errors 원문은 message에 보존).
4. fallthrough → `Http(errors[] 원문)`.

**정규식 사용 금지**. enum 코드는 substring 비교가 아닌 정확 일치 (`code == "RATE_LIMITED"`). gh subprocess 자체 stderr 매칭은 § gh 종료 코드 + stderr 표 그대로.

**REST stderr 매핑과의 우선순위 일관**: 같은 운영 신호(rate limit / auth fail)는 두 backend에서 같은 `GitlessError` variant + 같은 exit code로 떨어진다. 호출자(LLM)는 backend에 무관하게 exit code + error_code로 분기 가능.

**reset_at 필드 한계**: REST과 동일하게 v0.1에서 빈 문자열. GraphQL 응답에 reset 시각이 일관 노출되지 않음 [unverified].

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

### Per-file Pitfall Reasons (Phase 5)

`Status::Failed` 항목은 `failed_reason` 필드(spec-output-schema.md § 1.1)로 함정 종류 구분. fatal error(`GitlessError`) 아님 — per-file partial failure 카운트 누적.

| reason | 상황 | 처리 정책 | 구현 |
|---|---|---|---|
| `hash_io` | 로컬 파일 read / 권한 실패 | v0.1 기존 동작 | `compare.rs::FailedReason` enum에 미정의 (None special case). `pipeline.rs::build_one_pre_entry` line 122~128 — `failed_reason: None` 적용. v1.0 backward-compat |
| `encoding` | UTF-8 + 2차 detect 모두 실패 또는 UTF-16 BOM detect | spec-domain-pitfalls.md § Encoding | `compare.rs::FailedReason::Encoding` 정의됨 + `commands/scan/hash_local.rs::try_hash_local`가 raw read 1회 시점에 `try_decode_text` 결과 분기 (`Utf16Bom`/`Unknown` → `Some(Encoding)`) + `pipeline::build_one_pre_entry`가 PreState::Failed 격상. Phase 5.13 task AA |
| `submodule` | Trees mode `160000` entry | spec-domain-pitfalls.md § Submodule | `compare.rs::FailedReason::Submodule` 정의됨 + `pipeline.rs::try_short_circuit_failed` line 158~159 구현 |
| `symlink` | Trees mode `120000` entry 또는 local symlink | spec-domain-pitfalls.md § Symlink | `compare.rs::FailedReason::Symlink` 정의됨 + `pipeline.rs::try_short_circuit_failed` line 160~161 구현 |
| `lfs_pointer` | LFS pointer text 시그니처 detect | spec-domain-pitfalls.md § LFS pointer | `compare.rs::FailedReason::LfsPointer` 정의됨 + `pipeline.rs::try_short_circuit_failed` line 162~163 구현 + `commands/scan/lfs.rs::placeholder_pointer_for` 구현 |
| `long_path` | Windows 260자+ path 또는 예약 파일명 (CON/PRN/NUL/AUX 등) | spec-domain-pitfalls.md § Windows long path | `compare.rs::FailedReason::LongPath` 정의됨 + `pipeline.rs::try_short_circuit_failed` line 156~157 구현 |
| `nfd_collision` | macOS NFD/NFC 동일 path 두 개 공존 (precomposeunicode false 환경) | spec-domain-pitfalls.md § NFD edge | `compare.rs::FailedReason::NfdCollision` 정의됨 + `commands/scan/nfd_collision.rs::detect` group-by NFC key count ≥ 2 (walker output `&[LocalFile]`, HashMap dedup 전) + `pipeline::try_short_circuit_failed` cascade 첫 분기. Phase 5.13 task AA |
| `case_collision` | Windows local에서 case-sensitive 충돌 (`Foo.txt` + `foo.txt`) | spec-domain-pitfalls.md § Case | `compare.rs::FailedReason::CaseCollision` 정의됨 + `pipeline.rs::try_short_circuit_failed` line 154~155 구현 + `case_collision::detect` 구현 |
| `gitattributes_unsupported` | `.gitattributes`에 화이트리스트 외 attribute 적용 (예: `working-tree-encoding`, `ident`, `filter` (lfs 외)) | spec-domain-pitfalls.md § `.gitattributes` 화이트리스트 | `compare.rs::FailedReason::GitattributesUnsupported` 정의됨 + `pipeline::try_short_circuit_failed` `.gitattributes` match arm이 `AttributeMatch::Unsupported { .. }` → `FailedReason::GitattributesUnsupported`. `prepare_for_hash`는 v0.1 default fall-through 그대로 (defensive). Phase 5.13 task AA |
| `file_too_large` | local 또는 remote file size 100 MB (GitHub Blobs API hard limit) 초과 | spec-hash-and-normalize.md § Phase 7 — 큰 파일 처리 | Phase 7 신규. `compare.rs::FailedReason::FileTooLarge` + `size_bytes` field. local: `try_hash_local` size pre-flight (`fs::metadata().len()`). remote: Trees response size field pre-flight + fetch_blob 응답 size post-flight. |
| `memory_exceeded` | local 또는 remote file size 50 MB (tool 메모리 안전 임계) 초과 | spec-hash-and-normalize.md § Phase 7 — 큰 파일 처리 | Phase 7 신규. `compare.rs::FailedReason::MemoryExceeded` + `size_bytes` field. cascade에서 `file_too_large` 다음 (50 MB ≤ size < 100 MB 범위 적용). |

`failed_reason` 부재(`null`) 시 v0.1 baseline `hash_io` 동작과 일관 — 호출자 backward-compat.

### 인증 실패 / Rate Limit / Trees Truncated 동작
매핑 source는 위 § gh 종료 코드 + stderr → GitlessError 매핑 표. 기존 v0.1 ureq HTTP status 직접 관찰은 ADR 0002로 obsolete.

- **AuthFailed**: gh stderr `"Bad credentials"` 검출 → 즉시 종료, exit 2, stdout 출력 안 함. 원인 안내(stderr JSON `message` 필드): `gh auth login` 미수행 또는 토큰 만료. 본 도구는 PAT를 직접 보지 않으므로 권한 가이드(Contents:Read 등)는 도구 책임 밖 — gh 측에 위임.
- **RateLimitExceeded**: gh stderr `"API rate limit exceeded"` 또는 `"secondary rate limit"` 검출 → 즉시 종료, exit 3, stderr에 원문 메시지(reset 시각이 stderr에 노출되면 포함, 부재 시 빈 문자열). 부분 결과 출력 안 함 (재시도 가능).
- **TreesTruncated**: `gh api repos/{owner}/{repo}/git/trees/{branch}?recursive=1` 응답 JSON `truncated: true` 검출 → exit 5, stderr에 안내. v0.1 큰 repo(7MB / 100k entry 초과) 미지원. 부분 결과 사용 금지(G-002).

### init 에러 케이스 (Phase 2)

`gitless-sync init`은 외부 호출이 없으므로 발생 가능 에러는 `Config` variant 단일.

| 조건 | Variant | exit | stderr error_code | message |
|---|---|---|---|---|
| `--repo` 미명시 또는 빈 문자열 | `Config("repo not specified")` | 1 | `CONFIG` | `repo not specified` |

기타 케이스(파일 권한 / 기존 파일 충돌 / `--force`)는 ADR 0004로 obsolete — 도구 파일 작성 0이라 발생 자체 불가능. shell redirect 측 에러는 도구 책임 밖.

## Acceptance Criteria
단위 테스트는 모두 `MockGhClient` stub 응답 기반 (M2a~M2c, ADR 0002). v0.1 ureq baseline 시기에 사용된 mockito 시나리오는 모두 stub 응답으로 재작성한다.

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
- `[AUTO]` gh 미설치 환경: `RealGhClient::new().api(&["api".to_string(), "...".to_string()])` 첫 호출이 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환 → 도구 exit code 1 + stderr `error_code: "CONFIG_ERROR"`.
- `[AUTO]` 5xx fallthrough: `MockGhClient` stub 응답 stderr `"gh: ... (HTTP 503)"` + exit 1 → `GitlessError::Http(...)` → 도구 exit code 1 + stderr `error_code: "HTTP_ERROR"`. **N-task audit drift**: 본 spec § Exit Code 매핑 line `1` vs `error/core.rs::exit_code()` `3` (ureq 시절 잔재 의심) — 본 task scope 밖 fix follow-up. 본 line은 spec § stderr 출력 형식 § 1:1 매핑 원칙 + `error_code()` 메서드 결과 일관 정합 명시.
- `[AUTO]` PRD 검증 시나리오 17 (init repo 미명시): `cargo run -- init` (또는 `cargo run -- init --repo ""`) → 도구 exit code 1, stdout 출력 0, stderr JSON 한 줄에 `error_code: "CONFIG_ERROR"` + `message`에 `"repo not specified"` substring. library 경로로는 `commands::init::run(&InitArgs { repo: "".into(), .. }, &mut Vec<u8>)` → `Err(GitlessError::Config(_))` 반환 + `err.exit_code() == 1` + `err.error_code() == "CONFIG_ERROR"`.
- `[AUTO]` GraphQL backend `errors[].extensions.code == "RATE_LIMITED"` 응답 → 도구 exit code 3 + stderr `error_code: "RATE_LIMIT_EXCEEDED"` (P5a 단위 테스트 매트릭스).
- `[AUTO]` GraphQL backend `errors[].extensions.code == "UNAUTHENTICATED"` 응답 → 도구 exit code 2 + stderr `error_code: "AUTH_FAILED"`.
- `[AUTO]` GraphQL backend `errors[].extensions.code == "NOT_FOUND"` 응답 → 도구 exit code 1 + stderr `error_code: "HTTP_ERROR"`.
- `[AUTO]` GraphQL backend fallthrough 코드 (`INTERNAL_SERVER_ERROR` 등) → 도구 exit code 1 + stderr `error_code: "HTTP_ERROR"` + `message`에 errors[] 원문 보존.
- `[AUTO]` GraphQL 응답에 `data` 부분 결과 + `errors[]` 비어 있지 않음 → data 무시, errors[0] 매핑 후 통째 fail (partial errors 정책 일관).
