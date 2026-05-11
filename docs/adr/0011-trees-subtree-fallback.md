# ADR 0011: Trees API truncated repo sub-tree 재귀 fallback

- **Status**: Accepted
- **Date**: 2026-05-10
- **Related**: `docs/specs/spec-github-api.md` § Trees truncation handling, `docs/ralph/guardrails.md` § G-002, `docs/ralph/guardrails.md` § G-019

## Context

GitHub Trees API recursive=1 응답은 100,000 entry + 7MB 둘 중 먼저 도달 시 `truncated: true` 반환 [source: https://docs.github.com/en/rest/git/trees] (2026-05-10 fact check). 공식 권장: "use the non-recursive method of fetching trees, and fetch one sub-tree at a time".

v0.2.x까지는 G-002로 truncated 시 즉시 `GitlessError::TreesTruncated` 반환 + exit 5 (부분 결과 사용 금지). monorepo / 큰 vault 사용자 차단 — Phase 7에서 sub-tree 재귀 fallback 도입.

## Decision

sub-tree non-recursive 재귀 fallback 도입. v0.2.x 정책 (truncated 즉시 fail)은 v0.3.0부터 sub-tree fallback 진입 후 실패에만 적용.

### 2 cap (yagni 일관)

| 상수 | 값 | 근거 |
|---|---|---|
| `MAX_TREE_CALL_BUDGET` | 1000 | linux/torvalds 기준 sub-tree 호출 약 5000 (truncated 케이스 가정). 1000 cap = 약 200K entry vault 한도 추정 + GitHub rate limit (5000/h auth) safety. |
| `MAX_TREE_ENTRIES` | 500_000 | 누적 entry 한도. 도달 시 early-abort (메모리 안전). |

depth cap / wall-clock cap은 monorepo 측정 도달 (depth 20+ 또는 호출 시간 600s+) 시 추가 검토. 초기 spec은 2 cap만 (yagni — 추측 기반 cap 추가 회피).

### sha 일관성

- Trees fallback 진입 직전 1회 ref → commit sha → root tree sha resolve.
- 모든 sub-tree 호출은 immutable tree sha 직접 사용 (branch 이름 / ref 사용 금지).
- HEAD drift 차단 — resolve 시점과 sub-tree 호출 시점이 다른 commit이라도 동일 root tree sha 위에서 평가.

### early-abort

- 2 cap 중 하나 초과 시 `GitlessError::TreesTruncated` 즉시 반환 + entries 무시.
- 부분 결과 사용 금지 — G-002 정책 일관.

## Consequences

- spec-github-api.md § Trees truncation handling 신규 § + § fetch_tree truncated 처리 update.
- G-002 본문 update (Phase 7부터 sub-tree fallback 진입).
- 신규 unit test 2 시나리오 (call budget 1001 + entries 500_001).
- 합성 truncated mock fixture CI 자동 + linux/torvalds public repo manual 1회 sanity.
- 신규 코드: `shared/github/trees/fallback.rs` 후보. budget struct + algorithm.

## References

- `docs/specs/spec-github-api.md` § Trees truncation handling
- `docs/ralph/guardrails.md` § G-002
- [source: https://docs.github.com/en/rest/git/trees] (2026-05-10 fact check)
