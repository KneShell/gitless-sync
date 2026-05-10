# ADR 0013: 자율 chain hard cap — depth 3 + token 200k + wall-clock 6h 복합

- **Status**: Accepted
- **Date**: 2026-05-10
- **Related**: `docs/ralph/guardrails.md` § G-019, `docs/research/phase7-vague.md` § B, memory `feedback_release_phase_chain.md`, memory `feedback_quality_vs_complexity.md`

## Context

ralph 가동 → sub-claude clean-context 검증 → finding 발견 → 신규 phase 자동 plan/spec 생성 → ralph 추가 가동 chain 정책 (memory `feedback_release_phase_chain.md`)은 무한 loop 위험. finding이 매 iteration 새 각도로 도출되며 진동 가능 + token / wall-clock 비용 통제 부재 시 "비싼 진동" 발생.

Phase 7 vague (2026-05-10)에서 사용자 결정: 3차원 hard cap 복합 + 수렴 기준 + escape hatch.

## Decision

3차원 hard cap 복합:

| cap | 값 | 측정 |
|---|---|---|
| depth | 3 chain | Phase N → N+1 → N+2 → N+3. 그 너머 BLOCK. |
| token | 200k | 단일 ralph run + sub-claude 검증 + AUTO-FIX 합산. conversation token 카운터. |
| wall-clock | 6h | 첫 ralph launch 시점부터 측정. |

수렴 기준: "동일 finding 2회 연속 + 신규 0건" → CONVERGE PASS, push + tag 진행.

escape hatch: cap 초과 또는 sub-claude finding이 spec semantics 변경 요구 시 → BLOCK + changelog/research에 finding 기록만 + 다음 세션 wake-up 시 사용자 surface (자율 chain 중단).

trace file 자동 생성은 yagni — cap 도달 시점에 사람 surface 시 사후 분석 가능 (memory `feedback_quality_vs_complexity.md` 정합).

## Consequences

- guardrails.md § G-019 신규 추가.
- memory `feedback_release_phase_chain.md` 정합 (ralph 자율 주행 + 도중 wake-up 0 stance).
- cap 변경은 본 ADR 갱신 동반.

## References

- `docs/ralph/guardrails.md` § G-019
- `docs/research/phase7-vague.md` § B
- memory `feedback_release_phase_chain.md`
- memory `feedback_quality_vs_complexity.md`
