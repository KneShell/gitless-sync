# ADR 0007: GraphQL alias batch size — 200 default 유지

- **Status**: Accepted
- **Date**: 2026-05-07
- **Related**: ADR 0005 (rayon은 REST 한정), ADR 0006 (default backend GraphQL), `docs/specs/spec-github-api.md` § GraphQL backend, `docs/roadmap.md` § Phase 4 GraphQL batching, `docs/research/phase4-measurements.md` § P6a

## Context

Phase 4 GraphQL backend는 한 `gh api graphql` request 안에 N개의 `history(first: 1, path: ...)` alias를 묶는다 (ADR 0005). batch size N의 후보는 두 자연수: **100**과 **200**.

- **200**: `roadmap.md` § Phase 4 GraphQL batching 권장 상한. GitHub GraphQL node 한도(500,000 / request)와 점수 기반 rate limit 모델 안에서 single-request 효율 극대화.
- **100**: 좀 더 보수적. secondary rate limit / payload size에 안전 마진. 단 wire round-trip 회수가 200 대비 2배.

P6a (2026-05-07)에서 13 path scale로 batch 100 / 200 비교 측정. raw data + 환경 + 시퀀스 표 + 분석 4점: `docs/research/phase4-measurements.md` § P6a.

요약: 13 paths × 1 chunk (`paths.chunks(N)`이 batch 100/200 모두 1 chunk 생성, 13 ≤ 100 ≤ 200) → 발사 GraphQL request의 alias 개수·body 크기 동일 → batch size 차이가 wire/server 단위 식별 불가 scale. mean 격차 (100 vs 200 1.225~1.854x)는 GraphQL `committedDate` latency 단발 spike (3236 / 6044 / 10115 ms outlier)에서 발생한 measurement noise. 250+ path scale 검증은 v0.1/v0.2 비목표.

## Decision

**`GRAPHQL_BATCH_SIZE = 200` default 유지.**

근거:

- 본 raw data로 batch 100 우위를 주장할 근거 부족 (1 chunk scale에서 functional 동등 + measurement noise 지배).
- yagni + `roadmap.md` § Phase 4 GraphQL batching 권장 상한과 일관 — 권장값 채택이 default. 우위가 입증되지 않은 상태에서 보수 변경은 plan 외.
- 250+ path scale에서 batch 200이 chunk 분할 시 wire round-trip이 더 적음 — 그 시나리오에서 200이 100 대비 손해 볼 수 없는 구조 (점수 기반 rate limit 측면도 200이 동등 또는 우위).

## Consequences

### `crates/gitless-sync/src/commands/scan/graphql.rs`
- `pub(crate) const GRAPHQL_BATCH_SIZE: usize = 200;` 그대로 유지. doc comment에 "ADR 0007 confirmed"로 명시 (P7a 시점에서는 P6a 측정 후 confirm 단계).
- 단위 테스트 `chunks_paths_above_batch_size` (300 → 200+100) 그대로 유지. 결정값 200과 정합.

### `docs/specs/spec-github-api.md` § GraphQL backend
- "default 200" 표현을 "default 200 (ADR 0007 confirmed)"로 갱신. P2 시점의 P6a/P7a 미정 placeholder 정리.
- § batch size 변경 정책: batch size 변경 시 본 § + ADR 0007 동시 갱신. 변경 트리거는 (a) 250+ path scale에서 batch 200 측정 우위 부재 입증, (b) secondary rate limit 점수 모델에서 batch 200이 한도 트리거.

### 향후 재평가 트리거
- secondary rate limit (점수 기반) 발생 시 batch size 하향 + 본 ADR 갱신 (별도 ADR 분기 또는 본 ADR § Status: Superseded).
- vault scale (수백~천 path) 측정에서 batch 100 vs 200 식별 가능한 차이 surface 시 raw data 박고 재결정.
- 현재 plan(P9 dogfooding)은 KneShell/gitless-sync 43 files minimum scale로 cross-backend 정합만 검증 — batch size scale 검증은 비목표.

## References

- ADR 0005 § Decision (GraphQL = alias batching only, request 단위 순차)
- ADR 0006 § Decision (default backend `rest` → `graphql`)
- `docs/specs/spec-github-api.md` § GraphQL backend § Alias batching 패턴 + § batch size 변경 정책
- `docs/roadmap.md` § Phase 4 GraphQL batching (200 권장 상한)
- `docs/research/phase4-measurements.md` § P6a (raw data + 시퀀스 표 + 분석)
