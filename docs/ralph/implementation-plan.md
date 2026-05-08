# Implementation Plan

## Status
- Last updated: 2026-05-09 (Phase 5 진입 — 8 도메인 함정 + clean-context 보강 12 task 박힘)
- Total tasks: 34
- Completed: 0 / 34

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Phase 6 hard gate 모두 deny active 유지 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출). 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (project-ops.md). Phase 5 새 코드 cover 정책 — 신규 task의 acceptance에 unit test 포함.

## Tasks (Phase 5 — 도메인 함정 정리)

### Phase 5.1 — vault 운영 데이터 분석 + fact check (우선순위 입력)

- [ ] **A. vault scan 재실행 + drift 근원 분석 + 검증 필요 fact check**
  - acceptance: KneShell/gitless-sync 또는 사용자 vault repo (356+ files)에 대해 `cargo run -- scan` 재실행. drift/failed/local_only_changed/remote_only_changed 분류. 각 drift entry에 대해 함정 (NFD/case/encoding/submodule/symlink/empty/permission/.gitattributes/BOM/LFS/long_path/.gitignore) 중 어느 것이 원인인지 분석. **검증 필요 fact check sub-step**: encoding_rs binary size (cargo-bloat), git core NUL byte heuristic 정확 N (git source), Windows NTFS NFD 파일 생성 검증 (실험). `docs/research/phase5-vault-baseline.md` 박음.
  - spec: 없음 (research artifact).

- [ ] **B. 우선순위 박음 — vault 데이터 기반**
  - acceptance: vault 분석 결과로 함정 우선순위 박음. spec-domain-pitfalls.md § "v0.1 baseline 영향"에 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md`.

- [ ] **A2. `.gitignore` 무시 정책 spec 명시**
  - acceptance: spec-classification.md § `.gitignore` 무시 정책 박혀있음 (이미 Phase 5 spec 갱신에서 박힘). spec-ignore-policy.md 갱신해 scan 범위 명시. unit test 박음 — `target/`, `node_modules/`, custom `.gitignore` ignored path 검증.
  - spec: `docs/specs/spec-classification.md` § `.gitignore` 무시 정책 + `docs/specs/spec-ignore-policy.md`.

### Phase 5.2 — path 정규화 함정 (NFD / case)

- [ ] **C. NFD → NFC path 정규화**
  - acceptance: `walker.rs`에서 local file path를 NFC로 정규화 (`unicode-normalization` crate). remote tree path도 NFC 정규화. 비교 key는 NFC. unit test (raw bytes injection으로 NFD 가짜 fixture 박음).
  - spec: `docs/specs/spec-domain-pitfalls.md` § Path 정규화 + `spec-classification.md`.

- [ ] **D. 대소문자 충돌 처리 정책 박음**
  - acceptance: 같은 path key가 두 case (`README.md` vs `Readme.md`)로 박힐 때 case-sensitive 비교 정책 박음. integration test fixture (mock Trees API + walker).
  - spec: `docs/specs/spec-domain-pitfalls.md` § Path 정규화 + `spec-classification.md`.

- [ ] **D1. Windows NTFS case collision local-side detection**
  - acceptance: walker가 NTFS local에서 case-collision 박힌 directory 박을 때 (예: `Foo.txt` + `foo.txt` 같은 directory에 박힌 환경) 두 entry 모두 catch + case-sensitive 비교로 정합. NTFS volume이 case-insensitive로 1개만 catch한 case도 detect — `Status::Failed` + `failed_reason: "case_collision"` 박음. unit test (mock filesystem).
  - spec: `docs/specs/spec-domain-pitfalls.md` § Windows NTFS local-side case detection.

### Phase 5.3 — encoding 변환 시도 (hash 입력 (b) 정책)

- [ ] **E. 인코딩 라이브러리 조사 + 채택**
  - acceptance: `encoding_rs` (Mozilla, Recommended) vs `chardet` 평가. UTF-8 → 다른 인코딩 detect 정확도 + Rust ecosystem 정합 + license. `docs/research/encoding-library-eval.md` 박음. 결정 박음.
  - spec: 없음 (research).

- [ ] **F. 비-UTF-8 인코딩 변환 시도 박음 (hash 입력 (b))**
  - acceptance: `normalize.rs`에 `try_decode_text` 함수 박음. 1차 UTF-8 디코드 시도 → 2차 다른 인코딩 detect → 3차 binary 취급 (`Status::Failed` + `failed_reason: "encoding"`). **hash 입력은 항상 원본 raw bytes** (clean-context (b) 정책). detect는 reason 마크 + JSON 출력 정보만. unit test (EUC-KR / Shift_JIS / Latin-1 fixture + identical raw bytes 검증). tarpaulin 80% 유지 — encoding detect 분기 cover.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Encoding (hash 입력 (b)) + `spec-hash-and-normalize.md`.

- [ ] **F1. BOM 처리 정책 박음 (UTF-8 + UTF-16)**
  - acceptance: UTF-8 BOM은 v0.1 그대로 처리 (text=auto + 미명시에서 strip). UTF-16 BOM (`FF FE` LE / `FE FF` BE) detect 시 `Status::Failed` + `failed_reason: "encoding"`. unit test (UTF-8 BOM strip + UTF-16 BOM detect).
  - spec: `docs/specs/spec-hash-and-normalize.md` § BOM + `spec-domain-pitfalls.md`.

### Phase 5.4 — submodule / symlink / LFS pointer / Windows long path

- [ ] **G. submodule (`160000`) detect-only**
  - acceptance: `github.rs::trees`에서 submodule entry skip 대신 `RemoteFile`에 mode 박음. `compare.rs`에서 submodule path → `Status::Failed` + `failed_reason: "submodule"`. JSON 출력에 mode bit (`160000`) 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Submodule + `spec-classification.md`.

- [ ] **H. symlink (`120000`) detect-only**
  - acceptance: `github.rs::trees`에서 symlink entry mode 박음. walker가 local symlink 발견 시 `Status::Failed` + `failed_reason: "symlink"`. JSON 출력에 mode bit (`120000`) 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Symlink + `spec-classification.md`.

- [ ] **G1. git LFS pointer detection (.gitattributes filter=lfs 신호 기반)**
  - acceptance: scan은 **blob fetch 안 함** (Phase 4 GraphQL batching 이득 유지) — `.gitattributes` 파싱 시점에 `filter=lfs` 매칭된 path는 자동 `Status::Failed` + `failed_reason: "lfs_pointer"` + `lfs_pointer: {oid: "?", size: 0}` (oid/size unknown). diff 명령은 blob fetch 박혀있어 첫 줄 시그니처 `version https://git-lfs.github.com/spec/v1` 검증 + oid/size 정확 파싱 (defence-in-depth). **K1.5 LfsPointer variant 의존**. unit test (LFS pointer fixture + filter=lfs `.gitattributes` fixture). tarpaulin 80% 유지 — 신규 코드 cover 책임.
  - spec: `docs/specs/spec-domain-pitfalls.md` § LFS pointer + `spec-error-contracts.md` + `spec-hash-and-normalize.md` § 화이트리스트.

- [ ] **R1. Windows long path / 예약 파일명 detect-only**
  - acceptance: walker에서 260자+ path 또는 예약 파일명 (CON/PRN/NUL/AUX/COM1-9/LPT1-9) detect → `Status::Failed` + `failed_reason: "long_path"`. unit test (mock filesystem fixture).
  - spec: `docs/specs/spec-domain-pitfalls.md` § Windows long path + `spec-error-contracts.md`.

### Phase 5.5 — 빈 파일 실파일 검증

- [ ] **I. 빈 파일 실파일 fixture + integration test**
  - acceptance: integration test fixture로 실제 0-byte 파일 박음. `blob_hash(&[])` == git empty blob constant 확인. local empty file ↔ remote empty blob → `Status::Identical`.
  - spec: `spec-hash-and-normalize.md` § Acceptance.

### Phase 5.6 — 실행 권한 detect

- [ ] **J. mode bit (`100755` vs `100644`) detect-only**
  - acceptance: `RemoteFile`에 mode field 박음 (G/H에서 박힌 같은 field 활용). `compare.rs`에서 mode bit 차이 발견 시 `Status::Identical` 유지 (content 같으면), JSON 출력에 mode 정보 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § 실행 권한 + `spec-output-schema.md`.

### Phase 5.7 — `.gitattributes` 정확 hash 재현 (큰 변경)

- [ ] **K1. `.gitattributes` 파서 박음 (working tree 한정)**
  - acceptance: `shared/gitattributes.rs` 박음. project root + 하위 디렉토리의 `.gitattributes` 파일 1회 로드 + glob pattern matching (gitignore-style). 우선순위: 가장 깊은 `.gitattributes` 우선 + line-level 마지막 매칭 winner. `.git/info/attributes` / global 미지원. unit test (multi-level fixture). tarpaulin 80% 유지 — 신규 모듈 cover 책임.
  - spec: `spec-hash-and-normalize.md` § `.gitattributes` 파서 + `spec-config.md` § 위치 정책.

- [ ] **K1.5. `.gitattributes` 지원 attribute 화이트리스트 박음 (5 entry, advisor BLOCKING fix)**
  - acceptance: `AttributeMatch` enum 박음 — `TextAuto / Binary / EolLf / EolCrlf / LfsPointer / Unspecified / Unsupported { attribute_name }`. **`filter=lfs`는 LfsPointer variant** (advisor BLOCKING fix — git-lfs 표준 마커). 화이트리스트 외 (`working-tree-encoding`, `ident`, `filter=*` (lfs 외), macro attributes, `crlf` legacy) 매칭 시 `Unsupported` 박음. unit test (5 화이트리스트 + 5 unsupported fixture). tarpaulin 80% 유지.
  - spec: `docs/specs/spec-domain-pitfalls.md` § `.gitattributes` 화이트리스트 + `spec-hash-and-normalize.md` § 화이트리스트 강제 + § LFS pointer.

- [ ] **K2. conditional LF normalize + lifetime 계약 박음 (advisor flag 3 — 분기 helper 분리)**
  - acceptance: `prepare_for_hash` 함수 시그니처 변경 — `gitattr: &Arc<GitAttributes>` 인자 추가. **7 분기** (text=auto / binary / eol=lf / eol=crlf / **LfsPointer** / Unspecified / Unsupported) 박음. `Unspecified` default = v0.1 정책. `LfsPointer`/`Unsupported`는 caller가 `Status::Failed` 박음. **분기 helper fn 분리 박음** — `apply_text_auto`, `apply_binary`, `apply_eol_lf`, `apply_eol_crlf`, `apply_unspecified` 5 helper. Phase 6 `cognitive_complexity = 15` deny 회피 (advisor 권고). 모든 caller 갱신. unit test (단일 vault scan 1회 파싱 + N번 호출 reparse 0회 검증). tarpaulin 80% 유지 — 7 분기 모두 cover.
  - spec: `spec-hash-and-normalize.md` § Normalize 규칙 + § Lifetime 계약.

- [ ] **K3. binary attribute 정확 적용**
  - acceptance: `.gitattributes`에 `binary` 명시된 file은 NUL byte 휴리스틱 무시 + raw bytes 해시. unit test (NUL byte 0개 binary fixture).
  - spec: `spec-hash-and-normalize.md` § Normalize 규칙.

- [ ] **K4. `.gitattributes` 우선순위 정합 검증**
  - acceptance: project root `.gitattributes` < sub-directory `.gitattributes` < line-level pattern 마지막 매칭 winner 정합. unit test로 검증 (3-level fixture).
  - spec: `spec-hash-and-normalize.md` § `.gitattributes` 파서.

- [ ] **X. `.gitattributes` parser performance gate**
  - acceptance: 큰 vault 시뮬레이션 (10K+ files × 100+ rules) per-file glob fnmatch P95 측정. baseline 박음. Phase 6 hard gate에 perf regression 임계 박음 (P95 X ms 초과 시 fail).
  - 검증: cargo bench (criterion) + Phase 6 CI gate.
  - spec: 없음 (perf gate, R3 task와 통합 가능).

### Phase 5.8 — spec 갱신 cascade

- [ ] **L. spec-hash-and-normalize.md `.gitattributes` 박힌 정합 검증**
  - acceptance: spec 본문 + acceptance criteria가 K1~K4 결과와 정합. 기존 PRD 시나리오 5/6/7 통과 + 새 시나리오 박음. (Phase 5 spec 갱신에서 이미 박혔으니 본 task는 implementation 정합 검증.)
  - spec: `spec-hash-and-normalize.md`.

- [ ] **M. spec-classification.md path 정규화 정합**
  - acceptance: spec 본문에 NFC 정규화 + case-sensitive 정책 박혀있음 (이미 Phase 5 spec 갱신). 4분류 판정 정합 검증.
  - spec: `spec-classification.md`.

- [ ] **N. spec-error-contracts.md 함정별 reason 매핑**
  - acceptance: `failed_reason` enum 9 값 (이미 Phase 5 spec 갱신) 정합 검증. unit test 박음.
  - spec: `spec-error-contracts.md`.

- [ ] **O. spec-output-schema.md mode bit + reason + LFS 필드 검증**
  - acceptance: schema_version `"1.1"` + `mode` + `failed_reason` + `lfs_pointer` 필드 박힘 (이미 Phase 5 spec 갱신). v1.0 호출자 backward-compat 검증 unit test.
  - spec: `spec-output-schema.md`.

- [ ] **L1. spec-config.md `.gitattributes` 위치 정책 검증**
  - acceptance: spec 본문 박혀있음 (이미 Phase 5 spec 갱신). working tree 한정 + `.git/info/attributes` / global 미지원 정합 검증.
  - spec: `spec-config.md`.

### Phase 5.9 — 보강 fixture

- [ ] **P. NFD raw bytes injection unit test fixture**
  - acceptance: Windows 환경에서 raw bytes injection — compose 한글 (`가` = `\u{AC00}`) vs decompose (`가` = `\u{1100}\u{1161}`) 둘 다 시도. `walker.rs` + 비교 path key 정합 검증.
  - spec: `docs/specs/spec-domain-pitfalls.md` § 검증 환경.

- [ ] **P1. NFD NTFS 실파일 fixture (clean-context §5 fact check)**
  - acceptance: NTFS는 normalize 안 함 — NFD/NFC 실파일 직접 생성 가능. `tempfile` 박음 + walker가 정확 NFC 정규화 + NFC 정규화로 동일 key 검증. integration test.
  - spec: `docs/specs/spec-domain-pitfalls.md` § 검증 환경.

- [ ] **Q. 인코딩 변환 fixture 박음**
  - acceptance: EUC-KR / Shift_JIS / Latin-1 byte literal fixture 박음 + 변환 시나리오 unit test. **hash 입력 raw bytes 정합 검증**.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Encoding.

- [ ] **R. submodule/symlink/permission integration fixture**
  - acceptance: `MockGhClient` Trees API mock 응답에 submodule (`160000`) / symlink (`120000`) / `100755` entry 박음. integration test 박음 + JSON 출력 정합 검증.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Submodule/Symlink/실행 권한.

- [ ] **R2. error contract robustness fixture**
  - acceptance: 잘못된 `.gitattributes` syntax (parser robustness) / 깨진 UTF-8 (mid-byte truncation) / dangling symlink / circular symlink fixture 박음. spec-error-contracts.md N의 `failed_reason` 정합 검증.
  - spec: `docs/specs/spec-error-contracts.md` § Per-file Pitfall Reasons.

- [ ] **R3. large vault scale fixture (Phase 5 perf 회귀 차단)**
  - acceptance: 10K / 100K 파일 fixture 박음 (`tempfile` 또는 mock). `.gitattributes` 파싱 cost + per-file glob fnmatch P95 측정. Phase 4 batching 효과 상쇄 안 됨 검증 (1000 path scale ~38x speedup 유지).
  - spec: 없음 (perf 회귀 차단, X task와 통합).

- [ ] **S. `.gitattributes` integration fixture**
  - acceptance: `tempfile`에 `.gitattributes` 박은 후 K1~K4 + K1.5 통과 검증. text=auto / binary / eol=lf / eol=crlf / unsupported 5 시나리오 + multi-level fixture.
  - spec: `docs/specs/spec-domain-pitfalls.md` § `.gitattributes`.

- [ ] **Y. encoding_rs binary size 사후 측정**
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
