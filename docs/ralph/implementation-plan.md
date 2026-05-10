# Implementation Plan

## Status
- Phase 8 종료 (2026-05-10)
- Tasks: 31 (Phase 8)
- Completed: 31 / 31

## Notes for Build Mode
- ralph build mode는 첫 미완료 task (`[ ]`)부터 처리. 의존 순서가 본 plan에 명시 안 됐으면 acceptance + spec 본문에 잠재 의존 명시 (e.g., "X task 결과 위에서 진행").
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Phase 6 hard gate 모두 deny active 유지 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출). 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (project-ops.md). 신규 task의 acceptance에 unit test 포함.

## Completed Phases

### Phase 5 (2026-05-09 ~ 05-10, 57 task: A~TT)
도메인 함정 정리 본진 + plumbing follow-up + sibling cleanup + clean-context audit follow-up + md 자료 audit. 8 핵심 함정 + 추가 함정 4건 detect/handle + Schema v1.0→1.1 (mode/failed_reason 9 enum/lfs_pointer) + vault dogfood 117 files / 0 drift / 0 failed + CLAUDE.md privacy section 제거.

### Phase 6 (2026-05-09 ~ 05-10, 23 task: A~T 본진 + UU/VV/WW v0.2.x cleanup)
Code Quality Strengthening 본진 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출 hard gate) + v0.2.x cleanup (ADR 0010 cognitive_complexity vs LOC orthogonal proxy + CI runner Linux 전환 G-018).

### Phase 7 (2026-05-10, 36 task: A~JJ 본진 + clean-context audit chain, v0.3.0)
vault scale + Trees sub-tree fallback + 큰 파일 임계치 + clean-context audit chain (depth 3/3) + v0.3.0 release. Trees truncation sub-tree 재귀 (call budget 1000 / entries 500_000 cap) + file_too_large/memory_exceeded reason 2건 (100MB/50MB 임계, schema v1.1→1.2 + size_bytes field) + 합성 vault generator (xtask synth-vault, 1000+ dogfood walltime 829/1109 ms) + Phase 7.5/7.6/7.7 stale ref bulk fix (deterministic grep 0 hit CONVERGE PASS, G-019 chain depth 3/3 cap 미도달) + v0.3.0 annotated tag (main + tag push, v0.2.1 스타일 mirror).

### Phase 8 (2026-05-10, 31 task: A~DD, v0.4.0)
LLM-as-Caller Usability eval 7 friction (P0~P3) 해소 + v0.4.0 release. F1/F2 (`diff_meaningful: Option<bool>` + `presence: enum local_only/both/remote_only` 2 field, schema v1.2 → v1.3 minor bump + ADR 0014 'scan-diff metadata contract' 신규) + F3 (`diff --json` opt-in, side/unified/raw/binary JSON 형식) + F4/F5/F6 (clap surface — `///` doc comment + `--status` Vec<StatusFilter> value_enum + `--branch` default_value="main") + F7 (xtask check-readme-examples sub-command + CI gate 합류) + Phase 8 baseline regression (1000 file 합성 vault, 4-state breakdown 정합 + diff_meaningful/presence non-disruptive) + v0.4.0 annotated tag (main + tag push, v0.3.0 스타일 mirror).

## Active Phase

진행 중 phase 없음 (Phase 8 종료 직후, v0.4.0 release 시점). 다음 phase 진입 시 본 § 갱신.

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner (Phase 6.1 WW).
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **박제 expiration** — Phase 진입마다 재검토 (CLAUDE.md § 박제 정책).
- **자율 진행 회피 영역** (사용자 vague 답변, Phase 5.13.1/5.14 패턴) — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 vague + clean-context 외부 시각 보강 필수 (예외: 외부 시각 source가 plan 본문에 명시되어 있으면 skip — 본 phase § 본문에 source path 명시되어 있으면 정합으로 간주).
