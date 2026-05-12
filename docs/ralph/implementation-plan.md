# Implementation Plan

## Status

- Phase 10 진입 (2026-05-12)
- Tasks: 17 (Phase 10)
- Completed: 5 / 17

## Notes for Build Mode

- ralph build mode는 첫 미완료 task (`[ ]`)부터 처리. 의존 순서가 본 plan에 명시 안 됐으면 acceptance + spec 본문에 잠재 의존 명시 (e.g., "X task 결과 위에서 진행").
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Hard gate (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출 lint) 모두 deny active 유지. 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (`project-ops.md`). 신규 task의 acceptance에 unit test 포함.

## Active Phase

### Phase 10 — post-v0.5.0 verification finding 해소 (Finding 1/2/3 → v0.6.0)

v0.5.0 release 직후 clean-context audit (2026-05-12, 메모리 차단 fresh context) 결과 발견된 3 finding 해소 + v0.6.0 minor release. 사용자 사전 확정 (2026-05-12): 3건 한 phase 묶음 + implementation/spec 재정비 + 일회성 자료 정리 (`docs/cli-ux-feedback.md` v0.4.2→v0.5.0 transition source — 본 phase 진입 직전 삭제 + spec/CHANGELOG/test reference 9곳 정리 완료 [spec-output-schema.md § history note 라인 14 + line 185 + line 337 + spec-cli-interface.md line 84 + line 118 + CHANGELOG.md v0.5.0 entry 3곳 + tests/cli_help_about.rs + tests/scan_summary_only_failed.rs 주석 2곳], `docs/specs/spec-config.md` § Cache (Phase 4) ADR 0008로 obsolete된 § 삭제 완료). Phase 10 진입 직전 plan audit 2종 (general-purpose Agent + clean-context Skill, 2026-05-12) PASS — critical 4 + minor 4 finding 모두 본문 사전 반영 완료.

**ralph 진입 시점 사전 reading 권장**: 본 § 본문 + § Open Decisions + `docs/specs/spec-output-schema.md` § v1.5 본문 + `docs/specs/spec-error-contracts.md` § FailedReason 정의. 특히 Finding 2 (hash_io explicit emit) 가 wire shape change라 schema v1.5 → v1.6 minor bump 동반.

**Open Decisions 사전 확정** (사용자 vague 답변, 2026-05-12):
- Finding 1 (SemVer 라벨 모순) → spec-output-schema.md § v1.4 → v1.5 변경 § 본문에 면제 근거 한 줄 명문화. "v1.4 caller `files == null` 또는 key 부재로 summary-only mode 판단하는 분기는 v1.4 시점 도입된 신규 가정이라 SemVer 보호 대상 아님" 형식. 라벨 ("minor bump")은 유지 — wire-shape 기준 정합. doc-only.
- Finding 2 (hash_io entry 필드 부재 sentinel 위험) → `FailedReason` enum `None` 특수 케이스 제거 + 명시 `HashIo` variant 추가 + `failed_reason: "hash_io"` wire emit. wire shape change 동반 → schema v1.5 → v1.6 minor bump. impl + spec + test 동반.
- Finding 3 (minimal entry shape 발산) → spec-output-schema.md § minimal entry shape 발산 강조 한 줄 추가 + migration 강조 "응답 shape 추론 금지, caller 자신의 `--summary-only` argument 기준 분기" lock. doc-only.
- release: v0.5.0 → **v0.6.0** minor (Finding 2 wire shape change + schema bump 동반). v0.5.1 patch 회피 — schema 버전이 호출자 contract 일급 signal이라 SemVer minor가 정직 (Phase 9 동일 정책).
- 진입 직전 spec/일회성 자료 정리 (cli-ux-feedback.md 삭제 + 4 reference 정리 + spec-config.md § Cache 삭제) — Phase 10 task 외 자율 처리. plan 본문은 Phase 10 진입 시점부터 catch-up.

**환경 주의**: 본 PC = Windows + dasgut user. Phase 9 pattern mirror — 외부 시각 source는 (i) clean-context audit (post-v0.5.0) 본 결과 finding, (ii) plan 본문 audit chain (Phase 9 진입 시 사용한 general-purpose Agent + clean-context Skill 둘 다 본 phase 진입 직전 재호출 권장).

진행 순서: 10.1 spec/CHANGELOG 사전 확정 → 10.2 Finding 2 impl (wire shape + schema bump) → 10.3 test 갱신 + spec 마감 → 10.4 release → 10.5 plan close.

#### Phase 10.1 — spec/CHANGELOG 사전 확정 (5 task, doc-only)

- [x] **A**: spec-output-schema.md § v1.4 → v1.5 변경 § 본문에 Finding 1 면제 근거 한 줄 추가 — "v1.4 caller `files == null` / key 부재 분기는 v1.4 시점 도입된 신규 가정이라 SemVer 보호 대상 아님". `minor bump` 라벨 유지 + 면제 근거 명문화 둘 다. Files: docs/specs/spec-output-schema.md.
- [x] **B** (deps: A): spec-output-schema.md § v1.5 → v1.6 변경 § 신규 — Finding 2 본진. (i) hash_io entry explicit emit (`failed_reason: "hash_io"`) 도입, (ii) wire shape change 동반, (iii) v1.5 caller 가 v1.6 응답 파싱 시 `failed_reason` 필드 absent 가정한 분기는 깨질 수 있음 backward-compat 표 명시, (iv) migration: "missing-key sentinel 금지, `failed_reason == "hash_io"` 명시 분기" lock, **(v) Finding 1 면제 logic 일반화 — v1.5 → v1.6 caller `failed_reason` field absent 가정 분기는 v1.5 시점 도입된 신규 가정이라 SemVer 보호 대상 아님. task A 의 v1.4→v1.5 면제 근거와 동일 패턴 mirror — 매 schema bump 마다 신규 가정 caller 분기는 보호 대상 아님 공통 면제 표 한 줄 추가**. Phase 9 task C 패턴 mirror. Files: docs/specs/spec-output-schema.md.
- [x] **C** (deps: B): spec-output-schema.md § `--summary-only` 출력 또는 § v1.5 → v1.6 변경 § 안에 Finding 3 강조 한 줄 추가 — "summary-only `files[]` entry는 일반 mode entry와 shape 발산 (status/sha/size/mode/diff_meaningful detail field omit). caller 는 응답 shape 추론 금지, 자신의 `--summary-only` argument 기준 mode 분기". doc-only. Files: docs/specs/spec-output-schema.md.
- [x] **D** (deps: C): spec-output-schema.md § v1.6 신규 Acceptance Criteria — 5 시나리오 `[AUTO]`: (1) `report.schema_version == "1.6"`, (2) summary-only + failed (hash_io) → `files[]` entry는 `path` + `presence` + `failed_reason: "hash_io"` 세 field (이전 v1.5 의 `path + presence` 2 field 에서 변경), (3) summary-only + failed (그 외 reason: encoding / lfs_pointer / submodule 등) → 동일 3 field 유지, (4) v1.5 caller가 v1.6 JSON 파싱 시 hash_io entry 정상 deserialize (failed_reason field 가 등장), (5) status omit 정책 유지 (Finding 3 강조 정합, summary-only files[] entry 정의상 failed). Files: docs/specs/spec-output-schema.md.
- [x] **E** (deps: D): spec-error-contracts.md FailedReason 정의 갱신 + CHANGELOG.md `[Unreleased]` v0.6.0 prep entry — spec: FailedReason enum 정의에서 `None` 특수 케이스 (`hash_io` signal) 제거 + 명시 `HashIo` variant 추가 + serde rename `"hash_io"` 정합. CHANGELOG: Added (Finding 1 SemVer 면제 근거 + Finding 2 hash_io explicit emit + Finding 3 minimal entry shape 발산 강조), Changed (schema v1.5 → v1.6, FailedReason `None` 특수 케이스 → `HashIo` variant). Files: docs/specs/spec-error-contracts.md + CHANGELOG.md.

#### Phase 10.2 — Finding 2 impl (wire shape + schema bump) (5 task)

- [ ] **F** (deps: E): schema bump — `crates/gitless-sync/src/commands/scan/output.rs` `SCHEMA_VERSION` `"1.5"` → `"1.6"` constant 갱신. 기존 unit test `schema_version_field_serializes_as_1_5` → `_1_6` 함수명 + 본문 갱신. spec-output-schema.md task D `[AUTO]` 1번 정합. Phase 9 task J 패턴 mirror.
- [ ] **G** (deps: F): FailedReason enum 변경 — `crates/gitless-sync/src/commands/scan/compare/types.rs` (또는 FailedReason 정의 위치): `None` 특수 케이스 의미를 enum variant `HashIo` 로 명시화. `#[derive(Serialize)]` + `#[serde(rename_all = "snake_case")]` 또는 `#[serde(rename = "hash_io")]` 명시. spec-error-contracts.md task E 정합.
- [ ] **H** (deps: G): caller 전수 갱신 — `FailedReason::None` 사용처 전체 grep + 각 의미 재검토. plan audit 시점 (2026-05-12) 확인 사이트 10~11곳: `compare/types.rs:23` (enum 정의) + `output.rs:118,165` + `summary_view.rs:5,92,209,214,237` + `pipeline/normalize_pass.rs:109` + `pipeline/finalize/pre_entry.rs:134` + `pipeline/hash_pass/local.rs:59`. task G enum variant 제거 시 compile error 로 전수 강제 발견 (silent semantic invert risk 낮음) — 그러나 각 사용처 의미가 `None == hash_io signal` 가정인지 단순 `Option::None` 인지 분간 + variant 명시화 후 정합 검증. `Option<FailedReason>` 시그니처는 유지 (Failed 외 entry 는 여전히 None) — variant 만 변경. compile error 0 + cycle/cross-slice 0 + clippy warning 0 보장.
- [ ] **I** (deps: H): summary-only minimal entry 갱신 — `commands/scan/summary_view.rs::project_files` / `strip_to_summary_failed` (또는 동등 helper): FailedReason::HashIo entry 도 다른 reason 과 동일 3 field emit (`path + presence + failed_reason: "hash_io"`). spec-output-schema.md task D `[AUTO]` 2번 정합. v1.5 의 `path + presence` 2 field special case 제거.
- [ ] **J** (deps: I): v1.5 → v1.6 backward-compat lock test — `tests/scan_output_backward_compat.rs` 갱신. **현행 코드 사실 (plan audit 2026-05-12 확인)**: V10 / V11 / V12 / V15 client struct 4개만 존재 — V13 / V14 client 부재 (intermediate version skip 의도, Phase 8/v0.4.2 시 schema v1.3/v1.4 가 field 추가 0 또는 additive 정확화라 V12 client 로도 파싱 가능). v1.6 lock 추가 시: (i) V16 client struct 신규 + `v1_6_sample_json` fixture (summary-only + hash_io entry `failed_reason: "hash_io"` 명시 포함, 최소 2 entry: hash_io + 다른 reason e.g., long_path 비교 가능), (ii) **V10 / V11 / V12 / V15 client × v1.6 sample 4 client lock — V13/V14 신규 추가 X** (현행 구조 유지). (iii) "unknown field 0" 정의: 각 client struct 가 `#[serde(deny_unknown_fields)]` 명시 시 v1.6 신규 field 등장하면 parse error — V10~V12 시점에 등장하는 신규 field (failed_reason / mode / size_bytes / presence / diff_meaningful) 모두 already-known 이므로 v1.6 추가 field 0 → parse OK. (iv) v1.5 client × v1.6 sample: hash_io entry 의 failed_reason 값이 `"hash_io"` 로 등장 → v1.5 client 측 `FailedReason` enum 정의에 `HashIo` variant 없으면 (V15 struct 의 `failed_reason: Option<String>` 직렬화 가정) parse 시 enum 매칭 우회 → `Some("hash_io")` 로 정상 deserialize 검증 (`v1_5_client_parses_v1_6_hash_io_entry_with_some_hash_io` 패턴). 본 task 결과로 v1.0 ~ v1.5 lock 유지.

#### Phase 10.3 — test 갱신 + spec 마감 (3 task)

- [ ] **K** (deps: J): unit test v1.6 시나리오 (`commands/scan/output.rs::tests` 또는 `summary_view.rs::tests`): (1) summary-only + failed (hash_io) → entry 3 field `path + presence + failed_reason: "hash_io"` (v1.5 의 2 field 에서 변경), (2) summary-only + failed (encoding + lfs_pointer + 다른 reason) → 동일 3 field 유지, (3) FailedReason::HashIo serde rename `"hash_io"` 정합, (4) `commands/scan/status_filter.rs` 기존 test 영향 (hash_io 처리 위치 분리 — summary-only filter override 시 hash_io 정상 등장 시나리오 추가).
- [ ] **L** (deps: K): integration test 갱신 — `tests/scan_summary_only_failed.rs` (Phase 9 task O 결과 fixture, long_path 함정) 본문에 hash_io 발생 fixture 추가 (file IO 실패 시뮬레이션: e.g., 권한 0 file, EOF 트리거, 또는 mock IO error). 또는 신규 `tests/scan_summary_only_hash_io.rs` 분리. `--summary-only` 호출 + JSON parse + `files[]` entry 에 `failed_reason: "hash_io"` 명시 검증.
- [ ] **M** (deps: L): spec § 마감 점검 (scope 명시) — (i) spec-output-schema.md § v1.5 → v1.6 변경 § 본문 vs task K~L 결과 정합 (hash_io entry 3-field shape, `failed_reason: "hash_io"` serde rename, backward-compat 표 v1.0~v1.5 정확), (ii) § minimal entry shape 발산 강조 줄 위치 (§ `--summary-only` 출력 본문 안 또는 § v1.5 → v1.6 변경 § 안 — task C 결과와 정합), (iii) `[AUTO]` acceptance 시나리오 5건 (task D) wording 정확 + 시나리오 4번 (v1.5 caller × v1.6 hash_io entry parse) 본문 명시, (iv) spec-error-contracts.md FailedReason enum 11 cover 정확 (HashIo variant 추가 후 enum value list + 의미 표 갱신), (v) spec-classification.md § Failed status § FailedReason 인용 부분 dead reference 0. doc-only 검증.

#### Phase 10.4 — release (3 task)

- [ ] **N** (deps: M): README.md 갱신 (scope 명시) — (i) `### scan` 섹션 `--summary-only` 동작 정밀화 — hash_io entry 도 `failed_reason: "hash_io"` 명시 포함, v1.5 의 2 field special case 제거 (1 줄), (ii) `### scan` JSON 예시 본문에 summary-only failed entry 3 field shape 정확 (hash_io 케이스 포함, 가능하면), (iii) v0.5.0 README 본문에 cli-ux-feedback.md reference 있다면 정리 (Phase 9 진입 직전 정리 범위 외 잔존 가능성 — grep `cli-ux-feedback` 검사). 직접 영향 1~3줄. doc-only.
- [ ] **O** (deps: N): CHANGELOG.md v0.6.0 entry finalize — Added (Finding 1 SemVer 면제 근거 + Finding 2 hash_io explicit emit + Finding 3 minimal entry shape 발산 강조), Changed (schema v1.5 → v1.6, FailedReason `None` 특수 케이스 → `HashIo` variant), Spec (spec-output-schema.md § v1.6 변경 + § v1.6 신규 Acceptance Criteria + spec-error-contracts.md FailedReason 갱신), Verified (unit + integration test pass + clean-context audit Finding 1/2/3 해소). v0.5.0 entry 패턴 mirror.
- [ ] **P** (deps: O): v0.6.0 release tag — `git tag v0.6.0 -m "..." && git push origin main && git push origin v0.6.0`. clean-context audit (post-v0.5.0) 외부 시각 source + plan audit chain 사전 진행 — release 전 별도 audit 추가 안 함. v0.5.0 tag 스타일 mirror (annotated tag + main/tag push). **주의**: Phase 9 task R 에서 ralph 가 `git push origin main` 빠뜨린 패턴 발견 — 본 task P 는 tag push + main push 두 단계 명시적 수행 (`git push origin main` 먼저 + `git tag v0.6.0 -m "..."` + `git push origin v0.6.0` 순서).

#### Phase 10.5 — plan close (1 task)

- [ ] **Q** (deps: P): 본 plan Phase 10 task 모두 [x] mark + § 갱신 — Active Phase 비움. Completed Phases § 에 Phase 10 한 줄 요약 추가 (기존 Phase 1~9 압축 history 합류). 자세한 task별 결과는 git history (`git log --grep="Phase 10"`) + commit message + CHANGELOG.md v0.6.0 entry 로 cover.

## Completed Phases

Phase 1~9 + v0.4.1 / v0.4.2 누적 history — 자세한 내용은 git log + CHANGELOG.md 참조. 핵심 마일스톤:

- Phase 5 (2026-05-09~10) — 도메인 함정 8 핵심 + 4 추가 detect/handle + schema v1.0→1.1.
- Phase 6 (2026-05-09~10) — Hard gate 활성화 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출).
- Phase 7 (2026-05-10) — vault scale + Trees sub-tree fallback + 큰 파일 임계 + schema v1.1→1.2 + v0.3.0.
- Phase 8 (2026-05-10) — LLM-as-caller eval 7 friction 해소 (F1/F2 schema v1.2→1.3 + F3 diff --json + F4~F6 clap surface + F7 CI README sanity) + v0.4.0.
- v0.4.1 (2026-05-10) — clap argument-parse contract 회복 (try_parse + CONFIG_ERROR JSON wrap).
- v0.4.2 (2026-05-11) — cosmetic identical classification fix (normalize-equal sha-differ → Identical) + schema v1.3→1.4 (ADR 0015).
- Phase 9 (2026-05-12) — vault dogfood F1/F2/F3 (scan/diff about derive + init wording 정밀화 + summary-only failed visibility) + schema v1.4→1.5 + v0.5.0.

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner.
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **자율 진행 회피 영역** — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 외부 시각 검토 권장.
