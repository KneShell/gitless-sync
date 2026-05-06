# ADR 0002: v0.1 ureq → gh CLI subprocess 일괄 마이그레이션

- **Status**: Accepted
- **Date**: 2026-05-06
- **Resolves**: ADR 0001 § Follow-up Open Questions #1
- **Related**: ADR 0001 (gh subprocess 채택 + `gitless-push` 폐지)

## Context

ADR 0001은 Phase 4 GraphQL batching부터 `gh api graphql` subprocess 채택을 결정했으나, v0.1 기존 ureq 코드(scan/diff REST 호출)의 마이그레이션 시점은 open question으로 남겼다.

판단 기준은 **Claude Code 호출 마찰 제거 시점**이다.

| 선택지 | 특성 |
|---|---|
| 점진 (Phase 4 신규만 gh) | 두 backend(ureq + gh) 병존. 의존성·에러 매핑·테스트 분기 더블 트랙. 마찰 제거는 Phase 4 완성 후. |
| 일괄 (v0.1도 즉시 gh) | v0.1 ureq 코드 한 차례 제거로 종결. 이후 단일 경로. 마찰 즉시 해소. |

## Decision

**v0.1 scan/diff 명령의 GitHub REST 호출을 즉시 일괄 `gh api` subprocess로 전환한다.** ureq 구현은 제거한다.

`--backend rest|graphql` CLI flag는 유지한다 — `rest`는 "REST 단건 N× 호출"의 의미를, `graphql`은 Phase 4까지 stub의 의미를 그대로 가져간다. 호출 통로(ureq → gh api)만 바뀐다.

## Consequences

### 마이그레이션 작업 범위
- `commands/scan/github.rs::fetch_tree` / `fetch_blob` / `fetch_last_commit_at` 및 `_with_base` 변형을 `gh api` subprocess 호출로 재구현. JSON 응답은 stdout에서 파싱.
- `--token` CLI 인자 + `shared/config.rs::resolve_token` 토큰 해석 경로 제거. 인증은 `gh auth login` 한 줄로 단일화.
- `ureq`, `mockito` 의존성 제거.
- 통합 테스트(`tests/integration.rs`)는 mockito 기반이 깨짐. testability는 `GhClient` trait + `MockGhClient` inject 패턴으로 해결한다 (M2a~M2c). 가짜 `gh` 바이너리 PATH 주입 같은 외부 stub 전략은 채택하지 않는다.
- 통합 테스트용 `GITLESS_API_BASE` env 오버라이드(mockito URL 주입)도 의미 상실. 함께 제거 또는 testability 재설계.

### 호출자(Claude Code) 인터페이스
- `gitless-sync scan` 호출 시 `--token` 미사용. 사전 `gh auth login` 한 번이면 끝.
- 출력 JSON 스키마 변경 없음(`spec-output-schema.md` 그대로).
- `--backend rest|graphql` flag 의미 유지.

### guardrail 처분
- G-003(Commits API 호출 비용)와 G-011(rayon 8 concurrent abuse 회피)는 gh가 내부적으로 retry/backoff/abuse detection을 처리하므로 도구 책임 종료. 다만 병렬 subprocess spawn 비용 vs 순차 호출 시간 trade-off는 측정해 rayon 유지/제거 결정 — 별도 task.

### 에러 매핑
- `GitlessError::AuthFailed` / `RateLimitExceeded` / `Http(...)` / `TreesTruncated` variant는 gh 종료 코드 + stderr 파싱으로 재매핑. 구체 매핑 표는 마이그레이션 task 진행 중 `spec-error-contracts.md`에 박는다.

### 의존성 안내
- README에 `gh` CLI(>= 2.x) 사전 설치 요구사항 명시. gh 미설치 시 명확한 에러 (`GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")`).

## References

- ADR 0001 § Follow-up Open Questions #1
- 사용자 결정 (2026-05-06)
