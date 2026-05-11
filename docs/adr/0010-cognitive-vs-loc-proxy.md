# ADR 0010: cognitive complexity + LOC proxy 중복 — 둘 다 유지

- **Status**: Accepted
- **Date**: 2026-05-10
- **Related**: `docs/specs/spec-architecture.md` § LOC 임계 § Panic escape hatch 차단, `clippy.toml`, `Cargo.toml` § `[workspace.lints.clippy]`, `xtask/src/check_line_limits/`

## Context

코드 품질 강화 단계에서 인지부하 측정 proxy 3건을 deny 게이트로 도입.

| 룰 | granularity | 임계값 | enforcement |
|---|---|---|---|
| `clippy::cognitive_complexity` | function | 15 | `Cargo.toml` workspace lint deny + `clippy.toml` threshold |
| `clippy::too_many_lines` | function | 60 | `Cargo.toml` workspace lint deny + `clippy.toml` threshold |
| `xtask check-line-limits` | file | 300 | `cargo xtask check-line-limits` deny (CI + ralph step 3) |

외부 시각으로 "LOC 300 + `cognitive_complexity` 15 부분 중복"이라는 비판이 제기됐다. 본 ADR은 그 재검토.

비판의 무게 — 셋 다 "인지부하"라는 같은 frame으로 측정한다. proxy 중복은 (a) 위반 시 이중 fire로 noise 증가 + (b) 정책 항목 누적 위험. 따라서 여러 proxy 동시 deny는 정당화 필요.

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

### 측정 결과

- 위반 0건 (`cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo xtask check-line-limits` 56+5/56+5 within 300 클린).
- production 코드에 `#[allow(clippy::cognitive_complexity)]` / `#[allow(clippy::too_many_lines)]` override 0건. 셋 다 자연 통과 — proxy가 실제 코드 작성에 마찰 안 줌.

## Consequences

### `docs/specs/spec-architecture.md` § Function-level complexity gates 섹션

3 proxy table (cog_comp 15 / too_many_lines 60 / LOC 300) + 직교성 paragraph + ADR 0010 cross-ref + 면제 카테고리 (test 코드 / doc-heavy file).

### 재평가 트리거

1. cog_comp 단독 fire (LOC + too_many_lines 둘 다 통과인데 cog_comp만 fire) 사례 발생 시 — 직교성 evidence 보강.
2. cog_comp 위반 회피 위해 코드를 인위적으로 흐트러뜨리는 패턴 surface 시 — proxy 압박이 코드 quality 하락 신호.
3. clippy upstream에서 cog_comp 산식 변경 시 (currently `cognitive-complexity` is unstable in some versions) — 계산 방식 안정성 검토.

### 코드 변경

0. proxy 3건 모두 이미 deny + baseline 위반 0건 + override 0건 — 결정은 spec-only 갱신.

## References

- `Cargo.toml:16-23` (workspace lints clippy deny 6건)
- `clippy.toml:1-4` (threshold 60/15/5)
- `xtask/src/check_line_limits/mod.rs` (`DEFAULT_LIMIT = 300`, doc-heavy 50% 면제)
- `docs/specs/spec-architecture.md` § LOC 임계 § Function-level complexity gates
