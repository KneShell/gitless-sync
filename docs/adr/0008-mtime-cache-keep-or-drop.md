# ADR 0008: mtime cache 제거

- **Status**: Accepted
- **Date**: 2026-05-07
- **Related**: ADR 0009 (internal cache read-only 예외 — 본 ADR로 obsolete), `docs/ralph/implementation-plan.md` § Phase 4 사전 결정 §15 임계값, P6c Raw data, `docs/specs/spec-config.md` § Cache (Phase 4) (제거 대상), `crates/gitless-sync/src/shared/cache.rs` (제거 대상)

## Context

Phase 4 P4 task에서 `mtime` 기반 SHA cache를 도입했다. 동기는 1차/2차 scan 사이의 hash phase 단축으로 wall-clock 단축. P6c task에서 이 효과를 측정해 § Phase 4 사전 결정 §15 임계값에 매핑하고 본 ADR에서 유지/제거 결정.

§ Phase 4 사전 결정 §15 임계값 (P7 결정 자의성 회피, P6 raw data 매핑):
- **유지**: cache hit speedup ≥ 2x.
- **제거**: speedup < 1.5x (코드/의존성 부담만 — `dirs` crate 1 + cache.rs ~360 LOC).
- **경계 1.5~2.0x**: 본 ADR에서 raw data 박고 yagni 일관 시 제거 default.

## P6c Measurement Summary

(50 path scale, KneShell/gitless-sync, Windows 11 Pro 10.0.26100 / gh 2.88.1 / cargo 1.95.0 / release binary `target/release/gitless-sync.exe`. wall-clock = PowerShell `Measure-Command`.)

- **N=3 sequence**: cold mean **1324.8 ms** / warm mean **1274.0 ms** → speedup **1.040x** (variance 6.7%/3.7%, <30%).
- **N=5 sequence (변동 재확인)**: cold mean **1335.0 ms** / warm mean **1351.6 ms** → speedup **0.988x** — warm이 cold보다 살짝 느림. variance 8.6%/9.4%, <30%.

cache 정상 채워짐 확인:
- 1차 scan 후 cache 파일 size 9063 bytes. JSON entries 50 (= summary `identical 30 + local_only_changed 20`). 모든 local file이 cache에 박힘 → 2차 scan에서 100% cache hit 기대.
- cache `version` field = 1 (CACHE_VERSION 일관). 모든 entry는 `mtime` (UTC ISO-8601) + `sha` (hex) + `is_binary` (bool) — 형식 spec-config.md § cache 일관.

분석:
- 두 측정에서 speedup이 0.99 ~ 1.04 범위. variance ≈ 4~9%로 낮은데도 1차/2차 mean 차이가 변동 범위 안. **cache 효과가 wall-clock 측정 variance보다 작음**.
- dominant cost가 hash가 아닌 다른 곳: 1300ms 안에서 hash 50 file은 ~50ms (1KB-10KB 텍스트 × 50). 나머지 ~1250ms는 (i) cargo binary fork, (ii) `gh api` subprocess fork × 2 (Trees + GraphQL), (iii) Trees API 응답 다운로드 + 파싱, (iv) walker 파일 시스템 walk, (v) GraphQL 응답 파싱 + JSON 직렬화. cache는 hash phase만 단축 → 전체 대비 ~3-4% 영향. measured 결과(±5% noise) 내부.
- cache save는 cold/warm 모두 매 호출 발생 (`commands/scan/mod.rs::build_report` end). 9KB JSON serialize + tmp write + rename atomic 비용은 cold/warm 동일하게 발생 → cache 도입에 따른 *추가 비용*만 양쪽에 박힘. lookup 효과는 그 위에 누적되는데 net zero.

## Decision

§ Phase 4 사전 결정 §15 임계값 매핑:
- measured speedup **1.040x (N=3) / 0.988x (N=5)** → **둘 다 < 1.5x 제거 영역**. 경계도 아님 (1.5x에 한참 못 미침).

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

- 1000+ path scale에서 hash 비중이 늘어 cache speedup이 커질 가능성 — vault scale 측정에서 다시 검토. yagni 기조라면 그때 다시 박음.
- 본 ADR 0008은 v0.2 baseline에서의 결정. v0.3+에서 측정 환경 바뀌면 재고 가능.

### ADR 0009 obsolete

- ADR 0009 본문에 "**2026-05-07 obsolete by ADR 0008**: cache 효과 미달로 제거 결정." 한 줄 추가.
- ADR 0009의 "Read-only는 user 데이터·원격 보존이 본질. Internal cache는 예외" 명확화 줄도 obsolete 대상 — `CLAUDE.md` § Critical Rules § 도구 본성에서 해당 한 줄 제거.

### `CLAUDE.md` 갱신

- § Current State에 ADR 0007 + ADR 0008 결정 박스 추가.
- § Critical Rules § 도구 본성에서 "Internal cache는 예외" 한 줄 제거 (ADR 0009 obsolete cascade).

## References

- P6c raw data: `docs/ralph/implementation-plan.md` § P6c Raw data (2026-05-07).
- § Phase 4 사전 결정 §15 임계값.
- `docs/adr/0009-internal-cache-readonly-exception.md` (본 ADR로 obsolete).
- `docs/specs/spec-config.md` § Cache (Phase 4) (제거 대상).
- `crates/gitless-sync/src/shared/cache.rs` (제거 대상).
