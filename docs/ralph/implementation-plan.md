# Implementation Plan

## Status
- Last updated: 2026-05-10 (Phase 7.4 task Z — Phase 7 plan finalize: Active Phase § Phase 7 detailed task list (A~JJ, 36 task) → Completed Phases § Phase 7 (2026-05-10) 1~2 sentence 요약 이전. Phase 7 종결 + v0.3.0 release tag 박힌 상태. 다음 phase 진입 대기. Pending Phases (v0.4+)는 sub-claude clean-context finding 발생 시 자동 신규 phase plan/spec 정책 보존 (memory feedback_release_phase_chain.md 정합).)
- Total tasks: 96
- Completed: 96 / 96

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 처리. 의존 순서가 본 plan에 명시 안 됐으면 acceptance + spec 본문에 잠재 의존 명시 (e.g., "X task 결과 위에서 진행").
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Phase 6 hard gate 모두 deny active 유지 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출). 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (project-ops.md). 신규 task의 acceptance에 unit test 포함.

> **Slim 정책 (2026-05-10)**: completed phase는 1~2 sentence 요약만 retain. 자세한 task별 결과는 git history (`git log --grep="<task ID>"`) + commit message 본문 + CHANGELOG.md user-facing summary로 cover. active/pending phase만 verbose retain. 의존 순서 graph 제거 — completed phase는 이미 종결, 신규 phase는 phase 본문 안에 의존 명시.

## Completed Phases

### Phase 5 (2026-05-09 ~ 05-10, 57 task: A~TT)
도메인 함정 정리 본진 + plumbing follow-up + sibling cleanup + clean-context audit follow-up + md 자료 audit. 8 핵심 함정 + 추가 함정 4건 detect/handle + Schema v1.0→1.1 (mode/failed_reason 9 enum/lfs_pointer) + vault dogfood 117 files / 0 drift / 0 failed + CLAUDE.md privacy section 제거.

### Phase 6 (2026-05-09 ~ 05-10, 23 task: A~T 본진 + UU/VV/WW v0.2.x cleanup)
Code Quality Strengthening 본진 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출 hard gate) + v0.2.x cleanup (ADR 0010 cognitive_complexity vs LOC orthogonal proxy + CI runner Linux 전환 G-018).

### Phase 7 (2026-05-10, 36 task: A~JJ 본진 + clean-context audit chain, v0.3.0)
vault scale + Trees sub-tree fallback + 큰 파일 임계치 + clean-context audit chain (depth 3/3) + v0.3.0 release. Trees truncation sub-tree 재귀 (call budget 1000 / entries 500_000 cap) + file_too_large/memory_exceeded reason 2건 (100MB/50MB 임계, schema v1.1→1.2 + size_bytes field) + 합성 vault generator (xtask synth-vault, 1000+ dogfood walltime 829/1109 ms) + Phase 7.5/7.6/7.7 stale ref bulk fix (deterministic grep 0 hit CONVERGE PASS, G-019 chain depth 3/3 cap 미도달) + v0.3.0 annotated tag (main + tag push, v0.2.1 스타일 mirror).

## Active Phase

(Phase 7 종료. 다음 phase는 sub-claude clean-context finding 또는 사용자 신규 요청 진입 시 정의.)

## Pending Phases (v0.4+)

(Phase 7 종료 후 sub-claude clean-context finding 발생 시 자동 신규 phase plan/spec — memory `feedback_release_phase_chain.md` 정합)

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner (Phase 6.1 WW).
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **박제 expiration** — Phase 진입마다 재검토 (CLAUDE.md § 박제 정책).
- **자율 진행 회피 영역** (사용자 vague 답변, Phase 5.13.1/5.14 패턴) — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 vague + clean-context 외부 시각 보강 필수.
