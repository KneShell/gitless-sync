# ADR 0005: rayon은 REST backend 한정 — GraphQL backend는 alias batching 단독

- **Status**: Accepted
- **Date**: 2026-05-07
- **Related**: ADR 0001 (gh subprocess 채택), ADR 0003 (rayon 유지 결정 — REST backend 단독 시점), `docs/ralph/guardrails.md` G-011, `docs/specs/spec-github-api.md` § 병렬 호출 정책

## Context

ADR 0003은 commits API를 단건 N회 직렬 호출하던 구조에서 rayon 8 concurrent로 묶었을 때 4.86x speedup이 측정됐음을 근거로 rayon 유지를 결정했다. 그러나 그 결정은 **REST backend 단독 시점의 trade-off**였다. Phase 4에서 GraphQL backend(`gh api graphql` + alias batching)가 추가되면 한 request 안에서 default 200개 path의 `history(first: 1, path: ...)` node를 병렬 평가받는다. alias batching 자체가 병렬 효과를 내장하므로, 그 위에 rayon으로 request를 다시 8 concurrent로 풀면 효과가 중복된다.

추가로 GraphQL endpoint는 단일 request 처리 비용이 REST의 단건 commits API 호출과 다르다(secondary rate limit/시간당 point 한도 모델). 같은 path 200개를 8개 thread × 25 alias로 푸는 것보다 1 thread × 200 alias 단일 request가 점수 정책상 유리하고 측정 단순성도 더 높다.

ADR 0003 본문에도 향후 재측정 트리거로 "GraphQL backend 활성화 (Phase 4) 시 단건 N× 호출 패턴 자체가 alias batching으로 대체되므로 rayon 정책 재평가. 별도 ADR 분기 가능성 있음."이라고 명시되어 있었다. 본 ADR이 그 분기다.

## Decision

backend별로 정책을 분기한다.

- **REST backend**: ADR 0003 그대로 rayon 유지. `MAX_COMMITS_CONCURRENCY = 8` 상수는 REST 분기 내부에서만 active.
- **GraphQL backend**: rayon 미사용. alias batching이 자체 병렬 효과(default 200 alias/request)를 내장하므로, request 단위 추가 병렬화 없음. paths는 chunk 단위로 순차 호출 (`paths.chunks(GRAPHQL_BATCH_SIZE)`).

이 분기는 `commands::scan::run_with_client` 안에서 `args.backend` enum match로 처리. REST 분기만 rayon ThreadPool을 install하고, GraphQL 분기는 단순 for/iter로 chunk 처리한다.

## Consequences

### G-011 (rayon 8 concurrent abuse 회피)
- 본문에 "**REST backend 한정 활성**" 명시 추가. GraphQL backend에서는 alias batching이 abuse detection 회피 정책의 본체이므로 G-011 cap 개념 자체가 무관.
- `MAX_COMMITS_CONCURRENCY = 8` 상수는 REST 분기에서만 import/use. GraphQL 모듈에서는 참조 0.

### `spec-github-api.md` § 병렬 호출 정책 (P2에서 갱신)
- backend별 분기 표 추가:
  - REST = rayon 8c (ADR 0003)
  - GraphQL = alias batching only, request 단위는 순차 (ADR 0005)
- `MAX_COMMITS_CONCURRENCY` 상수 활성 범위가 REST에 한정됨을 명시.

### 코드 (P3a, P3b에서 작성)
- `commands/scan/graphql.rs`는 rayon 의존 0. `paths.chunks(GRAPHQL_BATCH_SIZE).flat_map(...)` 단순 for/map.
- `commands/scan/github.rs` (REST)의 `fetch_commit_dates_parallel`은 rayon ThreadPool 패턴 그대로.
- `commands/scan/mod.rs`에서 backend enum match로 분기 — REST 분기만 rayon install.
- `Cargo.toml` `rayon = "1"` 의존성은 그대로 유지 (REST backend 활성 동안 필요).

### 향후 재평가 트리거
- REST backend가 v0.2 이후 deprecated/제거되면 rayon 의존성 자체를 dropped 검토 (ADR 분기 가능성). 현재 시점에서는 `--backend rest`가 explicit fallback으로 유지(ADR 0006).
- GraphQL alias batching이 secondary rate limit을 트리거하면 batch size 하향 + ADR 0007 갱신 (rayon 재도입은 현재 plan 외).

## References

- ADR 0001 § D1 (gh subprocess 채택)
- ADR 0003 § Decision + § "향후 재측정 트리거" (Phase 4 GraphQL 분기 예고)
- `docs/ralph/guardrails.md` § G-011
- `docs/specs/spec-github-api.md` § 병렬 호출 정책 (P2 갱신 대상)
- `docs/ralph/implementation-plan.md` § Phase 4 사전 결정 §7 (rayon 처분)
