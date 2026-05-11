# Implementation Plan

## Status

진행 중 phase 없음. 다음 phase 진입 시 본 § 갱신.

## Notes for Build Mode

- ralph build mode는 첫 미완료 task (`[ ]`)부터 처리. 의존 순서가 본 plan에 명시 안 됐으면 acceptance + spec 본문에 잠재 의존 명시 (e.g., "X task 결과 위에서 진행").
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Hard gate (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출 lint) 모두 deny active 유지. 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (`project-ops.md`). 신규 task의 acceptance에 unit test 포함.

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner.
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **자율 진행 회피 영역** — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 vague + clean-context 외부 시각 보강 권장.
