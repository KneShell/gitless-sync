# Implementation Plan

## Status
- Phase 8 진입 (2026-05-10)
- Tasks: 31 (Phase 8)
- Completed: 13 / 31

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

## Active Phase

### Phase 8 — LLM-as-Caller Usability Fix (eval 7 friction → v0.4.0)

`docs/research/llm-as-caller-usability-eval.md` (다른 PC 평가, 2026-05-10) 7 friction (P0~P3) 해소 + Open Decisions 4건 사전 확정. 사용자 명시: vague + clean-context skip (eval 자체가 외부 시각, 2026-05-10) — Constraints § 자율 진행 회피 영역 본 phase 한정 면제 (eval source가 본 plan 본문에 명시됨). spec 본문 100% 사전 확정 후 ralph 자율 주행. 사용자 stance 정합 (memory `feedback_release_phase_chain.md` + `feedback_quality_vs_complexity.md`).

**ralph 진입 시점 사전 reading 권장**: `docs/research/llm-as-caller-usability-eval.md` § F1~F7 + Strengths + Limitations 본문. 특히 J/K/P/Q task의 fixture 작성 시 eval evidence (CRLF vs LF path / `.obsidian/app.json` local-only path 등) 정확 reference.

**환경 주의**: 본 PC는 Windows + dasgut user. eval 진행 PC는 다른 PC (admin user). eval 본문의 vault path (`C:\Users\admin\iCloudDrive\iCloud~md~obsidian`)는 본 PC 접근 불가 — AA task에서 합성 vault로 대체.

**Open Decisions 사전 확정** (eval (a) recommendation 채택):
- F1 → `diff_meaningful: bool` field (scan entry, 1 호출로 정보 다 받음)
- F2 → `presence: "local_only"|"both"|"remote_only"` field (4분류 status 유지, backward compat)
- F3 → `diff --json` opt-in (기존 unified text default 유지 + json 분기)
- F1+F2 → ADR 0014 'scan-diff metadata contract' 신규
- schema v1.2 → v1.3 minor bump

진행 순서: 8.1 spec/ADR → 8.2 F1+F2 schema → 8.3 F3 diff --json → 8.4 F4/F5/F6 surface → 8.5 F7 sanity → 8.6 release.

#### Phase 8.1 — spec/ADR 사전 확정 (5 task, doc-only)

- [x] **A**: ADR 0014 'scan-diff metadata contract' 신규 (Write) — F1+F2 묶음 결정 trail. context: eval F1/F2 본문 + Open Decisions. decision: diff_meaningful + presence field 추가, 4-state status 유지. 영향 spec mapping. Files: docs/adr/0014-scan-diff-metadata-contract.md.
- [x] **B** (deps: A): spec-output-schema.md schema v1.2 → v1.3 minor bump — `files[].diff_meaningful: Option<bool>` + `files[].presence: enum` field 추가 spec. v1.0/v1.1/v1.2 backward-compat lock test 정합 (Phase 7.2 task P 패턴). spec § v1.2 → v1.3 변경 § 신규. Files: docs/specs/spec-output-schema.md.
- [x] **C** (deps: B): spec-cli-interface.md `diff --json` 옵션 spec — opt-in, default unified text 유지. JSON 형식 `{"side": "...", "unified": "..." | null, "raw": "..." | null, "binary": bool}`. Files: docs/specs/spec-cli-interface.md.
- [x] **D** (deps: C): spec-output-schema.md § diff sub-schema 신규 — C의 JSON 형식 authoritative spec. Files: docs/specs/spec-output-schema.md.
- [x] **E** (deps: D): CHANGELOG.md `[Unreleased]` v0.4.0 prep entry — Added (diff_meaningful / presence / diff --json / clap surface fix), Changed (schema v1.3), Verified (eval 7 friction 해소). Files: CHANGELOG.md.

#### Phase 8.2 — F1 (diff_meaningful) + F2 (presence) (8 task)

- [x] **F**: `compare.rs::FileEntry` struct에 2 field 추가 — `diff_meaningful: Option<bool>` + `presence: Presence` (`#[derive(Serialize)]` + `#[serde(rename_all = "snake_case")]` enum). spec-output-schema.md § v1.3 정합.
- [x] **G** (deps: F): `Presence` enum 정의 — `LocalOnly` / `Both` / `RemoteOnly`. compare.rs.
- [x] **H** (deps: G): `compare.rs::compare` 함수 — local/remote 존재 여부로 presence 결정 + Hashed entry는 sha 비교 + normalize-equal 검증으로 diff_meaningful 계산 (sha differ but normalize-equal → false, sha differ AND normalize-diff → true, identical → false). Failed/local-or-remote-only entry는 None.
- [x] **I** (deps: H): `commands/scan/pipeline/finalize.rs` (또는 동등) — entry assemble 시점에 presence + diff_meaningful 채움. spec-hash-and-normalize.md § normalize 정합 검증 재사용.
- [x] **J** (deps: I): unit test 6 시나리오 — Identical (presence=both, diff_meaningful=Some(false)), LocalOnlyChanged-both (presence=both, diff_meaningful=Some(true) or Some(false)), LocalOnly (presence=local_only, diff_meaningful=None), RemoteOnly (presence=remote_only, diff_meaningful=None), Drift (presence=both, diff_meaningful=Some(true)), Failed (presence=both, diff_meaningful=None).
- [x] **K** (deps: J): integration test — eval F1 evidence 케이스 (sha differ but normalize-equal, e.g., CRLF vs LF 한쪽) 합성 fixture + scan + diff_meaningful=Some(false) 검증.
- [x] **L** (deps: K): SCHEMA_VERSION "1.2" → "1.3" + lock test 갱신 (v1.0/v1.1/v1.2 backward-compat). spec-output-schema.md § v1.3 신규 Acceptance Criteria 정합.
- [x] **M** (deps: L): spec-output-schema.md § v1.3 신규 Acceptance Criteria N 시나리오 unit test (output.rs::tests).

#### Phase 8.3 — F3 (diff --json) (5 task)

- [~] **N**: `commands/diff/args.rs` clap struct에 `--json` flag 추가. spec-cli-interface.md `diff --json` 정합.
- [ ] **O** (deps: N): `commands/diff/render.rs` (또는 동등) — `--json` 분기 시 JSON 형식 직렬화 (side / unified / raw / binary). 기존 unified text path 유지 (default, opt-out).
- [ ] **P** (deps: O): unit test 4 시나리오 — both normalize-equal + --json (`{"side":"both","unified":"","raw":null,"binary":false}`), local_only + --json (`{"side":"local_only","unified":null,"raw":"...","binary":false}`), both normalize-diff + --json (unified populated), binary + --json (`binary:true`).
- [ ] **Q** (deps: P): integration test — eval F3 evidence 케이스 (local-only) + diff --json 호출 + JSON 형식 검증.
- [ ] **R** (deps: Q): README.md `### diff` 섹션 갱신 — `--json` 옵션 + 3 case 명시. doc-only.

#### Phase 8.4 — F4/F5/F6 (clap surface) (6 task)

- [ ] **S**: F4 — `commands/scan/args.rs` clap struct 각 field 위에 `///` doc comment 한 줄씩 (--summary-only / --status / --repo / --branch / --local / --ignore / --keep-bom / --pretty / --backend).
- [ ] **T** (deps: S): F4 — `commands/init/args.rs` 동일 (--repo / --branch).
- [ ] **U** (deps: T): F4 — `commands/diff/args.rs` 동일 (--repo / --branch / --local / --keep-bom / --json).
- [ ] **V** (deps: U): F5 — `commands/scan/args.rs` `--status`를 `Vec<StatusFilter>` enum + clap `value_enum` derive + `value_delimiter = ','`. 자동으로 --help에 [possible values] + 에러에 valid 후보 노출. spec 변경 없음 (이미 5 카테고리 spec).
- [ ] **W** (deps: V): F6 — `commands/scan/args.rs`/`commands/diff/args.rs`/`commands/init/args.rs` `--branch` clap `default_value = "main"`. README "defaults to main" 약속을 --help에도 노출.
- [ ] **X** (deps: W): unit test — F5 valid status filter parsing + invalid status 에러 메시지 valid 후보 포함 검증.

#### Phase 8.5 — F7 (CI README sanity) (3 task)

- [ ] **Y**: `xtask/src/check_readme_examples.rs` 신규 sub-command — README.md에서 ` ```sh ` 코드블록 추출 + Quick Start 섹션 코드 실제 실행 (`cargo build --release` 사전 + `gitless-sync init --repo dummy/dummy --branch main` stdout redirect target은 `tempfile::NamedTempFile` 또는 `std::env::temp_dir()` join한 OS-agnostic path). exit 0 검증. 본 PC Windows + CI Linux runner 둘 다 cross-platform 호환 필수 — `/tmp/` 같은 POSIX 경로 hardcode 금지.
- [ ] **Z** (deps: Y): `.github/workflows/ci.yml` 새 step 추가 — `cargo xtask check-readme-examples`. Phase 6 hard gate에 합류.
- [ ] **AA** (deps: Z): Phase 8 결과 baseline regression 검증 — `cargo xtask synth-vault --out tmp/synth-vault-42 --count 1000 --seed 42` (Phase 7 task S xtask 재생성, 본 PC Windows에서 generate 가능) + KneShell/gitless-sync remote scan 측정 + Phase 7 task T 결과 (1000 local_only_changed / 129 remote_only_changed / 0 drift / 0 failed)와 4-state 카운트 정합. 추가 field (diff_meaningful / presence)가 4-state breakdown 안 깨뜨림 검증. 결과 `docs/research/phase8-regression.md` 신규. 주의: eval 본문 vault (`C:\Users\admin\iCloudDrive\iCloud~md~obsidian`)는 다른 PC (admin user) 한정 — 본 PC (dasgut user) 접근 불가, 합성 vault로 대체.

#### Phase 8.6 — release tag (3 task)

- [ ] **BB** (deps: AA): CHANGELOG.md v0.4.0 entry finalize — Added (F1/F2 schema, F3 diff --json, F4/F5/F6 surface, F7 CI sanity), Changed (schema v1.2 → v1.3), Verified (eval friction 해소 + vault regression). v0.3.0 entry 패턴 정합.
- [ ] **CC** (deps: BB): v0.4.0 release tag — `git tag v0.4.0 -m "..." && git push origin main && git push origin v0.4.0`. 사용자 명시 'eval 자체가 외부 시각'이라 사전 sub-claude clean-context audit skip (Phase 7.4 패턴 변형). v0.3.0 tag 스타일 mirror (annotated tag + main/tag push + ancestor=0 검증).
- [ ] **DD** (deps: CC): 본 plan Phase 8 task 모두 [x] mark + § 갱신 (Active → Completed Phases). 이동 시 1~2 sentence 요약만 retain (자세한 task별 결과는 git history `git log --grep="Phase 8"` + commit message + CHANGELOG.md v0.4.0 entry로 cover, Phase 5/6/7 패턴 mirror).

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner (Phase 6.1 WW).
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **박제 expiration** — Phase 진입마다 재검토 (CLAUDE.md § 박제 정책).
- **자율 진행 회피 영역** (사용자 vague 답변, Phase 5.13.1/5.14 패턴) — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 vague + clean-context 외부 시각 보강 필수 (예외: 외부 시각 source가 plan 본문에 명시되어 있으면 skip — 본 phase § 본문에 source path 명시되어 있으면 정합으로 간주).
