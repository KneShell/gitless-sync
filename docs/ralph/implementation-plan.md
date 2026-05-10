# Implementation Plan

## Status
- Last updated: 2026-05-10 (Phase 7.7 task II — 8 stale `*.rs` ref 일괄 fix across 4 spec files: spec-error-contracts.md (1 row, `pipeline/hash_pass::build_one_pre_entry`), spec-hash-and-normalize.md (5 hits → `pipeline/short_circuit`/`pipeline::assemble_entries`/`pipeline/hash_pass::build_pre_entries`/`shared/gitattributes/` ×2), spec-domain-pitfalls.md (`shared/gitattributes/`), spec-classification.md (`shared/github/trees/classify.rs::to_nfc`). Phase 5.13.1 module-folder split aftermath stale path 일괄 정정. doc-only, spec semantics 변경 0. JJ에서 deterministic grep 0 hit 검증 → CONVERGE PASS → X tag.)
- Total tasks: 96
- Completed: 93 / 96

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

- [x] **A**: `shared/github/trees/parse.rs::TreeEntry` struct에 `size: Option<u64>` field 추가 (Trees response size field 활용). spec-github-api.md § fetch_tree 정합. unit test 갱신 (size field 파싱).
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
- [x] **M**: `shared/github/blobs.rs::fetch_blob_with_size_gate` 신규 — Trees response size field pre-flight + 임계치 분기. spec-hash-and-normalize.md § fetch_blob_with_size_gate 정합.
- [x] **N**: `commands/scan/hash_remote.rs` update — Trees entry size field 전달 (caller plumbing). pre-flight skip 시 fetch_blob 호출 0회 검증.
- [x] **O**: unit test 4 시나리오 — 49MB local (정상 hash) + 51MB local (memory_exceeded) + 101MB local (file_too_large) + 30MB LFS pointer (LFS 우선순위). fixture file `tests/fixtures/large-files/`.
- [x] **P**: `output.rs::SCHEMA_VERSION` "1.1" → "1.2" + lock test 갱신 (v1.0/v1.1 backward-compat 검증). spec-output-schema.md § v1.2 신규 Acceptance Criteria 정합.
- [x] **Q**: spec-output-schema.md § v1.2 신규 Acceptance Criteria 7 시나리오 unit test (`output.rs::tests`). schema_version "1.2" + size_bytes field 정확 직렬화 + omit 검증.
- [x] **R**: CHANGELOG.md `[Unreleased]` → v0.3.0 prep entry — schema v1.2 + 2 reason + size_bytes field 포함 prep section.

#### Phase 7.3 — vault scale 1000+ dogfood (5 task)

- [x] **S**: `xtask/src/synth_vault.rs` 신규 sub-command — seed/UTF-8 NFC/LF/mtime epoch/markdown 1000+ 정책 정합. spec-domain-pitfalls.md § Phase 7 — 합성 vault generator 정합. unit test (generate 후 NFC/LF/mtime 검증).
- [x] **T**: 합성 vault generate + scan 측정 — `cargo xtask synth-vault --out tmp/synth-vault-42` + `cargo run -- scan --local tmp/synth-vault-42 --repo {public-test-repo}` 실행. 결과 raw data `docs/research/phase7-vault-scale-bench.md` 신규.
- [x] **U**: public repo cross-check sanity (manual) — linux/torvalds 또는 동등 1000+ entry repo. commit sha 박제. 결과 phase7-vault-scale-bench.md § public 추가.
- [x] **V** (skipped: vault scale instrumentation 부재): mtime cache 재도입 트리거 검토 (ADR 0008 § Future work) — 1000+ scale 측정 결과 hash 비중 ↑ 시 cache 재도입 정당성 검토. 결과 ADR (cache 재도입 OR keep-drop confirmed). T/U raw bench data가 walltime 전체만 측정 (hash phase 별도 instrumentation 부재) → "측정 결과 surface 안 하면 task skip 표시" 트리거 충족. ADR 0008 § Phase 7.3 재검토 추가 (path scale 20× 증가에도 walltime 1324.8 ms → 829/1109 ms로 hash 비중 폭증 신호 없음 보강) + keep-drop 유지 박제. 측정 결과 surface 임계는 별도 instrumentation task 도입 시점 — Phase 7 scope 외 (yagni 일관).
- [x] **W**: Phase 7 종합 measurements `docs/research/phase7-vault-scale-bench.md` 완성 + CHANGELOG.md v0.3.0 entry vault dogfood 결과 추가.

#### Phase 7.4 — release tag (3 task)

> Task 순서 swap (2026-05-10): Y(CHANGELOG finalize) → X(tag) → Z(plan 갱신). semver release 정합 — `[Unreleased]` → `[0.3.0]` heading commit 후 그 commit 위에 v0.3.0 tag 박힘. plan 본문 task ID는 보존, position만 swap.

- [x] **Y**: CHANGELOG.md v0.3.0 entry finalize — Added (sub-tree fallback / file_too_large / memory_exceeded / size_bytes / schema v1.2 / 합성 vault generator) + Changed (G-002 obsolete) + Verified (vault dogfood) + Known limitations.
- [ ] **X** (deps: Phase 7.7 JJ): v0.3.0 release tag — `git tag v0.3.0 -m "..." && git push origin v0.3.0`. 사전 deterministic grep 검증 + 0 stale ref CONVERGE PASS 확인. (2026-05-10 [~]→[ ] revert: 첫 audit 3 finding → Phase 7.5 chain depth 2 진입, fix + DD audit 결과 3 신규 finding → Phase 7.6 chain depth 3 진입, fix + HH audit 결과 grep cross-check로 8 stale ref surface → Phase 7.7 chain depth 3/3 deterministic grep 검증으로 전환. JJ CONVERGE PASS 후 X 자연 진행. Tag message + main push 패턴은 v0.2.1 정합 (origin/main..v0.2.1 ancestor=0 검증).)
- [ ] **Z** (deps: X): 본 plan Phase 7 task 모두 [x] mark + § 갱신 (Active → Completed Phases).

#### Phase 7.5 — clean-context audit finding fix (4 task)

> Phase 7.4 Y(CHANGELOG finalize) 직후 sub-claude clean-context audit 결과 3 finding (struct name typo + ADR cross-ref typo + spec literal swap, 모두 doc-only, spec semantics 변경 X). 자동 신규 phase chain 진입 — memory `feedback_release_phase_chain.md` (긴 자율 루프 + 0 finding 수렴까지) + G-019 정합. Chain depth 2/3, token loose (≪ 200k), wall-clock loose (≪ 6h).

- [x] **AA**: CHANGELOG.md `ResponseEntry::size` → `TreeEntry::size` typo fix (실제 struct 이름 정합). Files: CHANGELOG.md.
- [x] **BB** (deps: AA): guardrails.md G-019 § "cap 변경: ADR 0014 갱신 동반" → "ADR 0013 갱신 동반" cross-ref typo fix (자율 chain hard cap 결정 ADR 본은 0013, 역방향 cross-ref는 정합). Files: docs/ralph/guardrails.md.
- [x] **CC** (deps: BB): spec-hash-and-normalize.md § Phase 7 우선순위 cascade spec literal swap — spec 본문 (1=case_collision, 2=nfd_collision)을 코드 `short_circuit.rs::try_short_circuit_failed` dispatch 순서 (1=nfd_collision, 2=case_collision) + module doc 정합으로 swap (두 collision mutually exclusive로 실동작 영향 0, literal 정합 only). Files: docs/specs/spec-hash-and-normalize.md.
- [x] **DD** (deps: CC): clean-context audit re-run + 0 finding CONVERGE PASS 검증. 0 finding이면 X dep 해소 mark (X 본문 "(deps: Phase 7.5 DD)" 문구 제거). ≥1 finding이면 G-019 수렴 기준 ("동일 finding 2회 연속 + 신규 0건") 또는 cap 적용. Files: docs/ralph/implementation-plan.md. **결과**: 3 신규 finding 검출 (이전 3건과 다른 신규, doc-only stale literal/path/sig). G-019 수렴 미충족 → Phase 7.6 chain depth 3 진입.

#### Phase 7.6 — clean-context audit re-run finding fix (4 task)

> Phase 7.5 DD(audit re-run) 결과 3 신규 finding 검출 (모두 doc-only stale literal/path/sig, spec semantics 변경 X). 이전 3건과 다른 신규라 G-019 수렴 기준 ("동일 finding 2회 연속 + 신규 0건") 미충족 → 자동 신규 phase chain 진입. Chain depth 3/3 (cap 직전, HH에서 또 신규 finding 발생 시 escape hatch 적용 — BLOCK + 사용자 wake-up surface). Token loose (≪ 200k), wall-clock loose (≪ 6h).

- [x] **EE**: implementation-plan.md:36 task A 설명 `shared/github/trees/parse.rs::ResponseEntry` → `TreeEntry` typo fix (실제 struct 이름 정합, AA가 CHANGELOG에서 fix한 동일 typo 누락분; 라인 78 AA task 설명의 `ResponseEntry::size` → `TreeEntry::size` 표기는 typo fix 자체를 documenting 의도라 보존). Files: docs/ralph/implementation-plan.md.
- [x] **FF** (deps: EE): spec-error-contracts.md Per-file Pitfall Reasons 표 5 row `pipeline.rs::try_short_circuit_failed line N~N` → `pipeline/short_circuit.rs::try_short_circuit_failed` (Phase 5.13.1 module 폴더 분할 후 `pipeline.rs` 단일 파일 부재 + 라인 넘버 drift 회피 위해 라인 제거). 영향 row: submodule(line 158) / symlink(159) / lfs_pointer(160) / long_path(161) / case_collision(163). Files: docs/specs/spec-error-contracts.md.
- [x] **GG** (deps: FF): spec-domain-pitfalls.md:80-86 § lifetime 계약 `prepare_for_hash` 시그니처 3-arg → 4-arg (`path: &str` 4번째 인자 추가). normalize.rs:55-60 실 시그니처 + spec-hash-and-normalize.md:103-109 authoritative spec과 정합. Files: docs/specs/spec-domain-pitfalls.md.
- [x] **HH** (deps: GG): clean-context audit re-run 2nd round + CONVERGE PASS 검증. **결과**: LLM auditor 3 신규 finding 보고 (모두 stale `pipeline.rs::*` ref), grep cross-check로 추가 5건 surface (auditor long-line miss + 다른 split aftermath miss). 총 8 stale ref — 4 `pipeline.rs` (spec-error-contracts.md:156, spec-hash-and-normalize.md:15/114/257) + 3 `gitattributes.rs` (spec-domain-pitfalls.md:73, spec-hash-and-normalize.md:13/90) + 1 `trees.rs` (spec-classification.md:10). 모두 Phase 5.13.1 module-folder split aftermath stale path, doc-only, spec semantics 변경 X. G-019 0 finding 미충족 → Phase 7.7 chain depth 3/3 진입. **Phase 7.7은 LLM audit 대체 deterministic grep 검증** — sampling blind spot 차단 (advisor 권고). Files: docs/ralph/implementation-plan.md.

#### Phase 7.7 — Phase 5.13.1 split aftermath stale path comprehensive fix (2 task)

> Phase 7.6 HH(audit) 결과 LLM auditor 3 finding 보고 + grep cross-check 추가 5건 surface (총 8 stale ref). 패턴: 매 audit round마다 다른 finding (sampling blind spot — long line miss / 다른 split aftermath miss). Phase 7.5/7.6는 LLM auditor 결과 list 기반 narrow scope fix → 다음 round에서 다른 ref 노출 cycle. **본 phase는 deterministic grep으로 cycle 차단** (advisor 권고): II에서 grep으로 잡힌 모든 stale ref 일괄 fix + JJ에서 grep return 0 검증. Chain depth 3/3 (cap). JJ에서 grep return 0이면 CONVERGE PASS → X. ≥1 stale ref 잔존이면 escape hatch BLOCK + 다음 세션 wake-up 시 사용자 surface (G-019 cap 초과). Token loose (≪ 200k), wall-clock loose (≪ 6h).
>
> Phase 5.13.1 split target 박제 (grep 검증 패턴 base): `pipeline.rs` → `pipeline/{mod, orchestrator, short_circuit, finalize, hash_pass}` (commit `09bd5e6`), `shared/github/trees.rs` → `shared/github/trees/{mod, parse, classify, fetch, fallback}` (`445f1ec`), `shared/gitattributes.rs` → `shared/gitattributes/{mod, parser, classify, matching}` (Phase 5.13 Z `07aa888`). 향후 추가 split 발생 시 본 phase grep 패턴 갱신 — 본 plan에 base 박제.

- [x] **II** (deps: HH): 8 stale `*.rs` ref 일괄 fix (grep `\b(pipeline|gitattributes|github/trees)\.rs` `docs/specs/` 기반). 매핑:
  - spec-error-contracts.md:156 `pipeline.rs::build_one_pre_entry` line 122~128 → `pipeline/hash_pass::build_one_pre_entry` (line range drop, FF style)
  - spec-hash-and-normalize.md:15 `pipeline.rs::try_short_circuit_failed` → `pipeline/short_circuit::try_short_circuit_failed`
  - spec-hash-and-normalize.md:114 `commands/scan/pipeline.rs::assemble_entries` → `commands/scan/pipeline::assemble_entries` (module path, 향후 internal split survive)
  - spec-hash-and-normalize.md:257 `commands/scan/pipeline.rs는 sequential` → `commands/scan/pipeline/hash_pass::build_pre_entries는 sequential`
  - spec-domain-pitfalls.md:73 `shared/gitattributes.rs` 구현 → `shared/gitattributes/` 구현
  - spec-hash-and-normalize.md:13 `shared/gitattributes.rs` 신규 → `shared/gitattributes/` 신규
  - spec-hash-and-normalize.md:90 `shared/gitattributes.rs` 구현 → `shared/gitattributes/` 구현
  - spec-classification.md:10 `shared/github/trees.rs` (line 63/75/87, remote 3 mode) → `shared/github/trees/classify.rs::to_nfc` (line range drop)
  - Files: docs/specs/spec-error-contracts.md, docs/specs/spec-hash-and-normalize.md, docs/specs/spec-domain-pitfalls.md, docs/specs/spec-classification.md.
- [~] **JJ** (deps: II): deterministic grep 검증 — `grep -rn '\b(pipeline|gitattributes|github/trees)\.rs' docs/specs/` 결과 0 hit이면 CONVERGE PASS mark (X dep 해소 — X 본문 "(deps: Phase 7.7 JJ)" 문구 제거 + Phase 7 task 모두 [x] mark + § 갱신 Active → Completed Phases). ≥1 hit이면 II 재실행 필요 — grep output을 본 task 결과 noting + escape hatch BLOCK ([!]) + 사용자 wake-up surface (chain depth 3/3 cap 도달, G-019 escape hatch). Files: docs/ralph/implementation-plan.md.

## Pending Phases (v0.4+)

(Phase 7 종료 후 sub-claude clean-context finding 발생 시 자동 신규 phase plan/spec — memory `feedback_release_phase_chain.md` 정합)

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner (Phase 6.1 WW).
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **박제 expiration** — Phase 진입마다 재검토 (CLAUDE.md § 박제 정책).
- **자율 진행 회피 영역** (사용자 vague 답변, Phase 5.13.1/5.14 패턴) — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 vague + clean-context 외부 시각 보강 필수.
