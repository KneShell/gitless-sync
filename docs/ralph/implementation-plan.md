# Implementation Plan

## Status

- Phase 10 종료 (2026-05-12)
- Tasks: 17 (Phase 10)
- Completed: 17 / 17

## Notes for Build Mode

- ralph build mode는 첫 미완료 task (`[ ]`)부터 처리. 의존 순서가 본 plan에 명시 안 됐으면 acceptance + spec 본문에 잠재 의존 명시 (e.g., "X task 결과 위에서 진행").
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Hard gate (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출 lint) 모두 deny active 유지. 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (`project-ops.md`). 신규 task의 acceptance에 unit test 포함.

## Active Phase

진행 중 phase 없음 (Phase 10 종료 직후, v0.6.0 release 시점). 다음 phase 진입 시 본 § 갱신.

## Completed Phases

Phase 1~10 + v0.4.1 / v0.4.2 누적 history — 자세한 내용은 git log + CHANGELOG.md 참조. 핵심 마일스톤:

- Phase 5 (2026-05-09~10) — 도메인 함정 8 핵심 + 4 추가 detect/handle + schema v1.0→1.1.
- Phase 6 (2026-05-09~10) — Hard gate 활성화 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출).
- Phase 7 (2026-05-10) — vault scale + Trees sub-tree fallback + 큰 파일 임계 + schema v1.1→1.2 + v0.3.0.
- Phase 8 (2026-05-10) — LLM-as-caller eval 7 friction 해소 (F1/F2 schema v1.2→1.3 + F3 diff --json + F4~F6 clap surface + F7 CI README sanity) + v0.4.0.
- v0.4.1 (2026-05-10) — clap argument-parse contract 회복 (try_parse + CONFIG_ERROR JSON wrap).
- v0.4.2 (2026-05-11) — cosmetic identical classification fix (normalize-equal sha-differ → Identical) + schema v1.3→1.4 (ADR 0015).
- Phase 9 (2026-05-12) — vault dogfood F1/F2/F3 (scan/diff about derive + init wording 정밀화 + summary-only failed visibility) + schema v1.4→1.5 + v0.5.0.
- Phase 10 (2026-05-12) — post-v0.5.0 clean-context audit Finding 1/2/3 해소 (SemVer 면제 근거 명문화 + hash_io explicit emit + minimal entry shape 발산 강조) + schema v1.5→1.6 + v0.6.0.

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner.
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **자율 진행 회피 영역** — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 외부 시각 검토 권장.
