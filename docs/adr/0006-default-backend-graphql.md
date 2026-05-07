# ADR 0006: default backend `rest` → `graphql` 전환

- **Status**: Accepted
- **Date**: 2026-05-07
- **Related**: ADR 0001 (gh subprocess 채택), ADR 0003 (rayon 유지), ADR 0005 (rayon은 REST 한정), `docs/specs/spec-cli-interface.md` § Backend 분기, `docs/ralph/implementation-plan.md` § Phase 4 사전 결정

## Context

v0.1 + v0.2 시점의 default backend는 `--backend rest`였다. commits API를 path별 단건 호출 + rayon 8 concurrent로 묶은 구조다. M5a 측정(ADR 0003)에서 13 path 1.35초 / sequential 6.56초 / 4.86x speedup이 입증돼 v0.2까지 운영됐다.

그러나 vault scale(수백~천 단위 path)에서 단건 호출은 abuse detection 위험과 절대 시간 한계를 동시에 가진다. roadmap "확정" 카테고리에 박힌 GraphQL alias batching은 한 request에 default 200 path의 `history(first:1)`을 묶는다. 이론·사전 추정으로 1000 path 기준 REST 25초 → GraphQL 수 초(~5x speedup, P6에서 raw data로 확정). 측정 결과가 임계값 미만이면 ADR 0007/0008에서 재조정.

LLM 친화성 관점에서 호출자(Claude Code)는 ScanReport JSON을 받는 입장이라 backend 차이는 결과적으로 0. backend 전환은 호출자 코드 변경 0 + 호출자 인지 부담 0이다. 그렇다면 더 빠른 backend가 default가 되는 게 합리적이다.

REST backend를 통째 제거하지 않는 이유는 두 가지다. (a) v0.1/v0.2에서 검증된 자산이라 GraphQL 운영 이슈(rate limit, alias batching 응답 정합성, partial errors 등) 발생 시 즉시 fallback 가능. (b) backend 분기 자체가 코드 모듈성으로 가치 있음 — 새로운 백엔드 도입 시 패턴 유지.

## Decision

`gitless-sync scan`의 `--backend` flag default를 `rest` → `graphql`로 전환한다.

- `--backend graphql` (default, 명시 불필요)
- `--backend rest` (explicit fallback, 운영 이슈 시 수동 지정)

clap default 변경은 P3b에서 `#[arg(default_value_t = Backend::Graphql)]`로 박음. v0.1 stub error("GraphQL backend not implemented")는 P3a에서 본체 박힌 후 P3b에서 제거.

REST backend는 deprecated가 아니라 explicit fallback으로 유지. 별도 deprecation 정책 없음. 향후 GraphQL이 1년 이상 안정 운영 + sufficient telemetry 시 ADR 분기 가능성.

## Consequences

### `spec-cli-interface.md` § Backend 분기 (P2에서 갱신)
- default 표현을 `rest` → `graphql`로 갱신.
- v0.1 stub 표현 ("`--backend graphql`: 즉시 exit 1 + GitlessError::NotImplemented") 제거.
- explicit fallback으로 REST 유지 명시 + ADR 0006 cross-ref.

### `main.rs` clap (P3b에서 갱신)
- `Backend` enum default value `Rest` → `Graphql`. `#[arg(default_value_t = Backend::Graphql)]`.
- v0.1 stub error("GraphQL backend not implemented...") 제거.

### LLM 호출자 (Claude Code 등)
- 마찰 0. `--backend graphql`을 명시할 필요 없어지므로 명령 한 줄 더 짧아짐. 결과 ScanReport 동일.
- 운영 이슈 발생 시 `--backend rest` 추가 한 줄로 fallback. 인자 인터페이스는 v0.1과 그대로 호환.

### 측정 / 박제 (P6, P7a)
- P6b에서 REST vs GraphQL baseline 측정. raw data로 본 ADR 정당화 강화 + ADR 0007 batch size 박제용.
- 측정 결과가 ~5x 추정과 ±50% 차이 시 default 전환 자체 재평가 가능. 그러나 raw data가 박힌 ADR 0006은 본 ADR로 정합 유지 (재결정 시 신규 ADR 분기).

### 운영 / 관찰
- secondary rate limit / abuse detection 패턴 변화 모니터링. GraphQL은 점수 기반 rate limit이라 REST와 다른 노이즈 패턴 가능. P9 dogfooding + 사용자 vault 운영에서 surface.
- 로컬 cache 효과(P4) + GraphQL speedup 효과는 직교. cache 임계값 미달 시 ADR 0008에서 cache 제거해도 본 ADR 영향 0.

## References

- ADR 0001 (`docs/adr/0001-gh-subprocess-and-drop-push-tool.md`) § D1 (gh subprocess 채택)
- ADR 0003 (rayon 유지 — REST default 시점 측정)
- ADR 0005 (rayon은 REST 한정)
- `docs/specs/spec-cli-interface.md` § Backend 분기 (P2 갱신 대상)
- `docs/ralph/implementation-plan.md` § Phase 4 사전 결정 §2 (Backend default), §13 (Performance baseline)
