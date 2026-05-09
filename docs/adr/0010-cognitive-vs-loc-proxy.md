# ADR 0010: cognitive complexity + LOC proxy 중복 — 둘 다 유지

- **Status**: Accepted
- **Date**: 2026-05-10
- **Related**: `docs/specs/spec-architecture.md` § LOC 임계 § Panic escape hatch 차단, `docs/roadmap.md` § Phase 6 Step 1 / § clean-context 외부 시각 보강 §5-3, `clippy.toml`, `Cargo.toml` § `[workspace.lints.clippy]`, `xtask/src/check_line_limits/`, `docs/ralph/implementation-plan.md` § Phase 6.1 task UU

## Context

Phase 6 (코드 품질 강화)에서 인지부하 측정 proxy 3건을 deny 게이트로 박았다.

| 룰 | granularity | 임계값 | enforcement |
|---|---|---|---|
| `clippy::cognitive_complexity` | function | 15 | `Cargo.toml` workspace lint deny + `clippy.toml` threshold |
| `clippy::too_many_lines` | function | 60 | `Cargo.toml` workspace lint deny + `clippy.toml` threshold |
| `xtask check-line-limits` | file | 300 | `cargo xtask check-line-limits` deny (CI + ralph step 3) |

clean-context 외부 시각 §5-3이 "LOC 300 + `cognitive_complexity` 15 부분 중복"이라 비판하며 박제 expiration 정책에 따라 Phase 7 진입 시 재검토를 권고했다 (`docs/roadmap.md:126`). 본 ADR은 그 재검토.

비판의 무게 — 셋 다 "인지부하"라는 같은 frame으로 측정한다. proxy 중복은 (a) 위반 시 이중 fire로 noise 증가 + (b) 박제 항목 누적 — 박제 expiration 정책 자체가 "transitive constraint 누적 차단" (`docs/specs/spec-architecture.md` § 박제 expiration) 의도라 여러 proxy 동시 deny를 의심해야 한다.

## Decision

**셋 다 유지 (option a).** 같은 frame에서 측정하지만 서로 다른 escape hatch를 차단한다 — 한 쪽 제거 시 다른 쪽이 못 잡는 패턴이 통과된다.

### 직교성 — 각 proxy가 독립적으로 잡는 패턴

| 패턴 | cog_comp 15 | too_many_lines 60 | LOC 300 |
|---|---|---|---|
| 50-line 함수 + 20 nested branch | fire | pass | pass |
| 80-line 함수 + flat 분기 X | pass | fire | pass |
| 350-line file + 10개 30-line simple 함수 | pass | pass | fire |

cog_comp는 함수 단위 branching 깊이만 측정 — 짧지만 dense한 함수 catch. too_many_lines는 함수 단위 LOC 측정 — 길지만 flat한 함수 catch. LOC 300은 file 단위 누적 측정 — 잘 분리된 짧은 함수가 모여 file이 sprawling해지는 패턴 catch.

한 쪽 제거 시 빠지는 케이스:
- cog_comp 제거 → 60 LOC 안에 분기 20개인 dense parser 통과
- too_many_lines 제거 → 100 LOC flat helper 통과
- LOC 300 제거 → 1000 LOC file에 simple 함수 30개 통과

### 비판의 부분 인정

같은 frame (인지부하) 측정이라는 점은 사실. proxy 중복 (overlap)이 일부 존재 — 가령 200 LOC file 안에 200-line 함수 1개라면 LOC 300은 통과하지만 too_many_lines + cog_comp 둘 다 fire. 그러나 overlap은 redundancy가 아닌 reinforcement — 같은 패턴을 두 각도에서 잡는 것은 조기 detection이지 noise 아님 (clippy 룰은 함수 정의 시점 fire, xtask는 CI 시점 fire라 rapid feedback 차이도 있음).

### 박제 자료 (현 시점)

- 위반 0건 (`cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo xtask check-line-limits` 56+5/56+5 within 300 클린).
- production 코드에 `#[allow(clippy::cognitive_complexity)]` / `#[allow(clippy::too_many_lines)]` override 0건. 셋 다 자연 통과 — proxy가 실제 코드 작성에 마찰 안 줌 (test 코드의 `#[cfg_attr(test, allow(clippy::unwrap_used, ...))]`은 별 lint).
- 6 file이 290-299 LOC 범위 (`shared/normalize.rs` 299 / `commands/scan/pipeline/short_circuit.rs` 299 / `commands/scan/walker.rs` 298 / `commands/scan/mod.rs` 295 / `commands/scan/pipeline/hash_pass.rs` 294 / `commands/scan/pipeline/finalize.rs` 292) — LOC 300이 binding constraint.
- LOC 300 단독 fire 사례 — task AA (pipeline.rs 295→over-budget split into 5 sub-modules), task GG/HH carryover (short_circuit.rs 298→302 fmt drift). 함수 단위 분기·길이 정상이라 cog_comp / too_many_lines 동시 fire 안 함 — file 단위 sprawl 회피 차원에서만 fire.
- cog_comp + too_many_lines 동시 fire 사례 — task R 통합 test 단일 모놀리식 함수 (cognitive 25/15 + too_many_lines 68/60 둘 다 fire) → 4 test split + helper 추출로 해소. 함수 단위 game.

## Consequences

### `docs/specs/spec-architecture.md` § Function-level complexity gates 신규 섹션 (본 task에서 갱신)

기존 § LOC 임계 (file-level)와 § Panic escape hatch 차단 사이에 § Function-level complexity gates 섹션 추가:
- 3 proxy table (cog_comp 15 / too_many_lines 60 / LOC 300)
- 직교성 1 paragraph + ADR 0010 cross-ref
- 면제 카테고리 — test 코드는 `#[cfg_attr(test, allow(...))]` 자연 면제. doc-heavy file은 LOC 300 자연 면제 (`xtask` 50% 룰).

### `CLAUDE.md`

미변경. § 사용자 취향 결정 (박제)이 이미 `docs/specs/spec-architecture.md` pointer라 ADR 0010 결정이 spec 갱신으로 cascade.

### 박제 expiration 재평가 트리거

본 ADR의 박제 expiration: **Phase 7+ 진입 시 재검토**. 구체 트리거:
1. cog_comp 단독 fire (LOC + too_many_lines 둘 다 통과인데 cog_comp만 fire) 사례 발생 시 — 직교성 evidence 보강.
2. cog_comp 위반 회피 위해 코드를 인위적으로 흐트러뜨리는 패턴 surface 시 — proxy 압박이 코드 quality 하락 신호. 그 시점 재검토.
3. clippy upstream에서 cog_comp 산식 변경 시 (currently `cognitive-complexity` is unstable in some versions) — 계산 방식 안정성 검토.

### 코드 변경

0. proxy 3건 모두 이미 deny + baseline 위반 0건 + override 0건 — 결정은 spec-only 박제 갱신.

## References

- `Cargo.toml:16-23` (workspace lints clippy deny 6건)
- `clippy.toml:1-4` (threshold 60/15/5)
- `xtask/src/check_line_limits/mod.rs` (`DEFAULT_LIMIT = 300`, doc-heavy 50% 면제)
- `docs/specs/spec-architecture.md` § LOC 임계 § 박제 expiration
- `docs/roadmap.md` § Phase 6 Step 1 (clippy 강화 결정 trail) + §5-3 (clean-context 비판)
- `docs/ralph/implementation-plan.md` § task R / § task AA / § task GG / § task HH (proxy fire 사례 trail)
