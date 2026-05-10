# ADR 0008: mtime cache 제거

- **Status**: Accepted
- **Date**: 2026-05-07
- **Related**: ADR 0009 (internal cache read-only 예외 — 본 ADR로 obsolete), `docs/research/phase4-measurements.md` § P6c, `docs/specs/spec-config.md` § Cache (Phase 4) (제거 대상), `crates/gitless-sync/src/shared/cache.rs` (제거 대상)

## Context

Phase 4 P4 task에서 `mtime` 기반 SHA cache를 도입했다. 동기는 1차/2차 scan 사이의 hash phase 단축으로 wall-clock 단축. P6c task에서 50 path scale로 cold/warm 비교 측정.

§ Phase 4 사전 결정 §15 임계값 (P7 결정 자의성 회피, P6 raw data 매핑):
- **유지**: cache hit speedup ≥ 2x.
- **제거**: speedup < 1.5x (코드/의존성 부담만 — `dirs` crate 1 + cache.rs ~360 LOC).
- **경계 1.5~2.0x**: 본 ADR에서 raw data 박고 yagni 일관 시 제거 default.

raw data + 환경 + cache 정상 채워짐 확인 + dominant cost 분석: `docs/research/phase4-measurements.md` § P6c.

요약: speedup **1.040x (N=3)** / **0.988x (N=5)** — 두 측정 모두 wall-clock variance 안. dominant cost는 cargo fork + `gh api` subprocess × 2 + Trees/GraphQL 응답 처리이며 hash phase는 ~50ms (전체 1300ms 대비 3-4%). cache lookup gain ≈ save cost.

## Decision

measured speedup **1.040x / 0.988x** → **둘 다 < 1.5x 제거 영역** (경계도 아님).

→ **mtime cache를 제거한다.**

cache 코드는 다음 모두 삭제:
1. `crates/gitless-sync/src/shared/cache.rs` (전체 ~360 LOC)
2. `crates/gitless-sync/src/shared/mod.rs` `pub mod cache;` 한 줄
3. `crates/gitless-sync/src/commands/scan/mod.rs` cache load/save 진입점 + `build_pre_entries` cache lookup/insert 분기 (인자 단순화)
4. `crates/gitless-sync/Cargo.toml` `dirs = "5"` (production + dev-dep)
5. `Cargo.lock` 갱신 (cargo build로 자동)
6. `docs/specs/spec-config.md` § Cache (Phase 4) 본문 + Acceptance Criteria § Cache (Phase 4) 통째 삭제

ADR 0009 (internal cache read-only 예외)는 본 ADR로 **obsolete** 마크. cache 본체가 사라지므로 read-only 본성에 대한 명확화 줄이 더 이상 active rule 아님.

통합 테스트 시나리오 22/23/25 (cache 매트릭스)는 제거. cache 관련 helper(`cache_file_for` / `sanitize_component_local` / `cleanup_cache_for`)도 동반 삭제. 시나리오 20/21/24는 GraphQL backend 본체 검증이라 그대로.

## Consequences

### 사용자 contract 영향

- 0. cache는 internal metadata였고 ScanReport JSON identical 보장. 호출자 영향 0.
- cache 파일이 OS user-cache에 남아 있어도 무해 — 도구가 더 이상 읽지 않음. orphan 파일 삭제는 사용자 운영 (수동).

### 코드 부담 제거

- `dirs` crate 의존성 1 제거 (production + dev-dep).
- `cache.rs` ~360 LOC 삭제.
- `scan/mod.rs` cache 진입점 (load / save) + `build_pre_entries` lookup/insert 분기 제거 → `assemble_entries` 시그니처 단순화 (`cache: &mut Cache` 인자 삭제).
- 통합 테스트 helper(cache_file_for / sanitize_component_local / cleanup_cache_for) + 시나리오 22/23/25 제거.

### Phase 5 영향

- 1000+ path scale에서 hash 비중이 늘어 cache speedup이 커질 가능성 — vault scale 측정에서 다시 검토. yagni 기조라면 그때 다시 평가.
- 본 ADR 0008은 v0.2 baseline에서의 결정. v0.3+에서 측정 환경 바뀌면 재고 가능.

### ADR 0009 obsolete

- ADR 0009 본문에 "**2026-05-07 obsolete by ADR 0008**: cache 효과 미달로 제거 결정." 한 줄 추가.
- ADR 0009의 "Read-only는 user 데이터·원격 보존이 본질. Internal cache는 예외" 명확화 줄도 obsolete 대상 — `CLAUDE.md` § Critical Rules § 도구 본성에서 해당 한 줄 제거.

### `CLAUDE.md` 갱신

- § Current State에 ADR 0007 + ADR 0008 결정 박스 추가.
- § Critical Rules § 도구 본성에서 "Internal cache는 예외" 한 줄 제거 (ADR 0009 obsolete cascade).

## Phase 7.3 재검토 (2026-05-10, vault scale)

§ Consequences § Phase 5 영향에서 예고한 1000+ path scale 재검토를 Phase 7.3 task V에서 수행. 결론 — **keep-drop 유지** (cache 재도입 안 함).

### Raw data 비교

| 측정 | path scale | mean walltime | hash phase 측정 | 비고 |
|---|---:|---:|---|---|
| P6c (`docs/research/phase4-measurements.md`) | 50 | 1324.8 ms (cold N=3) | ~50 ms (전체 ~3-4%) | 단발 hash 시간 명시 |
| T (`docs/research/phase7-vault-scale-bench.md`) | 1000 | 829 ms (mean N=3) | (instrumentation 부재) | 전체 walltime만 |
| U (동 file § public repo cross-check) | 1000 + 4964 remote | 1109 ms (single run) | (동) | 동 |

### 분석

- **path scale 20× 증가 (50 → 1000)에도 walltime은 오히려 작거나 비슷** (1324.8 ms → 829 ms / 1109 ms). hash 비중이 path 수에 linear 폭증한다면 walltime도 동반 폭증해야 하나 신호 없음. rayon 8c 병렬 (ADR 0003) + 작은 remote (T: 129 entries) 영향이 hash 부담을 흡수했다고 추정 가능.
- **단** T 측정은 hash phase 별도 instrumentation 부재 (`phase7-vault-scale-bench.md` § Caveats § "Internal instrumentation 부재" 명시) — hash 비중 정량화 불가. 즉 P6c 3-4%가 1000 scale에서 정확히 몇 %로 변했는지는 verify 안 됨.
- **keep-drop trigger 임계는 § Decision의 "speedup ≥ 2x"** — 본 재검토 시점 측정 데이터로는 cache 도입 시 speedup 추정조차 불가. 임계 미달 신호도, 임계 도달 신호도 없음 → yagni 일관 적용.

### 결정

**keep-drop 유지** + § 7-2 task V acceptance "측정 결과 surface 안 하면 task skip 표시" 트리거 충족 (hash phase instrumentation 부재). 재도입 정당성 surface 시점은 별도 instrumentation work + measurement task 도입 시점 — Phase 7 scope 외 (yagni).

향후 재검토 trigger:
- (a) hash phase 별도 instrumentation 도입 후 측정 결과 hash 비중 ≥ 30% surface,
- (b) 또는 cache 도입 시 measured speedup ≥ 2x 직접 surface.

둘 중 하나 surface까지 본 ADR 0008 keep-drop 결정 유효.

## References

- `docs/research/phase4-measurements.md` § P6c (raw data + 분석 + 임계값 매핑)
- `docs/research/phase7-vault-scale-bench.md` § scan walltime + § public repo cross-check (Phase 7.3 vault scale 측정, T+U raw data)
- `docs/adr/0009-internal-cache-readonly-exception.md` (본 ADR로 obsolete).
- `docs/specs/spec-config.md` § Cache (Phase 4) (제거 대상).
- `crates/gitless-sync/src/shared/cache.rs` (제거 대상).
