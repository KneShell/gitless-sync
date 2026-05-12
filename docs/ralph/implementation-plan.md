# Implementation Plan

## Status

- Phase 9 종료 (2026-05-12)
- Tasks: 19 (Phase 9)
- Completed: 19 / 19

## Notes for Build Mode

- ralph build mode는 첫 미완료 task (`[ ]`)부터 처리. 의존 순서가 본 plan에 명시 안 됐으면 acceptance + spec 본문에 잠재 의존 명시 (e.g., "X task 결과 위에서 진행").
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Hard gate (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출 lint) 모두 deny active 유지. 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (`project-ops.md`). 신규 task의 acceptance에 unit test 포함.

## Active Phase

진행 중 phase 없음 (Phase 9 종료 직후, v0.5.0 release 시점). 다음 phase 진입 시 본 § 갱신.

## Completed Phases

### Phase 5 (2026-05-09 ~ 05-10, 57 task: A~TT)
도메인 함정 정리 본진 + plumbing follow-up + sibling cleanup + clean-context audit follow-up + md 자료 audit. 8 핵심 함정 + 추가 함정 4건 detect/handle + Schema v1.0→1.1 (mode/failed_reason 9 enum/lfs_pointer) + vault dogfood 117 files / 0 drift / 0 failed + CLAUDE.md privacy section 제거.

### Phase 6 (2026-05-09 ~ 05-10, 23 task: A~T 본진 + UU/VV/WW v0.2.x cleanup)
Code Quality Strengthening 본진 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출 hard gate) + v0.2.x cleanup (ADR 0010 cognitive_complexity vs LOC orthogonal proxy + CI runner Linux 전환 G-018).

### Phase 7 (2026-05-10, 36 task: A~JJ 본진 + clean-context audit chain, v0.3.0)
vault scale + Trees sub-tree fallback + 큰 파일 임계치 + clean-context audit chain (depth 3/3) + v0.3.0 release.

### Phase 8 (2026-05-10, 31 task: A~DD, v0.4.0)
LLM-as-Caller Usability eval 7 friction (P0~P3) 해소 + v0.4.0 release. F1/F2 (`diff_meaningful: Option<bool>` + `presence: enum local_only/both/remote_only` 2 field, schema v1.2 → v1.3) + F3 (`diff --json` opt-in) + F4/F5/F6 (clap surface) + F7 (xtask check-readme-examples + CI gate).

### v0.4.1 (2026-05-10, post-Phase-8 minor fix)
F5 contract 회복 (clap argument-parse 실패 → CONFIG_ERROR JSON + exit 1) + F4 백틱 노이즈 제거 + main.rs 분해 (clippy too_many_lines 60 cap).

### v0.4.2 (2026-05-11, Issue #1 hotfix)
byte-identical files (UTF-8 BOM / LF-CRLF cosmetic SHA drift) 가 `LocalOnlyChanged` 로 잘못 분류되던 spec/code drift fix. schema_version 1.3 → 1.4 (additive 의미 정확화). 결정 trail은 `docs/adr/0015-cosmetic-identical-classification.md`.

### Phase 9 (2026-05-12, 19 task: A~S, v0.5.0)
Vault 도그푸딩 `docs/cli-ux-feedback.md` 3 Improvement Finding (F1/F2/F3) 해소 + v0.5.0 release. F1/F2 (clap `#[command(about)]` derive — scan/diff 신규 추가 + init wording "Emit gitless-sync.toml body from input args (stdout)"로 정밀화) + F3 (`--summary-only` mode failed visibility — failed status entry 한정 minimal `{path, presence, failed_reason}` 3 field emit, summary-only 정체성 + G2 presence 직교 contract 유지, schema v1.4 → v1.5 minor bump + tests/scan_output_backward_compat.rs V15 client 추가 + tests/scan_summary_only_failed.rs long_path fixture integration) + v0.5.0 annotated tag (main + tag push, v0.4.2 스타일 mirror).

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner.
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **자율 진행 회피 영역** — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 외부 시각 검토 권장.
