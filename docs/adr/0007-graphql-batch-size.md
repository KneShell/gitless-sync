# ADR 0007: GraphQL alias batch size — 200 default 유지

- **Status**: Accepted
- **Date**: 2026-05-07
- **Related**: ADR 0005 (rayon은 REST 한정), ADR 0006 (default backend GraphQL), `docs/specs/spec-github-api.md` § GraphQL backend, `docs/roadmap.md` § Phase 4 GraphQL batching, `docs/ralph/implementation-plan.md` § P6a raw data

## Context

Phase 4 GraphQL backend는 한 `gh api graphql` request 안에 N개의 `history(first: 1, path: ...)` alias를 묶는다 (ADR 0005). batch size N의 후보는 두 자연수: **100**과 **200**.

- **200**: `roadmap.md` § Phase 4 GraphQL batching 권장 상한. GitHub GraphQL node 한도(500,000 / request)와 점수 기반 rate limit 모델 안에서 single-request 효율 극대화.
- **100**: 좀 더 보수적. secondary rate limit / payload size에 안전 마진. 단 wire round-trip 회수가 200 대비 2배.

이 trade-off를 P6a에서 raw data로 측정해 결정하기로 plan에 박혀 있었다 (`docs/ralph/implementation-plan.md` § Phase 4 사전 결정 §4 + §13).

## Measurement (P6a raw data, 2026-05-07)

- **환경**: Windows 11 Pro 10.0.26100 / gh 2.88.1 (KneShell account active) / cargo 1.95.0 / release binary `target/release/gitless-sync.exe`. wall-clock = PowerShell `Measure-Command`. `gh auth status` exit 0 확인.
- **대상**: `KneShell/gitless-sync` @ main, local = `D:\00.Projects\02.Personal\05.gitless-sync`. 측정 직전 13개 commited `.md` 파일에 trailing newline 임시 추가 → 13 path가 `local_sha != remote_sha` 분기로 commits map fetch (`commands/scan/mod.rs::fetch_commit_map`). 측정 종료 후 `git restore` 복원, 코드 임시 변경(`GRAPHQL_BATCH_SIZE = 100`)도 revert.
- **명령어**: `gitless-sync.exe scan --repo KneShell/gitless-sync --branch main --local <local> --summary-only`. cache 매 시퀀스 시작 시 cold start.

| 측정 시퀀스 | N | warm-up dropped (ms) | mean (ms) | min/max (ms) | (max-min)/mean |
|---|---|---|---|---|---|
| (a) batch 200 (default) | 5 | 1821.8 | **2076.7** | 1556.9 / 3236.4 | 80.9% |
| (b) batch 100 (임시 변경) | 3 | 1768.3 | **1694.7** | 1651.4 / 1755.9 | 6.2% |
| (c) batch 200 재측정 | 3 | 1731.9 | **3142.2** | 1567.5 / 6044.3 | 142.5% |

raw N개 측정값(ms)은 implementation-plan.md § P6a Raw data 본문에 박힘.

## Analysis

1. **13 paths × 1 chunk**: batch 100과 batch 200 모두 `paths.chunks(N)` 호출에서 **1개 chunk 생성** (13 ≤ 100 ≤ 200). 즉 발사되는 GraphQL request의 alias 개수·body 크기 동일. 코드 분기는 chunk loop 한 번만 돌고 종료. **batch size 차이가 wire/server 단위에서 식별 불가능한 scale.**
2. **GraphQL `committedDate` latency 자연 변동이 지배적**: batch 200 두 시퀀스(a/c) 모두 high outlier(3236.4 ms / 6044.3 ms)에 의해 mean 왜곡. batch 100 시퀀스는 outlier 0회. 동일 코드 경로(1 chunk)인데 분포가 갈리는 건 GitHub server-side latency 단발 spike가 지배적이라는 신호. P6b에서도 GraphQL g2 단발 spike 10115ms가 동일 패턴으로 관측됨.
3. **단순 mean 비교의 함정**: batch 100이 batch 200 (a) 대비 1.225x 빠른 모습이지만, batch 200 (c)와 비교하면 1.854x. 이 격차는 코드 인자가 아니라 N=3~5 표본의 운에 가까움. 동일 코드 경로(1 chunk 발사)에서 1.85x 차이가 나오는 건 measurement noise.
4. **250+ path scale 검증 부재**: 본 측정 환경(13 path)에서는 batch 100 vs 200의 차이를 분리할 수 없다. chunk 분할이 실제로 발생하는 250+ path scale은 KneShell/gitless-sync 외 repo 또는 synthetic 시나리오가 필요하나 v0.1/v0.2 비목표 (vault scale 측정은 사용자 환경 의존).

## Decision

**`GRAPHQL_BATCH_SIZE = 200` default 유지.**

근거:

- 본 raw data로 batch 100 우위를 주장할 근거 부족 (1 chunk scale에서 functional 동등 + measurement noise 지배).
- yagni + `roadmap.md` § Phase 4 GraphQL batching 권장 상한과 일관 — 권장값 채택이 default. 우위가 입증되지 않은 상태에서 보수 변경은 plan 외.
- 250+ path scale에서 batch 200이 chunk 분할 시 wire round-trip이 더 적음 — 그 시나리오에서 200이 100 대비 손해 볼 수 없는 구조 (점수 기반 rate limit 측면도 200이 동등 또는 우위).

## Consequences

### `crates/gitless-sync/src/commands/scan/graphql.rs`
- `pub(crate) const GRAPHQL_BATCH_SIZE: usize = 200;` 그대로 유지. doc comment에 "ADR 0007 confirmed"로 박음 (P7a 시점에서는 P6a 측정 후 confirm 단계).
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
- `docs/ralph/implementation-plan.md` § Phase 4 사전 결정 §4 (default 200), §13 (Performance baseline 패턴), § P6a Raw data (본 ADR 결정 입력)
