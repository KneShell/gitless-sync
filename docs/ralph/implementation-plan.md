# Implementation Plan

## Status

- Phase 9 진입 (2026-05-12)
- Tasks: 19 (Phase 9)
- Completed: 4 / 19

## Notes for Build Mode

- ralph build mode는 첫 미완료 task (`[ ]`)부터 처리. 의존 순서가 본 plan에 명시 안 됐으면 acceptance + spec 본문에 잠재 의존 명시 (e.g., "X task 결과 위에서 진행").
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Hard gate (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출 lint) 모두 deny active 유지. 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (`project-ops.md`). 신규 task의 acceptance에 unit test 포함.

## Active Phase

### Phase 9 — CLI UX Feedback Fix (vault 도그푸딩 F1/F2/F3 → v0.5.0)

`docs/cli-ux-feedback.md` (vault 측 도그푸딩 노트, 2026-05-12, v0.4.2 (`d09f310`) + sha1 bump 1 commit 후 origin/main HEAD `5f2f439` 기준) 3 Improvement Finding (F1/F2/F3) 해소 + Open Decisions 사용자 사전 확정. 사용자 명시: cli-ux-feedback.md 자체가 vault 측 외부 시각 source + Phase 9 entry 작성 직후 별도 sub-claude clean-context audit 1회 진행 — 두 source 결합 + plan audit chain (general-purpose Agent + clean-context Skill 둘 다 2026-05-12 PASS, finding 모두 entry 본문 사전 반영 완료) 으로 외부 시각 보강 완료. spec 본문 100% 사전 확정 후 ralph 자율 주행. 사용자 stance 정합 (memory `feedback_release_phase_chain.md` + `feedback_quality_vs_complexity.md`).

**ralph 진입 시점 사전 reading 권장**: `docs/cli-ux-feedback.md` § F1~F3 + Good (G1~G4) + 정정 §. 특히 F3 acceptance 본문 ("summary-only 모드라도 failed status entries는 files[]에 emit") + G2 (presence/status 직교) + G4 (stdout/stderr 분리) 본 phase에서 깨뜨리지 않을 것.

**Open Decisions 사전 확정** (사용자 vague 답변, 2026-05-12):
- F1 → Scan/Diff subcommand `#[command(about = "...")]` derive 추가 (clap surface only, schema 영향 0).
- F2 → Init subcommand `about` wording 정밀화 ("template printer" 인상 제거). 두 후보 ("Emit gitless-sync.toml body from input args (stdout)" / "Compose gitless-sync.toml body from --repo/--branch/--ignore, emit to stdout") 중 task 진행 시 spec 본문 + clap derive 양쪽 일치만 acceptance — wording 자체는 task B에서 결정.
- F3 → `--summary-only` mode failed status entry 한정 emit. entry payload는 **축약** (path + presence + failed_reason). full FileEntry 노출 X — "summary-only 정체성 = 카운트 + 무엇이 실패했나 명단" contract 유지. **`presence` 필드 포함 근거** (clean-context audit 2026-05-12): cli-ux-feedback.md § G2 본문이 `presence vs status 두 축 직교 분리` 를 호출자에게 일급 contract 로 박았음 — failed entry에서도 `presence: local_only` (로컬 발생 함정) vs `presence: both` (양쪽 존재 함정) vs `presence: remote_only` (원격 발생) 다음 액션 분기에 필수. path + failed_reason 만 emit 시 호출자가 G2 분기 못 하고 두 번째 scan 호출 — F3 motivation 자기 모순. 추가 비용 0 (enum 1 field). sha/size/mode/diff_meaningful 등 detail field는 여전히 omit — "축약" 정신 보존. ADR 신규 X — spec-output-schema.md § summary-only § + § v1.4 → v1.5 변경 § 갱신만으로 결정 trail 충분.
- schema v1.4 → v1.5 minor bump (F3 한정. F1/F2는 schema 영향 0). 결정 근거: summary-only mode 출력 contract 확장 (omit-only → failed 한정 emit). additive에 가까우나 caller-visible behavior change이므로 minor.
- release: v0.4.2 → **v0.5.0** minor (schema bump 동반). v0.4.3 patch 회피 — schema 버전이 호출자 contract 일급 signal이라 SemVer minor가 정직.
- baseline regression: 타겟티드 unit/integration 한정 (Phase 8 task AA 1000-file synth-vault 패턴 mirror X) — F3 영향 범위가 summary-only mode + failed 한정이라 큰 vault regression이 information value 대비 헤비. failed 발생 fixture (e.g., NFD collision)로 충분.

**환경 주의**: 본 PC = Windows + dasgut user. cli-ux-feedback.md는 동일 PC에서 작성된 도그푸딩 결과 — Phase 8 eval과 달리 환경 mismatch 없음. vault repo (`KneShell/obsidian-vault@main`)는 실제 도그푸딩 대상이고 KneShell/gitless-sync 측 regression은 합성 fixture로 cover.

진행 순서: 9.1 spec/CHANGELOG 사전 확정 → 9.2 F1+F2 clap surface → 9.3 F3 summary-only failed visibility → 9.4 release → 9.5 plan close.

#### Phase 9.1 — spec/CHANGELOG 사전 확정 (5 task, doc-only)

- [x] **A**: spec-cli-interface.md F1 acceptance — `### Acceptance Criteria` (또는 동등) section에 "`cargo run -- --help` stdout에 `scan` / `diff` 서브커맨드 description 한 줄 이상 노출" `[AUTO]` acceptance 추가. cli-ux-feedback.md § F1 본문 "Acceptance" 항목 직인용. Files: docs/specs/spec-cli-interface.md.
- [x] **B** (deps: A): spec-cli-interface.md F2 acceptance — § init subcommand 본문에 `--help` 문구 형식 acceptance 한 줄 추가. 정확 문자열은 "Emit gitless-sync.toml body from input args (stdout)" (Open Decisions § F2 최종 채택). spec acceptance + task H clap derive 양쪽 byte-identical 검증. cli-ux-feedback.md § F2 본문 "Acceptance" 직인용. Files: docs/specs/spec-cli-interface.md.
- [x] **C** (deps: B): spec-output-schema.md § `--summary-only` 출력 갱신 + § v1.4 → v1.5 변경 § 신규 — F3 본진. 기존 "위 JSON에서 `files` 필드 자체를 제거" → "failed status entry 한정 `files[]`에 emit (path + presence + failed_reason 만, sha/size/mode/diff_meaningful 등 detail field는 모두 omit), failed 0건이면 `files` 필드 omit (기존 동작 유지). 그 외 status entry는 summary-only에서 emit 안 함." 명시. `failed_reason` 필드는 기존 `FailedReason` enum 11 variant (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported` / `file_too_large` / `memory_exceeded`) 그대로 유지 — Phase 9에서 enum 추가/제거 0. `presence` 필드는 v1.3에서 도입된 enum 3 variant (`local_only` / `both` / `remote_only`) 그대로. v1.5 변경 §에 "summary-only mode failed visibility (path + presence + failed_reason 3 field)" lock + caller v1.4까지 `files == null` 가정한 분기가 있다면 v1.5에서 깨질 수 있음 명시. Files: docs/specs/spec-output-schema.md.
- [x] **D** (deps: C): spec-output-schema.md § v1.5 신규 Acceptance Criteria — 5 시나리오 `[AUTO]`: (1) `report.schema_version == "1.5"`, (2) summary-only + failed 0 → `files` 필드 omit (v1.4 baseline 유지), (3) summary-only + failed N → `files[]` N entry 포함, (4) summary-only `files[]` entry는 `path` + `presence` + `failed_reason` 세 field만 emit + 그 외 (sha/size/mode/diff_meaningful 등) omit, (5) summary-only + `--status` filter 동시 명시 시 filter 무시 (summary-only 정체성 우선) — Phase 8 patterned. Files: docs/specs/spec-output-schema.md.
- [~] **E** (deps: D): CHANGELOG.md `[Unreleased]` v0.5.0 prep entry — Added (F1 scan/diff `about` derive + F2 init `about` wording 정밀화 + F3 summary-only failed visibility (path + failed_reason)), Changed (schema v1.4 → v1.5), Verified (cli-ux-feedback.md F1/F2/F3 해소). v0.4.0/v0.4.1/v0.4.2 entry 패턴 정합. Files: CHANGELOG.md.

#### Phase 9.2 — F1 + F2 (clap surface) (4 task)

- [ ] **F** (deps: E): F1 — `crates/gitless-sync/src/main.rs` `Commands::Scan` variant에 `#[command(about = "Compare local directory against remote repo, emit 4-state classification JSON")]` derive 추가. cli-ux-feedback.md § F1 "기대" 본문 wording 채택. 한 줄 description, 종지부 없음 (clap 관행). spec-cli-interface.md task A acceptance 정합.
- [ ] **G** (deps: F): F1 — `Commands::Diff` variant에 `#[command(about = "Show unified diff (or JSON) of a single file vs remote")]` derive 추가. 동일 출처 wording.
- [ ] **H** (deps: G): F2 — `Commands::Init` variant 기존 `about = "Print a gitless-sync.toml template to stdout (you redirect to a file)"` → 정밀화. **최종 채택 wording** (Open Decisions § F2): `about = "Emit gitless-sync.toml body from input args (stdout)"`. cli-ux-feedback.md § F2 첫 번째 후보 (짧고 명확). `after_help` (Example) 그대로 보존. task B spec 본문과 byte-identical 정합 검증 (acceptance 위반 시 task `[!]` BLOCKED).
- [ ] **I** (deps: H): integration test — `tests/cli_help_about.rs` 신규 (또는 기존 integration suite 안). `cargo run -- --help` stdout 캡처 + `"Scan"` / `"Diff"` / `"Init"` 각 description 정확 문자열 noncontain → contain 전이 (regression evidence). Phase 8 task X 패턴 (clap parse error / valid 후보 검증) mirror. acceptance: spec-cli-interface.md task A/B `[AUTO]` acceptance 매핑.

#### Phase 9.3 — F3 (summary-only failed visibility) (6 task)

- [ ] **J** (deps: I): F3 schema bump — `crates/gitless-sync/src/commands/scan/output.rs` `SCHEMA_VERSION` `"1.4"` → `"1.5"` constant 갱신. 기존 unit test `schema_version_field_serializes_as_1_4` → `_1_5` 함수명 + 본문 갱신. spec-output-schema.md task C/D acceptance 정합. **F3 impl 본진 (task K~L) 전에 schema bump 먼저** — Phase 8 task L 패턴 mirror (impl 후 bump보다 schema-driven 사전 bump가 v1.4 lock test 갱신 흐름 깔끔).
- [ ] **K** (deps: J): F3 impl — `crates/gitless-sync/src/commands/scan/mod.rs::build_report` summary-only 분기 (현행 line 122 `let files = if args.summary_only { None } else { Some(entries) };`) 갱신. summary-only mode + failed entry 존재 시 failed 한정 minimal entry vector emit. entry minimal = path + presence + failed_reason 3 field (G2 호환). 구현 선택지 두 가지: (a) 별도 `SummaryFailedEntry { path, presence, failed_reason }` struct 신규 + `ScanReport.files` 타입 enum 변경, (b) 기존 `FileEntry` 재사용 + summary-only 모드 시 strip 외 모든 Option field None + `#[serde(skip_serializing_if)]`로 omit. (b) 권장 — wire JSON identical + 구현 단순 + 추가 struct 0. **주의**: `presence` 필드는 v1.3부터 모든 FileEntry에 emit (omit 0) 이라 strip 시 보존. acceptance: spec-output-schema.md task D `[AUTO]` 4번 (entry 3 field 만 emit).
- [ ] **L** (deps: K): F3 helper — task K 구현 시 strip 로직이 build_report 안에 inline되면 LOC 300 cap 위험. 별도 helper (e.g., `commands/scan/output.rs::strip_to_summary_failed` 또는 `commands/scan/pipeline/finalize/` 안 helper) 추출. cycle/cross-slice 0 + slice-internal directional discipline 유지. acceptance: helper unit test 1건 (full FileEntry input → minimal output, path/presence/failed_reason 보존 + sha/size/mode/diff_meaningful 모두 None 확인).
- [ ] **M** (deps: L): v1.4 → v1.5 backward-compat lock test — `tests/scan_output_backward_compat.rs` 갱신. **현행 코드 사실**: V10/V11/V12 client struct + `parse_v1_0`/`_1`/`_2` + `v1_3_sample_json`만 존재 (V13/V14 별도 client struct 없음 — v1.3 + v1.4는 field 추가 0이라 V12 client 그대로 v1.3/v1.4 sample 파싱 가능, 별도 client 도입 안 됨). v1.5는 summary-only mode failed entry contract 확장이므로 (i) `v1_5_sample_json` fixture 신규 (summary-only + failed N 시나리오 포함), (ii) `V15` client struct 신규 — `files: Option<Vec<V15FailedEntry { path: String, presence: String, failed_reason: String }>>` shape, summary-only + failed 케이스 parse 검증. (iii) V10~V12 client + v1.5 sample 파싱 시 unknown field 0 보장 (path/presence/failed_reason 모두 v1.1~v1.3부터 존재) — backward-compat lock. V13/V14 별도 struct 신규 X (현행과 동일 유지).
- [ ] **N** (deps: M): F3 unit test 4 시나리오 — `commands/scan/output.rs::tests` 또는 `mod.rs::tests`: (1) summary-only + failed 0 → output JSON에 `"files"` 문자열 미포함 (기존 동작 유지), (2) summary-only + failed 1 → `files` 배열 1 entry + `path` + `presence` + `failed_reason` 세 key만, (3) summary-only + failed N + identical/drift entry M → `files[]` len == N (failed만), (4) summary-only + `--status drift` + failed 존재 → `files[]`에 failed 그대로 (summary-only 정체성 우선, filter 무시) — spec-output-schema.md task D `[AUTO]` 5번 매핑. `commands/scan/status_filter.rs` 기존 test 2건 동시 갱신: `build_report_summary_only_drops_files_field` (line 24~) → "failed 0건 시 drop" 의미 정확화 + 이름 유지하되 fixture에 failed 0 명시, `build_report_summary_only_overrides_status_filter` (line 101~) → fixture에 failed N + drift M 동시 명시 + summary-only가 filter override해도 failed entry는 등장 검증.
- [ ] **O** (deps: N): F3 integration test — 실제 failed entry 발생 fixture (Phase 5 도메인 함정 중 합성 가능한 케이스). **fixture 택일** (자율 진행 중 결정 가능): `long_path` 권장 (Windows OS-level 함정, 250자 path 생성 + scan trigger — 합성 가장 단순, NFD collision은 Unicode normalization OS dependency 있음). `tests/scan_summary_only_failed.rs` 신규. fixture 셋업 + `--summary-only` 호출 + JSON parse + `summary.failed > 0` + `files[]` failed entry (path + presence + failed_reason 3 key) 검증. cli-ux-feedback.md § F3 본문 motivation ("어떤 파일이 실패했는지 한 호출로 확인") 직접 검증.

#### Phase 9.4 — release (3 task)

- [ ] **P** (deps: O): README.md 갱신 — `### scan` 섹션에 `--summary-only` 동작 정밀화 (failed 0건 → `files` omit, failed N건 → minimal entry list `{path, presence, failed_reason}`). `### diff` 섹션 영향 0. F1/F2 `--help` 출력 예시는 README 본문에 snippet 0건이라 (sub-agent audit 2026-05-12 확인) 갱신 대상 없음. doc-only.
- [ ] **Q** (deps: P): CHANGELOG.md v0.5.0 entry finalize — Added (F1/F2 about wording + F3 summary-only failed visibility), Changed (schema v1.4 → v1.5), Spec (cli-ux-feedback.md F1/F2/F3 acceptance 정합), Verified (unit + integration test pass + cli-ux-feedback.md 3 friction 해소). v0.4.2 entry 패턴 mirror.
- [ ] **R** (deps: Q): v0.5.0 release tag — `git tag v0.5.0 -m "..." && git push origin main && git push origin v0.5.0`. cli-ux-feedback.md 자체가 외부 시각 source + Phase 9 entry 작성 직후 clean-context audit (사용자 명시) 둘 다 사전 진행 — release 전 별도 audit 추가 안 함. v0.4.2 tag 스타일 mirror (annotated tag + main/tag push).

#### Phase 9.5 — plan close (1 task)

- [ ] **S** (deps: R): 본 plan Phase 9 task 모두 [x] mark + § 갱신 (Active → Completed Phases). 이동 시 1~2 sentence 요약만 retain (자세한 task별 결과는 git history `git log --grep="Phase 9"` + commit message + CHANGELOG.md v0.5.0 entry로 cover, Phase 5/6/7/8 패턴 mirror).

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

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner.
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **자율 진행 회피 영역** — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 외부 시각 검토 권장.
