# Implementation Plan

## Status
- Last updated: 2026-05-09 (Phase 5 진입 — 8 도메인 함정 + clean-context 보강 12 task 박힘)
- Total tasks: 34
- Completed: 33 / 34

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Phase 6 hard gate 모두 deny active 유지 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출). 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (project-ops.md). Phase 5 새 코드 cover 정책 — 신규 task의 acceptance에 unit test 포함.

## Tasks (Phase 5 — 도메인 함정 정리)

### Phase 5.1 — vault 운영 데이터 분석 + fact check (우선순위 입력)

- [x] **A. vault scan 재실행 + drift 근원 분석 + 검증 필요 fact check**
  - acceptance: KneShell/gitless-sync 또는 사용자 vault repo (356+ files)에 대해 `cargo run -- scan` 재실행. drift/failed/local_only_changed/remote_only_changed 분류. 각 drift entry에 대해 함정 (NFD/case/encoding/submodule/symlink/empty/permission/.gitattributes/BOM/LFS/long_path/.gitignore) 중 어느 것이 원인인지 분석. **검증 필요 fact check sub-step**: encoding_rs binary size (cargo-bloat), git core NUL byte heuristic 정확 N (git source), Windows NTFS NFD 파일 생성 검증 (실험). `docs/research/phase5-vault-baseline.md` 박음.
  - spec: 없음 (research artifact).

- [x] **B. 우선순위 박음 — vault 데이터 기반**
  - acceptance: vault 분석 결과로 함정 우선순위 박음. spec-domain-pitfalls.md § "v0.1 baseline 영향"에 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md`.

- [x] **A2. `.gitignore` 무시 정책 spec 명시**
  - acceptance: spec-classification.md § `.gitignore` 무시 정책 박혀있음 (이미 Phase 5 spec 갱신에서 박힘). spec-ignore-policy.md 갱신해 scan 범위 명시. unit test 박음 — `target/`, `node_modules/`, custom `.gitignore` ignored path 검증.
  - spec: `docs/specs/spec-classification.md` § `.gitignore` 무시 정책 + `docs/specs/spec-ignore-policy.md`.

### Phase 5.2 — path 정규화 함정 (NFD / case)

- [x] **C. NFD → NFC path 정규화**
  - acceptance: `walker.rs`에서 local file path를 NFC로 정규화 (`unicode-normalization` crate). remote tree path도 NFC 정규화. 비교 key는 NFC. unit test (raw bytes injection으로 NFD 가짜 fixture 박음).
  - spec: `docs/specs/spec-domain-pitfalls.md` § Path 정규화 + `spec-classification.md`.

- [x] **D. 대소문자 충돌 처리 정책 박음**
  - acceptance: 같은 path key가 두 case (`README.md` vs `Readme.md`)로 박힐 때 case-sensitive 비교 정책 박음. integration test fixture (mock Trees API + walker).
  - spec: `docs/specs/spec-domain-pitfalls.md` § Path 정규화 + `spec-classification.md`.

- [x] **D1. Windows NTFS case collision local-side detection**
  - acceptance: walker가 NTFS local에서 case-collision 박힌 directory 박을 때 (예: `Foo.txt` + `foo.txt` 같은 directory에 박힌 환경) 두 entry 모두 catch + case-sensitive 비교로 정합. NTFS volume이 case-insensitive로 1개만 catch한 case도 detect — `Status::Failed` + `failed_reason: "case_collision"` 박음. unit test (mock filesystem).
  - spec: `docs/specs/spec-domain-pitfalls.md` § Windows NTFS local-side case detection.

### Phase 5.3 — encoding 변환 시도 (hash 입력 (b) 정책)

- [x] **E. 인코딩 라이브러리 조사 + 채택**
  - acceptance: `encoding_rs` (Mozilla, Recommended) vs `chardet` 평가. UTF-8 → 다른 인코딩 detect 정확도 + Rust ecosystem 정합 + license. `docs/research/encoding-library-eval.md` 박음. 결정 박음.
  - spec: 없음 (research).

- [x] **F. 비-UTF-8 인코딩 변환 시도 박음 (hash 입력 (b))**
  - acceptance: `normalize.rs`에 `try_decode_text` 함수 박음. 1차 UTF-8 디코드 시도 → 2차 다른 인코딩 detect → 3차 binary 취급 (`Status::Failed` + `failed_reason: "encoding"`). **hash 입력은 항상 원본 raw bytes** (clean-context (b) 정책). detect는 reason 마크 + JSON 출력 정보만. unit test (EUC-KR / Shift_JIS / Latin-1 fixture + identical raw bytes 검증). tarpaulin 80% 유지 — encoding detect 분기 cover.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Encoding (hash 입력 (b)) + `spec-hash-and-normalize.md`.

- [x] **F1. BOM 처리 정책 박음 (UTF-8 + UTF-16)**
  - acceptance: UTF-8 BOM은 v0.1 그대로 처리 (text=auto + 미명시에서 strip). UTF-16 BOM (`FF FE` LE / `FE FF` BE) detect 시 `Status::Failed` + `failed_reason: "encoding"`. unit test (UTF-8 BOM strip + UTF-16 BOM detect).
  - spec: `docs/specs/spec-hash-and-normalize.md` § BOM + `spec-domain-pitfalls.md`.

### Phase 5.4 — submodule / symlink / LFS pointer / Windows long path

- [x] **G. submodule (`160000`) detect-only**
  - acceptance: `github.rs::trees`에서 submodule entry skip 대신 `RemoteFile`에 mode 박음. `compare.rs`에서 submodule path → `Status::Failed` + `failed_reason: "submodule"`. JSON 출력에 mode bit (`160000`) 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Submodule + `spec-classification.md`.

- [x] **H. symlink (`120000`) detect-only**
  - acceptance: `github.rs::trees`에서 symlink entry mode 박음. walker가 local symlink 발견 시 `Status::Failed` + `failed_reason: "symlink"`. JSON 출력에 mode bit (`120000`) 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Symlink + `spec-classification.md`.

- [x] **G1. git LFS pointer detection (.gitattributes filter=lfs 신호 기반)**
  - acceptance: scan은 **blob fetch 안 함** (Phase 4 GraphQL batching 이득 유지) — `.gitattributes` 파싱 시점에 `filter=lfs` 매칭된 path는 자동 `Status::Failed` + `failed_reason: "lfs_pointer"` + `lfs_pointer: {oid: "?", size: 0}` (oid/size unknown). diff 명령은 blob fetch 박혀있어 첫 줄 시그니처 `version https://git-lfs.github.com/spec/v1` 검증 + oid/size 정확 파싱 (defence-in-depth). **K1.5 LfsPointer variant 의존**. unit test (LFS pointer fixture + filter=lfs `.gitattributes` fixture). tarpaulin 80% 유지 — 신규 코드 cover 책임.
  - spec: `docs/specs/spec-domain-pitfalls.md` § LFS pointer + `spec-error-contracts.md` + `spec-hash-and-normalize.md` § 화이트리스트.

- [x] **R1. Windows long path / 예약 파일명 detect-only**
  - acceptance: walker에서 260자+ path 또는 예약 파일명 (CON/PRN/NUL/AUX/COM1-9/LPT1-9) detect → `Status::Failed` + `failed_reason: "long_path"`. unit test (mock filesystem fixture).
  - spec: `docs/specs/spec-domain-pitfalls.md` § Windows long path + `spec-error-contracts.md`.

### Phase 5.5 — 빈 파일 실파일 검증

- [x] **I. 빈 파일 실파일 fixture + integration test**
  - acceptance: integration test fixture로 실제 0-byte 파일 박음. `blob_hash(&[])` == git empty blob constant 확인. local empty file ↔ remote empty blob → `Status::Identical`.
  - spec: `spec-hash-and-normalize.md` § Acceptance.

### Phase 5.6 — 실행 권한 detect

- [x] **J. mode bit (`100755` vs `100644`) detect-only**
  - acceptance: `RemoteFile`에 mode field 박음 (G/H에서 박힌 같은 field 활용). `compare.rs`에서 mode bit 차이 발견 시 `Status::Identical` 유지 (content 같으면), JSON 출력에 mode 정보 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § 실행 권한 + `spec-output-schema.md`.

### Phase 5.7 — `.gitattributes` 정확 hash 재현 (큰 변경)

- [x] **K1. `.gitattributes` 파서 박음 (working tree 한정)**
  - acceptance: `shared/gitattributes.rs` 박음. project root + 하위 디렉토리의 `.gitattributes` 파일 1회 로드 + glob pattern matching (gitignore-style). 우선순위: 가장 깊은 `.gitattributes` 우선 + line-level 마지막 매칭 winner. `.git/info/attributes` / global 미지원. unit test (multi-level fixture). tarpaulin 80% 유지 — 신규 모듈 cover 책임.
  - spec: `spec-hash-and-normalize.md` § `.gitattributes` 파서 + `spec-config.md` § 위치 정책.

- [x] **K1.5. `.gitattributes` 지원 attribute 화이트리스트 박음 (5 entry, advisor BLOCKING fix)**
  - acceptance: `AttributeMatch` enum 박음 — `TextAuto / Binary / EolLf / EolCrlf / LfsPointer / Unspecified / Unsupported { attribute_name }`. **`filter=lfs`는 LfsPointer variant** (advisor BLOCKING fix — git-lfs 표준 마커). 화이트리스트 외 (`working-tree-encoding`, `ident`, `filter=*` (lfs 외), macro attributes, `crlf` legacy) 매칭 시 `Unsupported` 박음. unit test (5 화이트리스트 + 5 unsupported fixture). tarpaulin 80% 유지.
  - spec: `docs/specs/spec-domain-pitfalls.md` § `.gitattributes` 화이트리스트 + `spec-hash-and-normalize.md` § 화이트리스트 강제 + § LFS pointer.

- [x] **K2. conditional LF normalize + lifetime 계약 박음 (advisor flag 3 — 분기 helper 분리)**
  - acceptance: `prepare_for_hash` 함수 시그니처 변경 — `gitattr: &Arc<GitAttributes>` 인자 추가. **7 분기** (text=auto / binary / eol=lf / eol=crlf / **LfsPointer** / Unspecified / Unsupported) 박음. `Unspecified` default = v0.1 정책. `LfsPointer`/`Unsupported`는 caller가 `Status::Failed` 박음. **분기 helper fn 분리 박음** — `apply_text_auto`, `apply_binary`, `apply_eol_lf`, `apply_eol_crlf`, `apply_unspecified` 5 helper. Phase 6 `cognitive_complexity = 15` deny 회피 (advisor 권고). 모든 caller 갱신. unit test (단일 vault scan 1회 파싱 + N번 호출 reparse 0회 검증). tarpaulin 80% 유지 — 7 분기 모두 cover.
  - spec: `spec-hash-and-normalize.md` § Normalize 규칙 + § Lifetime 계약.

- [x] **K3. binary attribute 정확 적용**
  - acceptance: `.gitattributes`에 `binary` 명시된 file은 NUL byte 휴리스틱 무시 + raw bytes 해시. unit test (NUL byte 0개 binary fixture).
  - spec: `spec-hash-and-normalize.md` § Normalize 규칙.

- [x] **K4. `.gitattributes` 우선순위 정합 검증**
  - acceptance: project root `.gitattributes` < sub-directory `.gitattributes` < line-level pattern 마지막 매칭 winner 정합. unit test로 검증 (3-level fixture).
  - spec: `spec-hash-and-normalize.md` § `.gitattributes` 파서.

- [x] **X. `.gitattributes` parser performance gate**
  - acceptance: 큰 vault 시뮬레이션 (10K+ files × 100+ rules) per-file glob fnmatch P95 측정. baseline 박음. Phase 6 hard gate에 perf regression 임계 박음 (P95 X ms 초과 시 fail).
  - 검증: cargo bench (criterion) + Phase 6 CI gate.
  - spec: 없음 (perf gate, R3 task와 통합 가능).
  - 결과 (2026-05-09): `crates/gitless-sync/benches/gitattributes_match.rs` 박음 (criterion 0.5 default-features off, `harness = false`). 100 rules × 10K paths fixture에서 baseline P50=40.7µs / MEAN=41.9µs / **P95=50.2µs** / P99=62.1µs. `docs/research/phase5-gitattributes-bench.md` 박음. CI 게이트는 R3에서 — 본 task는 record-only (advisor: GitHub windows-latest noise로 절대 임계는 R3에서 ratio 또는 vault-scale 기반으로 박음).

### Phase 5.8 — spec 갱신 cascade

- [x] **L. spec-hash-and-normalize.md `.gitattributes` 박힌 정합 검증**
  - acceptance: spec 본문 + acceptance criteria가 K1~K4 결과와 정합. 기존 PRD 시나리오 5/6/7 통과 + 새 시나리오 박음. (Phase 5 spec 갱신에서 이미 박혔으니 본 task는 implementation 정합 검증.)
  - spec: `spec-hash-and-normalize.md`.
  - 결과 (2026-05-09): K1~K4 + K1.5 구현 vs spec 정합 audit 5 drift 박음 + 수정. (1) `현재 상태` section 박힌 K-task 박힘 marker 박음 (K1/K2/K3/K4/K1.5 박힘 + commands/scan 경로 + decode.rs 경로 박음). (2) `Lifetime 계약` section signature 4 인자 박음 (`path: &str` 추가, 구현 정합). (3) Acceptance criterion `Unsupported → Status::Failed` 매핑 박음 — K1.5 classifier scope만 cover, caller-side `pipeline.rs` 단락 plumbing은 follow-up. (4) `Arc<GitAttributes>` rayon worker 공유 criterion 박음 — sequential `.iter().map()` 현실 + 1000+ path scale에서 활성화 가능 박음. (5) BOM `호출 지점` section path 오류(`shared/normalize.rs` → `shared/decode.rs`) + caller mapping 미구현 명시. validation: cargo fmt clean (spec-only, G-012 적용). 코드 변경 0 — baseline 유지.

- [x] **M. spec-classification.md path 정규화 정합**
  - acceptance: spec 본문에 NFC 정규화 + case-sensitive 정책 박혀있음 (이미 Phase 5 spec 갱신). 4분류 판정 정합 검증.
  - spec: `spec-classification.md`.
  - 결과 (2026-05-09): spec § 판정 로직 의사코드 vs `compare.rs::classify` 구현 정합 (5 status branch: Identical/LocalOnlyChanged/RemoteOnlyChanged/Drift + Failed) 검증 통과 + NFC 정규화 boundary 양쪽 박힘 (`walker.rs::relative_path` line 92 + `shared/github/trees.rs` line 63/75/87 — 3 mode) 검증 통과 + case_collision symmetric detect (canonical/diagonal/local-both 3 시나리오, `case_collision.rs::detect`) 검증 통과. 1 drift surface + fix: § Path 정규화 § edge case의 `nfd_collision`이 spec 박혀있는데 `FailedReason` enum + `pipeline.rs` 매핑 미박힘 — spec-domain-pitfalls.md "99%/1%" hedge 표현 mirror로 § edge case에 hedge 박음 ("Phase 5 후속, task N 박힌 후 implement task로 박음"). § 현재 상태에 audit verification 박음 (NFC 박힘 line + case_collision 박힘 line). **cross-task carryover**: spec-error-contracts.md § Per-file Pitfall Reasons 표 line 162에 `nfd_collision` 박혀있음 — task N에서 동일 drift hit 예상 (enum-spec'd-but-unimplemented align 또는 enum variant 박음 결정은 task N scope). validation: cargo fmt clean (spec-only, G-012 적용). 코드 변경 0 — baseline 유지.

- [x] **N. spec-error-contracts.md 함정별 reason 매핑**
  - acceptance: `failed_reason` enum 9 값 (이미 Phase 5 spec 갱신) 정합 검증. unit test 박음.
  - spec: `spec-error-contracts.md`.
  - 결과 (2026-05-09): spec § Per-file Pitfall Reasons 9 reason vs `compare.rs::FailedReason` 5 variant 정합 audit + 6 drift 박음. (1) § 현재 상태 § N-task audit (2026-05-09) section 신설 — 박힘 (정합) 6건 + 미박힘 (Phase 5 후속, hedge marker) 3건 (`encoding`/`nfd_collision`/`gitattributes_unsupported` enum-spec'd-but-unimplemented align) + Drift surface (`Http` exit code spec 1 vs `error/core.rs::exit_code()` 박힘 3, ureq 시절 잔재 의심, follow-up 박음) + Spec self-consistency fix (acceptance error_code 양식 inconsistency `"CONFIG"`/`"HTTP"` → `"CONFIG_ERROR"`/`"HTTP_ERROR"`, § stderr 출력 형식 § 1:1 매핑 원칙 정합) + Spec 잔재 hedge (`format_graphql_errors` re-export). (2) § Per-file Pitfall Reasons 표에 구현 컬럼 박음 — 박힘 5 variant + 미박힘 3 reason hedge marker + None special case 박음. (3) § Acceptance Criteria 4 line self-consistency fix (CONFIG → CONFIG_ERROR ×2, HTTP → HTTP_ERROR ×3 — gh 미설치 + 5xx fallthrough + GraphQL NOT_FOUND + GraphQL fallthrough). (4) `compare.rs::tests`에 unit test 10건 박음 — `FailedReason` snake_case round-trip 5건 + None skip_serializing 1건 + Some emit 1건 + LfsPointer placeholder shape 2건 + helper. (5) **F task 정합 의심 surface 박음** — F task acceptance "3차 binary 취급 (`Status::Failed` + `failed_reason: "encoding"`)" 박혀있는데 `decode.rs` sniff-only + `pipeline.rs` surface plumbing 미박힘 — 본 task hedge marker 박음, fix는 follow-up task. validation: cargo fmt clean + clippy 0 warnings + xtask check-line-limits (52 files within 300) + xtask check-cycles (0/0) + cargo machete clean + cargo test 286 lib + 25 integration + 49 xtask = **360 tests pass** + tarpaulin **90.34%** (945/1046 lines, +0.00% change). compare.rs LOC 158 → 256 (300 안). **다른 task scope 침범 없음** (`error/core.rs` exit_code 박힘 3 코드 변경 박지 않음, follow-up task 영역).

- [x] **O. spec-output-schema.md mode bit + reason + LFS 필드 검증**
  - acceptance: schema_version `"1.1"` + `mode` + `failed_reason` + `lfs_pointer` 필드 박힘 (이미 Phase 5 spec 갱신). v1.0 호출자 backward-compat 검증 unit test.
  - spec: `spec-output-schema.md`.
  - 결과 (2026-05-09): spec § 현재 상태에 § O-task audit (2026-05-09) section 박음 — 박힘 11건 (정합) + 미박힘 3건 (`encoding` / `nfd_collision` / `gitattributes_unsupported` task N drift mirror, fix scope follow-up) + Spec self-consistency hedge marker 2 line 박음 (§ 안정성 보장 enum 9 reason line 81 + § Acceptance Criteria § v1.1 신규 line 113) — 구현 5 variant + None special case = 6 cover 정합. `output.rs::tests` 신설 — v1.0 backward-compat lock test 5건 박음 (envelope / identical entry / failed-with-lfs entry / mode 박음 + failed_reason/lfs_pointer omit / failed_reason + lfs_pointer placeholder 박음). validation: cargo fmt clean + clippy 0 warnings + xtask check-line-limits (52 files within 300, output.rs 41 → 240) + xtask check-cycles (0/0) + cargo machete clean + cargo test 291 lib + 25 integration + 49 xtask = **365 tests pass** + tarpaulin **90.34%** (945/1046 lines, +0.00% change). 다른 task scope 침범 없음 (`compare.rs::FailedReason` enum 코드 변경 박지 않음, follow-up task 영역).

- [x] **L1. spec-config.md `.gitattributes` 위치 정책 검증**
  - acceptance: spec 본문 박혀있음 (이미 Phase 5 spec 갱신). working tree 한정 + `.git/info/attributes` / global 미지원 정합 검증.
  - spec: `spec-config.md`.
  - 결과 (2026-05-09): spec § `.gitattributes` 위치 정책 (Phase 5) ↔ K1 구현 (`shared/gitattributes.rs`) ↔ caller (`commands/scan/mod.rs::scan` line 93) 정합 audit. (1) § 현재 상태에 § L1-task audit (2026-05-09) section 박음 — 박힘 (정합) 4건 (working tree 한정 / `.git/info/attributes` 미지원 / global 미지원 / macro attributes 화이트리스트) + Drift surface 0건 (advisor BLOCKING fix 박음). (2) § 미지원 § macro attributes line trace 박음 — `[attr]binary`는 gitignore-style glob character class `{a,t,r}` + literal `binary`로 박혀 ignore crate `GitignoreBuilder::add_line` valid pattern 통과 + attributes 토큰은 K1.5 `Unsupported` variant 박음. **advisor BLOCKING fix**: 초기 audit에서 macro attribute graceful skip 미박힘을 drift surface로 박았으나 phantom drift (오류). 검증: glob `[abc]` character class는 POSIX glob 표준 + ignore crate 0.4.x 표준 지원. retract 후 § Drift surface section을 "0건, advisor BLOCKING fix"로 박음. (3) Cross-spec 정합 ✓ — spec-domain-pitfalls.md / spec-hash-and-normalize.md cross-ref line 정합. validation: cargo fmt clean + clippy 0 warnings + xtask check-line-limits (52 files within 300) + xtask check-cycles (0/0) + cargo machete clean + cargo test pass (코드 변경 0). 코드 변경 0 — baseline 유지 (G-012 spec-only 적용).

### Phase 5.9 — 보강 fixture

- [x] **P. NFD raw bytes injection unit test fixture**
  - acceptance: Windows 환경에서 raw bytes injection — compose 한글 (`가` = `\u{AC00}`) vs decompose (`가` = `\u{1100}\u{1161}`) 둘 다 시도. `walker.rs` + 비교 path key 정합 검증.
  - spec: `docs/specs/spec-domain-pitfalls.md` § 검증 환경.
  - 결과 (2026-05-09): `walker.rs::tests`에 `nfd_and_nfc_synthetic_paths_collapse_to_same_key` 박음 — Hangul (algorithmic LV/LVT composition `\u{AC00}` ≡ `\u{1100}\u{1161}`) + Latin ñ (canonical decomposition table `\u{00F1}` ≡ `n\u{0303}`) 두 케이스 직접 collapse 박음 (advisor flag — Hangul 단일 cover는 `unicode-normalization` 한 코드 경로만 박는 tautology). 박힌 두 NFD/NFC 단일 검증 테스트 (`relative_path_normalizes_nfd_to_nfc` + `relative_path_nfc_input_is_preserved`) 그대로 유지 — 새 테스트는 collapse(=) delta 박음. `Files` scope walker.rs 한정 — remote-side `shared/github/trees.rs` 정합 검증은 R task scope (advisor: P scope 외). validation: cargo fmt clean + clippy 0 warnings + xtask check-line-limits (52 files within 300, walker.rs 290 → 299) + xtask check-cycles (0/0) + cargo machete clean + cargo test 292 lib + 25 integration + 49 xtask = **366 tests pass** (+1) + tarpaulin **90.34%** (945/1046 lines, +0.00% change).

- [x] **P1. NFD NTFS 실파일 fixture (clean-context §5 fact check)**
  - acceptance: NTFS는 normalize 안 함 — NFD/NFC 실파일 직접 생성 가능. `tempfile` 박음 + walker가 정확 NFC 정규화 + NFC 정규화로 동일 key 검증. integration test.
  - spec: `docs/specs/spec-domain-pitfalls.md` § 검증 환경.
  - 결과 (2026-05-09): `crates/gitless-sync/tests/scan_nfd_real_file.rs` 박음 — 3 integration tests (P unit-level synthetic fixture에 실파일 fs round-trip 차원 추가). (1) `local_nfd_hangul_real_file_matches_remote_nfc_blob` — `\u{1100}\u{1161}.txt` (NFD jamo LV) 실파일 + 원격 `\u{AC00}.txt` (NFC) tree → Identical, walker side `to_nfc` (walker.rs:92) 정합 검증. (2) `local_nfd_latin_n_tilde_real_file_matches_remote_nfc_blob` — `n\u{0303}ame.txt` (NFD decomposition table) 실파일 + 원격 `\u{00F1}ame.txt` (NFC) tree → Identical, 알고리즘과 다른 normalization 코드 경로 cover. (3) `local_nfc_real_file_matches_remote_nfd_blob` — symmetric 방향 (NFC 실파일 + NFD remote tree) → Identical, `shared/github/trees.rs::to_nfc` (line 63/75/87) 정합 검증. NFD path는 `\u{}` Rust escape 박은 const + raw format! interpolate 패턴 (advisor 권고). Platform note: NTFS/ext4/APFS는 raw bytes 박음 (NFD 실파일 생성 가능), HFS+은 canonicalize-on-write로 NFD 박혀도 양쪽 NFC 수렴 → 모든 platform 통과. Commits API stub 안 박음 — Identical entries skip Commits API (G-003). validation: cargo fmt clean + clippy 0 warnings + xtask check-line-limits (52 files within 300, scan_nfd_real_file.rs 113) + xtask check-cycles (0/0) + cargo machete clean + cargo test 292 lib + 28 integration + 49 xtask = **369 tests pass** (+3) + tarpaulin **90.34%** (945/1046 lines, +0.00% change).

- [x] **Q. 인코딩 변환 fixture 박음**
  - acceptance: EUC-KR / Shift_JIS / Latin-1 byte literal fixture 박음 + 변환 시나리오 unit test. **hash 입력 raw bytes 정합 검증**.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Encoding.
  - 결과 (2026-05-09): advisor scope 확정 — F-task 박은 EUC-KR / Shift_JIS / Latin-1 fixture (decode.rs:100-133)는 박힘, gap은 (1) `try_decode_text_preserves_raw_bytes_for_hashing`이 EUC-KR 단일만 cover + (2) decode + normalize chain raw bytes invariant 미박힘. 두 gap 박음. (1) 기존 test를 3 인코딩 loop로 확장 — encoding_rs distinct decoder path 명시 (EUC-KR/CP949 stateful · Shift_JIS lead+trail multi-byte · Windows-1252 single-byte table) tautology 회피 (advisor flag, P task 패턴 mirror). (2) `prepare_for_hash_preserves_non_utf8_raw_bytes_via_pipeline` 신규 — `prepare_for_hash` × 3 인코딩 NUL-free non-UTF-8 raw bytes (default policy, `Arc<GitAttributes>::default()`) → unspecified branch `normalize_text` 통과 + LF/CRLF 0이라 output == input == hash input. local + remote 같은 raw bytes → 같은 blob_hash via 실제 scan pipeline 거친 chain. **Files**: `decode.rs::tests` 한정 — advisor 권고 locus normalize.rs였으나 normalize.rs 299 LOC (300 게이트 1줄 여유) 위반 회피로 decode.rs 박음 (이미 cross-module pattern 박힘 — line 287 `utf16_bom_passes_through_unchanged_for_hashing_and_normalize`). **out of scope**: pipeline.rs surface plumbing for `failed_reason: "encoding"` (N task hedge marker 박힘 — F task 정합 의심 surface, follow-up task 영역) + 새 file / sibling `_tests.rs` (Z task 영역). validation: cargo fmt clean + clippy 0 warnings + xtask check-line-limits (52 files within 300, decode.rs 188 → 222) + xtask check-cycles (0/0) + cargo machete clean + cargo test 293 lib + 28 integration + 49 xtask = **370 tests pass** (+1) + tarpaulin **90.34%** (945/1046 lines, +0.00% change).

- [x] **R. submodule/symlink/permission integration fixture**
  - acceptance: `MockGhClient` Trees API mock 응답에 submodule (`160000`) / symlink (`120000`) / `100755` entry 박음. integration test 박음 + JSON 출력 정합 검증.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Submodule/Symlink/실행 권한.
  - 결과 (2026-05-09): `crates/gitless-sync/tests/scan_modes.rs` 박음 — 4 integration tests (3 per-mode + 1 summary). 단일 Trees API mock body가 submodule (`160000` + `type: "commit"`) / symlink (`120000` + `type: "blob"`) / executable (`100755` + `type: "blob"`) entry를 모두 박음 (acceptance 정확 mirror). pipeline_tests*.rs 박힌 unit-level cover에 full pipeline integration (Trees JSON parse → walker → compare → output → serialize) 차원 추가. (1) `trees_executable_100755_classifies_identical_and_carries_mode_bit` — 실파일 `build.sh` content == Trees SHA → `Status::Identical` + `mode: "100755"` + `failed_reason`/`lfs_pointer` omit (mode bit 자체는 drift 아님, spec-domain-pitfalls.md § 실행 권한 + spec-output-schema.md § v1.1 acceptance line 144). (2) `trees_submodule_160000_classifies_failed_with_reason_and_mode` — `Status::Failed` + `failed_reason: "submodule"` + `mode: "160000"` + `remote_sha` 보존 + `lfs_pointer` omit. (3) `trees_symlink_120000_classifies_failed_with_reason_and_mode` — `Status::Failed` + `failed_reason: "symlink"` + `mode: "120000"` + `remote_sha` 보존 + `lfs_pointer` omit. (4) `trees_mode_combo_summary_counts_match_v1_1_classification` — 조인트 envelope 검증 (`schema_version: "1.1"` + summary `1 identical / 0 local_only_changed / 0 remote_only_changed / 0 drift / 2 failed` + `files.len() == 3`). **No commits stub** — submodule/symlink는 short-circuit before Commits API + identical executable는 SHA-equality skip (G-003). 원치 않는 Commits API 호출은 `TestGhClient: no stub registered`로 surface (contract 자동 가드). **advisor real verification gap fix** — 초기 4번째 test가 `files` only 검증 (`json["summary"]` 미박힘) → helper signature 변경 (`scan_all_modes()`가 envelope 그대로 return) + summary 자체 verify로 정합. cognitive_complexity 25/15 + too_many_lines 68/60 위반 1회 (단일 모놀리식 test) → 4 test split + scan helper 추출로 해소. validation: cargo fmt clean + clippy 0 warnings + xtask check-line-limits (52 files within 300, scan_modes.rs 158) + xtask check-cycles (0/0) + cargo machete clean + cargo test 293 lib + 32 integration + 49 xtask = **374 tests pass** (+4) + tarpaulin **90.34%** (945/1046 lines, +0.00% change).

- [x] **R2. error contract robustness fixture**
  - acceptance: 잘못된 `.gitattributes` syntax (parser robustness) / 깨진 UTF-8 (mid-byte truncation) / dangling symlink / circular symlink fixture 박음. spec-error-contracts.md N의 `failed_reason` 정합 검증.
  - spec: `docs/specs/spec-error-contracts.md` § Per-file Pitfall Reasons.
  - 결과 (2026-05-09): `crates/gitless-sync/tests/scan_robustness.rs` 박음 — 4 integration tests (cross-platform 2 + Unix-only 2). (1) `malformed_gitattributes_skips_negation_comments_and_empties_without_panic` — `parse_one_line`이 `!`-prefix negation / `#`-comment / 공백 line / pattern-only 박은 line 박은 silently skip 박음 (spec-hash-and-normalize.md § `.gitattributes` 파서 정합) + 트레일링 valid `*.txt text=auto` 박은 박은 적용 박음 (`keep.txt`가 LF-normalized 박은 hash 박은 동일 → `Status::Identical`). robustness 의미 = parse 도중 panic·abort 안 박음 + 후속 valid line 정상 적용. (2) `mid_byte_truncated_utf8_local_matches_remote_with_identical_raw_bytes` — EUC-KR-style 단일 leading byte continuation 누락 raw bytes (`0xC7 0xD1 0xC7`) 박은 NUL byte 0 + LF/CRLF 0 → unspecified branch 박은 raw bytes 그대로 통과 → b-policy 박은 spec-domain-pitfalls.md § Encoding 정합 박음 + 같은 raw bytes 박은 remote blob → `Status::Identical`. mid-byte invalid UTF-8 박은 hash chain 정상 통과 박음. (3) `dangling_symlink_local_classifies_failed_with_symlink_reason` — `#[cfg(unix)]` 박은 박은 target `nonexistent` 박은 symlink 박음 → walker `is_symlink: true` (lstat 박은 target follow X) → pipeline `Status::Failed` + `failed_reason: "symlink"` + `mode: "120000"`. dangling 박은 lstat success 박은 박은 graceful detect. (4) `circular_symlink_local_does_not_loop_and_classifies_both_failed` — `#[cfg(unix)]` 박은 `loop_a → loop_b → loop_a` 박은 cycle symlink → `WalkDir::follow_links(false)` 박은 박은 박은 lstat-only 박은 박은 무한 descent X → 두 endpoint 모두 detect → 각자 `Status::Failed` + `failed_reason: "symlink"` + `mode: "120000"`. **scope 박음**: ignore::GitignoreBuilder 박은 lenient 박은 character class (`[unclosed text=auto`) 받아들임 박은 박은 박음 — initial fixture (Config error expectation) 박은 ignore crate 0.4 박은 박은 박은 박은 wrong → 박은 박은 박은 박은 박은 robustness fixture (silent-skip + 트레일링 valid 박은 적용 박은 contract) 박은 retarget. spec-error-contracts.md § Per-file Pitfall Reasons 박은 `gitattributes_unsupported`/`encoding`/`nfd_collision` 박은 enum-spec'd-but-unimplemented 박은 정합 — 본 task 박은 박은 caller-side plumbing 추가 안 박음 (Phase 5 후속 follow-up scope). validation: cargo fmt clean + clippy 0 warnings + xtask check-line-limits (53 files within 300, scan_robustness.rs 174) + xtask check-cycles (0/0) + cargo machete clean + cargo test 293 lib + 34 integration + 49 xtask = **376 tests pass** (Windows; +2 vs R, Unix runner +4) + tarpaulin **90.54%** (947/1046 lines, +0.19% change, gitattributes.rs +3.39% — silent-skip path 박음 cover).

- [x] **R3. large vault scale fixture (Phase 5 perf 회귀 차단)**
  - acceptance: 10K / 100K 파일 fixture 박음 (`tempfile` 또는 mock). `.gitattributes` 파싱 cost + per-file glob fnmatch P95 측정. Phase 4 batching 효과 상쇄 안 됨 검증 (1000 path scale ~38x speedup 유지).
  - spec: 없음 (perf 회귀 차단, X task와 통합).
  - 결과 (2026-05-09): `crates/gitless-sync/benches/scan_scale.rs` 박음 (criterion 0.5 default-features off, `harness = false`, `BenchGhClient` inline). 3 시나리오 — 10K real-file identical without `.gitattributes` (mean 497 ms / 95% CI 486-509 / N=20), 10K real-file identical with 100-rule `.gitattributes` (mean 1402 ms / 95% CI 1391-1414 / N=20), 100K mock-only remote-only no `.gitattributes` (mean 175 ms / 95% CI 173-177 / N=10, walker/hash 미포함). `.gitattributes` overhead **2.82x** (`1402 / 497`) — per-path ~90 µs end-to-end, X bench의 ~50 µs P95 single match × 2 (`is_lfs` + `prepare_for_hash`) 정합. **Phase 4 batching 직접 측정 불가** — `BenchGhClient`가 HashMap lookup이라 REST/GraphQL backend cost 차이 0. Indirect 검증으로 박음 (advisor BLOCKING fix #1) — `.gitattributes` overhead 0.9s vs Commits API real-`gh` walltime (ADR 0006: 13 path REST 2484 ms / GraphQL 1437 ms) 비교로 batching gain 보존 structural 주장. `docs/research/phase5-scan-scale-bench.md` 박음. CI 게이트는 U task에서 — 본 task record-only (advisor BLOCKING fix #2 — GitHub `windows-latest` noise로 절대 ms 임계 회피, ratio-based threshold `with_attrs / without_attrs ≤ 3.5×` 권고). 코드 변경: bench 1 file (270 LOC) + Cargo.toml [[bench]] entry + research doc + plan status. 다른 task scope 침범 없음. validation: cargo fmt clean + clippy 0 warnings + xtask check-line-limits (52 + 5 within 300, bench DEFAULT_SCAN_ROOTS 외 자연 스킵) + xtask check-cycles (0/0) + cargo machete clean + cargo test 293 lib + 34 integration + 49 xtask = **376 tests pass** (R2 baseline 그대로) + tarpaulin **90.54%** (947/1046 lines, +0.00% change — bench는 tarpaulin 대상 아님).

- [x] **S. `.gitattributes` integration fixture**
  - acceptance: `tempfile`에 `.gitattributes` 박은 후 K1~K4 + K1.5 통과 검증. text=auto / binary / eol=lf / eol=crlf / unsupported 5 시나리오 + multi-level fixture.
  - spec: `docs/specs/spec-domain-pitfalls.md` § `.gitattributes`.
  - 결과 (2026-05-09): `crates/gitless-sync/tests/scan_gitattributes.rs` 박음 — 7 integration tests (5 branch routing + multi-level depth + envelope summary). 단일 ROOT_ATTRS + NESTED_ATTRS fixture (root에 5 line + `*.txt eol=crlf`, nested에 `*.txt eol=lf`) + 단일 Trees mock body 박음 — 9 entry (.gitattributes 자체 2 + scenario 7) 모두 양쪽 raw bytes 일치 → Identical → SHA-equality skip (G-003) → Commits API 호출 0 (TestGhClient unstubbed argv면 Http error 박혀 contract guard). routing 검증은 published `local_sha` value가 각 분기의 expected output bytes hash와 일치하는지 박음 (advisor 권고 — `apply_text_auto`/`apply_eol_lf` body identical 박혀 observable 차이 0건이라 routing+hash value pin만 가능). (1) text=auto: NUL byte 박은 fixture (`b"a\x00b\r\nc"`) → NUL 휴리스틱 bypass + LF normalize → expected `blob_hash(b"a\x00b\nc")` + `is_binary=false` 검증. (2) binary: NUL-free CRLF (`b"hello\r\nworld\r\n"`) → raw bytes 보존 → expected `blob_hash(raw)` + `is_binary=true` 검증. (3) eol=lf: CRLF → LF normalize → expected `blob_hash(b"line\n")`. (4) eol=crlf: CRLF 보존 → expected `blob_hash(raw CRLF)` + 기본 LF hash와 `assert_ne` (routing pin). (5) unsupported (`working-tree-encoding=UTF-16`): caller-side `failed_reason: "gitattributes_unsupported"` plumbing이 K1.5 scope 외 (spec-hash-and-normalize.md line 168) → default fall-through → `apply_unspecified` LF normalize + `failed_reason` omit 검증 + follow-up comment. (6) multi-level: root `*.txt eol=crlf` (`notes.txt`) + nested `*.txt eol=lf` (`nested/notes.txt`) → 동일 raw CRLF input → 다른 hash (depth winner observable). (7) summary envelope: 9 identical / 0 others + `schema_version: "1.1"` lock. **scope**: 새 `tests/scan_gitattributes.rs` (266 LOC) + plan status. production code edit 0 (다른 task scope 침범 없음). validation: cargo fmt clean (4 fmt fix — array entry breaking 정합) + clippy 0 warnings (5 warning fix — 4 `doc-markdown` 백틱 + 1 `format-push-string` → `write!` macro + `Write` trait import; LSP false positive `unused_imports` warning은 macro hygiene으로 cargo build에서 surface 안 함) + xtask check-line-limits (53 files within 300, scan_gitattributes.rs 266) + xtask check-cycles (0/0) + cargo machete clean + cargo test 293 lib + 41 integration + 49 xtask = **383 tests pass** (Windows; +7 vs R3 baseline 376) + tarpaulin **90.73%** (949/1046 lines, +0.19% change vs R3 baseline 90.54%).

- [~] **Y. encoding_rs binary size 사후 측정**
  - acceptance: cargo-bloat + dependency tree 분석으로 encoding_rs 박힘 후 binary size 변화 측정. baseline (Phase 4 시점) 대비 delta. clean-context §5 의심점 사후 검증. `docs/research/encoding-library-eval.md` 박음 (E task와 통합).
  - spec: 없음 (research artifact).

### Phase 5.10 — vault dogfooding + 회귀 검증

- [ ] **T. vault dogfooding (Phase 5 후)**
  - acceptance: Phase 5 후 vault scan 재실행 — false drift 0건 (의도된 detect-only drift = submodule/symlink/encoding fail 제외). `docs/research/phase5-vault-after.md` 박음 + before/after 비교.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Acceptance Criteria.

- [ ] **W. v0.1 baseline regression diff (정확화 vs 회귀 자동 분류)**
  - acceptance: v0.1 출력 baseline JSON 박고 v0.2 출력과 자동 diff 분류. 정확화 화이트리스트 (`.gitattributes` 박힌 vault에서 binary 정확 분류 / NFC 정규화로 false drift 해소 / LFS pointer 명시 박힘) 박음. 화이트리스트 외 status 변화는 회귀로 박음 (자동 fail). `docs/research/phase5-regression.md` 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § v0.1 vs v0.2 회귀 정의.

### Phase 5.11 — 최종 박제 + CI

- [ ] **U. CI gate 갱신 (.github/workflows/ci.yml)**
  - acceptance: `.gitattributes` / encoding fixture / submodule mock / LFS pointer / Windows long path 시나리오를 CI에서 검증. Windows runner에서 통과.
  - spec: 없음 (CI).

- [ ] **V. CLAUDE.md / roadmap.md 완료 박스 박음**
  - acceptance: Phase 6 완료 박스 처럼 Phase 5 완료 박스 박음. 다음 세션 진입점 갱신 (vault scale 1000+ path / Phase 7+).
  - spec: 없음 (docs).

- [ ] **V1. CHANGELOG.md v0.2 박음**
  - acceptance: `CHANGELOG.md` 신규 박힘 (이미 Phase 5 docs 갱신에서 박힘). v0.2 entry 박음 — Phase 5 함정 처리 + spec 변경 + breaking changes (schema_version 1.1 minor bump 박힘, 호환).
  - spec: 없음 (docs).

### Phase 5.12 — Audit & cleanup (병렬 sub-agent, 모든 task 후 마지막 sweep)

- [ ] **Z. 코드 스멜 audit + sibling test file 정리 (병렬 Explore sub-agent 6개)**
  - acceptance: 6 sub-agent로 코드베이스 audit 병렬 박음 — (1) Test 구조 (sibling test file / same file mod tests vs integration test 분류 / `#[cfg(test)]` 가드 일관) / (2) Module 구조 (`mod.rs` re-export only 패턴 / sub-module 분리 / file LOC 분포) / (3) Error handling (`Result` propagation / `failed_reason` enum 분류 / `?` + `Context` vs panic) / (4) Public API exposure (`pub` over-exposure / `pub(crate)`/`pub(super)` 가시성 정합) / (5) Rust 관용 위반 (`.clone()` 남용 / `&Vec<T>` vs `&[T]` / `&String` vs `&str` / `Vec::new()` + push vs `vec![]` macro / dead code) / (6) Panic escape hatch 우회 (`.expect("unreachable")` + `#[allow(clippy::expect_used)]` 같은 우회 패턴). audit 결과 `docs/research/code-smell-audit.md` 박음 (영역별 발견 list + 위반 위치).
  - **확정 fix 1건 즉시 박음** — `shared/gitattributes.rs` (296 LOC) → `shared/gitattributes/{mod.rs, parser.rs, classify.rs, matching.rs}` module 폴더 분할 + 각 sub-module에 `#[cfg(test)] mod tests` 박음 + `shared/gitattributes_tests.rs` + `shared/gitattributes_classify_tests.rs` 제거 (spec-architecture.md § 금지 패턴 정합).
  - 추가 fix는 audit list로 사람이 후행 task 박음 (CLAUDE.md ralph plan 모드 스킵 정책).
  - 검증: tarpaulin 80% 유지 + 257+ tests pass + Phase 6 hard gate 모두 deny active 유지.
  - spec: `docs/specs/spec-architecture.md` § LOC 임계 § 금지 패턴 + § 구조적 분리.

## 의존 순서

```
A → B (vault 데이터 → 우선순위 박음)
A → A2 (.gitignore 정책 — vault 분석 결과 정합)
B → {C, D, E, F1, G, H, I, J, K1}  (우선순위 박힌 후 함정별 처리 시작)
C → D1 (NTFS case local-side detection은 NFC 정규화 후 박음)
E → F → Y (인코딩 라이브러리 결정 후 변환 + binary size 측정)
G → H → J → R (mode field 공유 + integration fixture)
G/H → G1 (LFS pointer는 blobs IO에 박힘 — github 모듈 갱신)
G/H/J → R1 (Windows long path는 walker 박힘)
K1 → {K1.5, K2, K3, K4} (.gitattributes 파서 후 정책)
K2 → F (conditional normalize 박힌 후 인코딩 변환 hash 입력 정합)
{K1, K2, K3, K4, K1.5} → X (perf gate은 K 박힌 후 측정)
{C, D, D1} → M (path 정규화 → spec)
{G, H, G1, R1} → N (함정별 reason → spec)
{J, G1, G, H} → O (mode bit + reason + LFS → spec)
{K1, K2, K3, K4} → L (.gitattributes → spec)
{K1, L1} → spec-config 정합
{C, D, D1, F, F1, G, H, G1, R1, I, J, K1~K4, K1.5} → {P, P1, Q, R, R2, R3, S, Y} (함정 처리 후 fixture 박음)
모든 함정 task + L/M/N/O/L1 완료 → T (vault dogfooding)
T → W (regression diff 자동 분류)
T/W → U (CI gate 박힘)
U → V → V1 (완료 박스 + CHANGELOG)
V1 → Z (모든 task 완료 후 audit + cleanup sweep)
```

ralph build mode 진행 권장 순서:
1. A (vault 분석 + fact check)
2. B → A2 (우선순위 + .gitignore 정책)
3. C → D → D1 (path 정규화 + NTFS local-side)
4. E → F → Y → F1 (encoding 변환 + binary size + BOM)
5. G → H → J → G1 → R1 (mode bit + LFS + Windows long path)
6. I (빈 파일 실파일 fixture)
7. K1 → K1.5 → K2 → K3 → K4 → X (.gitattributes 5 sub-task + perf gate)
8. L → M → N → O → L1 (spec 갱신 cascade)
9. P → P1 → Q → R → R2 → R3 → S (보강 fixture)
10. T → W (vault dogfooding + regression diff)
11. U → V → V1 (CI + 완료 박스 + CHANGELOG)
12. Z (audit + cleanup, 모든 task 후 마지막 sweep — 병렬 sub-agent)
