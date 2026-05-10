# Implementation Plan

## Status
- Last updated: 2026-05-10 (Phase 7.2 task L 완료 — `try_short_circuit_failed` 모듈 doc에 cascade priority list 1-9 박제: items 1-7 path-string cascade 그대로, items 8-9 `file_too_large`/`memory_exceeded` post-read in `try_hash_local::try_size_gate` (task K) + item 9 `Encoding` post-read in `try_hash_local` (task FF). cascade가 `Some` 반환 시 8-9 lock-out — byte/size-blind cascade의 priority 정합 spec-hash-and-normalize.md § 우선순위. 기존 LFS lock-out test rename + docstring 갱신으로 size_gate priority 명시.)
- Total tasks: 86
- Completed: 72 / 86

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

## Active Phase

### Phase 7 — vault scale + Trees sub-tree + 큰 파일 임계치 (통합, v0.3.0)

3 후보 한 phase 통합 (사용자 결정 2026-05-10). 사전 vague + clean-context 외부 시각 보강 완료 (`docs/research/phase7-vague.md`). spec 본문 100% 사전 확정 (spec-github-api.md § Trees truncation handling + spec-hash-and-normalize.md § Phase 7 — 큰 파일 처리 + spec-output-schema.md § v1.2 + spec-error-contracts.md + spec-domain-pitfalls.md + ADR 0011/0012/0013 + guardrails.md G-019). ralph 가동 시 spec 변경 0건 (자율 주행). 사용자 stance: 무한 chain 자율 주행 + 과설계 회피 (memory `feedback_release_phase_chain.md` + `feedback_quality_vs_complexity.md`).

진행 순서: 7.1 Trees → 7.2 큰 파일 → 7.3 vault → 7.4 release.

#### Phase 7.1 — Trees sub-tree 재귀 fallback (8 task)

- [x] **A**: `shared/github/trees/parse.rs::ResponseEntry` struct에 `size: Option<u64>` field 추가 (Trees response size field 활용). spec-github-api.md § fetch_tree 정합. unit test 갱신 (size field 파싱).
- [x] **B**: `shared/github/trees/fallback.rs` 신규 module — `Budget` struct + 2 cap 상수 (`MAX_TREE_CALL_BUDGET = 1000` + `MAX_TREE_ENTRIES = 500_000`). spec-github-api.md § Trees truncation handling § 한도 상수 정합.
- [x] **C**: `shared/github/trees/fallback.rs::resolve_root_tree_sha` — ref → commit sha → root tree sha 1회 resolve. 2회 gh api 호출 (`refs/heads/{branch}` + `commits/{commit_sha}`). unit test mock fixture.
- [x] **D**: `shared/github/trees/fallback.rs::fetch_subtree_recursive` — sub-tree non-recursive 재귀 알고리즘. 2 cap check + early-abort. spec-github-api.md § sub-tree 재귀 알고리즘 정합.
- [x] **E**: `shared/github/trees/mod.rs::fetch_tree_with_fallback` — 1차 truncated 검출 → fallback 진입 entry point. 정상 path는 v0.2.x 동작 유지.
- [x] **F**: unit test 2 시나리오 — call budget 1001 (mock fixture 1001번째 호출 trigger) + entries 500_001 (누적 cap trigger). 둘 다 `GitlessError::TreesTruncated` 검증.
- [x] **G**: integration test — 합성 truncated mock fixture (Trees response `truncated:true` → fallback 진입 → sub-tree 정상 응답 → 합산 entries 반환). `tests/scan_trees_fallback.rs` 신규.
- [x] **H**: G-002 본문 update + spec-github-api.md § Trees truncation handling cross-ref 정합 자체 검증. doc-only.

#### Phase 7.2 — 큰 파일 임계치 (file_too_large + memory_exceeded) (10 task)

- [x] **I**: `compare.rs::FailedReason` enum에 2 variant 추가 — `FileTooLarge` + `MemoryExceeded`. Display impl 갱신.
- [x] **J**: `compare.rs::FileEntry` struct에 `size_bytes: Option<u64>` field 추가 — `#[serde(skip_serializing_if = "Option::is_none")]`. spec-output-schema.md § v1.2 정합.
- [x] **K**: `commands/scan/hash_local.rs::try_hash_local` size pre-flight 추가 — `fs::metadata().len()` 측정 + 100MB/50MB 분기. spec-hash-and-normalize.md § 검출 알고리즘 정합.
- [x] **L**: `commands/scan/pipeline/short_circuit.rs::try_short_circuit_failed` cascade에 `file_too_large` + `memory_exceeded` 분기 추가 (LFS 다음 우선순위). spec-hash-and-normalize.md § 우선순위 정합.
- [ ] **M**: `shared/github/blobs.rs::fetch_blob_with_size_gate` 신규 — Trees response size field pre-flight + 임계치 분기. spec-hash-and-normalize.md § fetch_blob_with_size_gate 정합.
- [ ] **N**: `commands/scan/hash_remote.rs` update — Trees entry size field 전달 (caller plumbing). pre-flight skip 시 fetch_blob 호출 0회 검증.
- [ ] **O**: unit test 4 시나리오 — 49MB local (정상 hash) + 51MB local (memory_exceeded) + 101MB local (file_too_large) + 30MB LFS pointer (LFS 우선순위). fixture file `tests/fixtures/large-files/`.
- [ ] **P**: `output.rs::SCHEMA_VERSION` "1.1" → "1.2" + lock test 갱신 (v1.0/v1.1 backward-compat 검증). spec-output-schema.md § v1.2 신규 Acceptance Criteria 정합.
- [ ] **Q**: spec-output-schema.md § v1.2 신규 Acceptance Criteria 7 시나리오 unit test (`output.rs::tests`). schema_version "1.2" + size_bytes field 정확 직렬화 + omit 검증.
- [ ] **R**: CHANGELOG.md `[Unreleased]` → v0.3.0 prep entry — schema v1.2 + 2 reason + size_bytes field 포함 prep section.

#### Phase 7.3 — vault scale 1000+ dogfood (5 task)

- [ ] **S**: `xtask/src/synth_vault.rs` 신규 sub-command — seed/UTF-8 NFC/LF/mtime epoch/markdown 1000+ 정책 정합. spec-domain-pitfalls.md § Phase 7 — 합성 vault generator 정합. unit test (generate 후 NFC/LF/mtime 검증).
- [ ] **T**: 합성 vault generate + scan 측정 — `cargo xtask synth-vault --out tmp/synth-vault-42` + `cargo run -- scan --local tmp/synth-vault-42 --repo {public-test-repo}` 실행. 결과 raw data `docs/research/phase7-vault-scale-bench.md` 신규.
- [ ] **U**: public repo cross-check sanity (manual) — linux/torvalds 또는 동등 1000+ entry repo. commit sha 박제. 결과 phase7-vault-scale-bench.md § public 추가.
- [ ] **V**: mtime cache 재도입 트리거 검토 (ADR 0008 § Future work) — 1000+ scale 측정 결과 hash 비중 ↑ 시 cache 재도입 정당성 검토. 결과 ADR (cache 재도입 OR keep-drop confirmed). 측정 결과 surface 안 하면 task skip 표시.
- [ ] **W**: Phase 7 종합 measurements `docs/research/phase7-vault-scale-bench.md` 완성 + CHANGELOG.md v0.3.0 entry vault dogfood 결과 추가.

#### Phase 7.4 — release tag (3 task)

- [ ] **X**: v0.3.0 release tag — `git tag v0.3.0 -m "..." && git push origin v0.3.0`. 사전 sub-claude clean-context 검증 + 0 finding CONVERGE PASS 확인.
- [ ] **Y**: CHANGELOG.md v0.3.0 entry finalize — Added (sub-tree fallback / file_too_large / memory_exceeded / size_bytes / schema v1.2 / 합성 vault generator) + Changed (G-002 obsolete) + Verified (vault dogfood) + Known limitations.
- [ ] **Z**: 본 plan Phase 7 task 모두 [x] mark + § 갱신 (Active → Completed Phases).

## Pending Phases (v0.4+)

(Phase 7 종료 후 sub-claude clean-context finding 발생 시 자동 신규 phase plan/spec — memory `feedback_release_phase_chain.md` 정합)

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner (Phase 6.1 WW).
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **박제 expiration** — Phase 진입마다 재검토 (CLAUDE.md § 박제 정책).
- **자율 진행 회피 영역** (사용자 vague 답변, Phase 5.13.1/5.14 패턴) — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 vague + clean-context 외부 시각 보강 필수.
