# Implementation Plan

## Status
- Last updated: 2026-05-09 (Phase 5 진입 — 8 도메인 함정 정리, vague 결론 박힘)
- Total tasks: 22
- Completed: 0 / 22

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Phase 6 hard gate 모두 deny active 유지 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출). 위반 시 task `[!]` BLOCKED.

## Tasks (Phase 5 — 도메인 함정 정리)

### Phase 5.1 — vault 운영 데이터 분석 (우선순위 입력)

- [ ] **A. vault scan 재실행 + drift 근원 분석**
  - acceptance: KneShell/gitless-sync 또는 사용자 vault repo (356+ files)에 대해 `cargo run -- scan` 재실행. 결과 drift/failed/local_only_changed/remote_only_changed 분류. 각 drift entry에 대해 8 함정 (NFD/case/encoding/submodule/symlink/empty/permission/.gitattributes) 중 어느 것이 원인인지 분석. `docs/research/phase5-vault-baseline.md` 박음.
  - spec: 없음 (research artifact).

- [ ] **B. 우선순위 박음 — vault 데이터 기반**
  - acceptance: vault 분석 결과로 8 함정 우선순위 박음 (가장 자주 발생하는 함정 → 가장 적은 함정). spec-domain-pitfalls.md § "v0.1 baseline 영향"에 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md`.

### Phase 5.2 — path 정규화 함정 (NFD / case)

- [ ] **C. NFD → NFC path 정규화**
  - acceptance: walker.rs에서 local file path를 NFC로 정규화 (`unicode-normalization` crate). remote tree path도 NFC 정규화. 비교 key는 NFC. 함수 시그니처 변경 없음. unit test (raw bytes injection으로 NFD 가짜 fixture 박음).
  - spec: `docs/specs/spec-domain-pitfalls.md` § Path 정규화 + `spec-classification.md`.

- [ ] **D. 대소문자 충돌 처리 정책 박음**
  - acceptance: 같은 path key가 두 case (`README.md` vs `Readme.md`)로 박힐 때 처리. 정책: case-sensitive 비교 (Unix-style). Windows 환경에서는 OS가 case-insensitive로 처리하니 도구는 case-sensitive 그대로 박음. integration test fixture 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Path 정규화 + `spec-classification.md`.

### Phase 5.3 — encoding 변환 시도

- [ ] **E. 인코딩 라이브러리 조사 + 채택**
  - acceptance: `encoding_rs` (Mozilla) vs `chardet` 평가. UTF-8 → 다른 인코딩 detect 정확도 + Rust ecosystem 정합 + license + dependency 확장 검토. `docs/research/encoding-library-eval.md` 박음. 결정 박음.
  - spec: 없음 (research).

- [ ] **F. 비-UTF-8 인코딩 변환 시도 박음**
  - acceptance: `normalize.rs`에 `try_decode_text` 함수 박음. 1차 UTF-8 디코드 시도 → 2차 다른 인코딩 detect → 3차 binary 취급 (Status::Failed). 변환 성공 시 LF normalize 적용. unit test (EUC-KR / Shift_JIS / Latin-1 fixture).
  - spec: `docs/specs/spec-domain-pitfalls.md` § encoding + `spec-hash-and-normalize.md`.

### Phase 5.4 — submodule / symlink detect

- [ ] **G. submodule (`160000`) detect-only**
  - acceptance: `github.rs::trees`에서 submodule entry skip 대신 `RemoteFile`에 mode 박음. `compare.rs`에서 submodule path → Status::Failed + reason "submodule". JSON 출력에 mode bit (`160000`) 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Submodule + `spec-classification.md`.

- [ ] **H. symlink (`120000`) detect-only**
  - acceptance: `github.rs::trees`에서 symlink entry mode 박음. walker가 local symlink 발견 시 Status::Failed + reason "symlink". JSON 출력에 mode bit (`120000`) 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Symlink + `spec-classification.md`.

### Phase 5.5 — 빈 파일 실파일 검증

- [ ] **I. 빈 파일 실파일 fixture + integration test**
  - acceptance: integration test fixture로 실제 0-byte 파일 박음. `blob_hash(&[])` == git empty blob constant (`e69de29bb2d1d6434b8b29ae775ad8c2e48c5391`) 확인. local empty file ↔ remote empty blob → Status::Identical.
  - spec: `spec-hash-and-normalize.md` § Acceptance.

### Phase 5.6 — 실행 권한 detect

- [ ] **J. mode bit (`100755` vs `100644`) detect-only**
  - acceptance: `RemoteFile`에 mode field 박음 (G/H에서 박힌 같은 field 활용). `compare.rs`에서 mode bit 차이 발견 시 Status::Identical 유지 (content 같으면), JSON 출력에 mode 정보 박음.
  - spec: `docs/specs/spec-domain-pitfalls.md` § 실행 권한 + `spec-output-schema.md`.

### Phase 5.7 — `.gitattributes` 정확 hash 재현 (큰 변경)

- [ ] **K1. `.gitattributes` 파서 박음**
  - acceptance: `shared/gitattributes.rs` 박음. project root + 하위 디렉토리의 `.gitattributes` 파일 1회 로드 + glob pattern matching (gitignore-style). 우선순위: 가장 깊은 `.gitattributes`가 우선 + line-level pattern은 마지막 매칭이 winner. unit test (multi-level fixture).
  - spec: `spec-hash-and-normalize.md` § `.gitattributes` 파서.

- [ ] **K2. conditional LF normalize 박음**
  - acceptance: `prepare_for_hash` 함수 시그니처 변경 — `gitattr: &GitAttributesMatch` 인자 추가. `text=auto` / `binary` / `eol=lf` / `eol=crlf` / 미명시 5 분기 박음. 미명시 default = v0.1 정책 그대로. 모든 caller 갱신.
  - spec: `spec-hash-and-normalize.md` § Normalize 규칙.

- [ ] **K3. binary attribute 정확 적용**
  - acceptance: `.gitattributes`에 `binary` 명시된 file은 NUL byte 휴리스틱 무시 + raw bytes 해시. unit test (NUL byte 0개 binary fixture).
  - spec: `spec-hash-and-normalize.md` § Normalize 규칙.

- [ ] **K4. `.gitattributes` 우선순위 정합 검증**
  - acceptance: project root `.gitattributes` < sub-directory `.gitattributes` < line-level pattern 마지막 매칭 winner 정합. unit test로 검증 (3-level fixture).
  - spec: `spec-hash-and-normalize.md` § `.gitattributes` 파서.

### Phase 5.8 — spec 갱신 cascade

- [ ] **L. spec-hash-and-normalize.md `.gitattributes` 박힌 정합 검증**
  - acceptance: spec 본문 + acceptance criteria가 K1~K4 결과와 정합. 기존 PRD 시나리오 5/6/7 통과 + 새 시나리오 (`.gitattributes` binary / eol=crlf) 박음.
  - spec: `spec-hash-and-normalize.md`.

- [ ] **M. spec-classification.md path 정규화 정합**
  - acceptance: spec 본문에 NFC 정규화 + case-sensitive 정책 박음. 4분류 판정 정합.
  - spec: `spec-classification.md`.

- [ ] **N. spec-error-contracts.md 함정별 에러 매핑**
  - acceptance: encoding 변환 fail / submodule / symlink → `GitlessError` variant 박음. exit code 매핑.
  - spec: `spec-error-contracts.md`.

- [ ] **O. spec-output-schema.md mode bit + reason 필드**
  - acceptance: 출력 JSON에 mode bit + skipped reason 박음. `schema_version` bump.
  - spec: `spec-output-schema.md`.

### Phase 5.9 — 보강 task

- [ ] **P. NFD raw bytes injection unit test fixture 박음**
  - acceptance: Windows 환경에서 macOS NFD path 시뮬레이션 fixture 박음. compose 한글 (`가` = `\u{AC00}`) vs decompose 한글 (`가` = `\u{1100}\u{1161}`) 둘 다 시도. `walker.rs` + 비교 path key 정합 검증.
  - spec: `docs/specs/spec-domain-pitfalls.md` § 검증 환경.

- [ ] **Q. 인코딩 변환 fixture 박음**
  - acceptance: EUC-KR / Shift_JIS / Latin-1 byte literal fixture 박음 + 변환 시나리오 unit test.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Encoding.

- [ ] **R. submodule/symlink/permission integration fixture**
  - acceptance: `MockGhClient` Trees API mock 응답에 submodule (`160000`) / symlink (`120000`) / `100755` entry 박음. integration test 박음 + JSON 출력 정합 검증.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Submodule/Symlink/실행 권한.

- [ ] **S. `.gitattributes` integration fixture**
  - acceptance: `tempfile`에 `.gitattributes` 박은 후 K1~K4 통과 검증. text=auto / binary / eol=lf / eol=crlf 4 시나리오 + multi-level fixture.
  - spec: `docs/specs/spec-domain-pitfalls.md` § `.gitattributes`.

### Phase 5.10 — vault dogfooding

- [ ] **T. vault dogfooding (Phase 5 후)**
  - acceptance: Phase 5 후 vault scan 재실행 — false drift 0건 (의도된 detect-only drift = submodule/symlink/encoding fail 제외). `docs/research/phase5-vault-after.md` 박음 + before/after 비교.
  - spec: `docs/specs/spec-domain-pitfalls.md` § Acceptance Criteria.

### Phase 5.11 — 최종 박제 + CI

- [ ] **U. CI gate 갱신 (.github/workflows/ci.yml)**
  - acceptance: `.gitattributes` / encoding fixture / submodule mock 추가 시나리오를 CI에서 검증. Windows runner에서 통과.
  - spec: 없음 (CI).

- [ ] **V. CLAUDE.md / roadmap.md 완료 박스 박음**
  - acceptance: Phase 6 완료 박스 처럼 Phase 5 완료 박스 박음. 다음 세션 진입점 갱신 (vault scale 1000+ path / Phase 7+).
  - spec: 없음 (docs).

## 의존 순서

```
A → B (vault 데이터 → 우선순위 박음)
B → {C, D, E, G, H, I, J, K1}  (우선순위 박힌 후 함정별 처리 시작)
E → F (인코딩 라이브러리 결정 후 변환 박음)
G → H (submodule mode 박힌 후 symlink 같은 field 활용)
G/H → J (mode field 공유)
K1 → {K2, K3, K4} (.gitattributes 파서 후 정책)
K2 → F (conditional normalize 박힌 후 인코딩 변환 정합)
{C, D} → M (path 정규화 → spec)
{G, H} → N (submodule/symlink → spec)
J → O (mode bit → spec)
{K1, K2, K3, K4} → L (.gitattributes → spec)
{C, D, F, G, H, I, J, K1~K4} → {P, Q, R, S}  (함정 처리 후 fixture 박음)
모든 함정 task + L/M/N/O 완료 → T (vault dogfooding)
T → U (CI gate 박힘)
U → V (완료 박스)
```

ralph build mode 진행 권장 순서:
1. A (vault 분석)
2. B (우선순위 박음)
3. C → D (path 정규화 — 가장 silent drift 위험 큼 추정)
4. E → F (encoding 변환)
5. G → H → J (mode bit detect-only 묶음)
6. I (빈 파일 실파일 fixture)
7. K1 → K2 → K3 → K4 (.gitattributes 4 sub-task)
8. L → M → N → O (spec 갱신 cascade)
9. P → Q → R → S (보강 fixture)
10. T (vault dogfooding)
11. U → V (CI + 완료 박스)
