# Spec — Domain Pitfalls

> Phase 5 박제 (2026-05-09, vague + clean-context 외부 시각 보강). v0.1 비목표였던 "도메인 함정 정리"를 영구 박음. 박제 expiration: Phase 진입마다 재검토 (CLAUDE.md § 박제 expiration 정책).

## 목적

GitHub repo와 로컬 디렉토리 간 비교에서 OS / 인코딩 / git 메타데이터 차이로 발생하는 false drift / silent drift 함정을 정확히 처리. v0.1 vault 검증(356 파일, 0 drift) 위에 누락된 도메인 케이스 박음.

## 함정 처리 정책 매핑

### 8 핵심 함정

| 함정 | 정책 | 영향 spec |
|---|---|---|
| **NFD vs NFC** (path) | 정확 hash 재현 — path NFC 정규화 | spec-classification.md |
| **대소문자 충돌** | 정확 hash 재현 — case-sensitive 비교 (Unix-style) + Windows local-side detection | spec-classification.md |
| **비-UTF-8 인코딩** | 변환 시도 후 detect-only — hash 입력은 **(b) 원본 raw bytes** (clean-context §1) | spec-output-schema.md / spec-error-contracts.md / spec-hash-and-normalize.md |
| **submodule (`160000`)** | detect-only — `Status::Failed` + `failed_reason: "submodule"` + mode bit 보고 | spec-github-api.md / spec-output-schema.md |
| **symlink (`120000`)** | detect-only — `Status::Failed` + `failed_reason: "symlink"` + mode bit 보고 | spec-github-api.md / spec-output-schema.md |
| **빈 파일** | 정확 hash + 실파일 검증 | spec-hash-and-normalize.md |
| **실행 권한 (`100755` vs `100644`)** | detect-only — content 같으면 `Status::Identical` 유지, mode bit 정보 보고 | spec-output-schema.md |
| **`.gitattributes`** | **정확 hash 재현 (큰 변경) + 화이트리스트** | spec-hash-and-normalize.md (구조 변경) / spec-config.md |

### 추가 함정 (clean-context §2 보강)

| 함정 | 정책 | 영향 spec |
|---|---|---|
| **UTF-8 BOM 처리** | text=auto에서 BOM strip (v0.1 그대로). UTF-16 BOM detect 시 `failed_reason: "encoding"` | spec-hash-and-normalize.md |
| **git LFS pointer** | detect-only — pointer 시그니처 detect → `Status::Failed` + `failed_reason: "lfs_pointer"` + `lfs_pointer: {oid, size}` | spec-output-schema.md / spec-error-contracts.md |
| **Windows long path** (260자+) / 예약 파일명 (CON/PRN/NUL/AUX) | detect-only — `Status::Failed` + `failed_reason: "long_path"` | spec-error-contracts.md |
| **`.gitignore` 무시 정책 vs scan 범위** | 명시 — `.gitignore` + `--ignore` + 도구 내장 (`.git/`, `target/`, `node_modules/`) 합집합 | spec-classification.md / spec-ignore-policy.md |

## 정확 hash 재현 정책 상세

### Path 정규화 (NFD / case)

- **NFD → NFC**: 모든 path를 NFC로 정규화 후 비교 key 박음. `unicode-normalization` crate.
- **macOS default 환경** (`core.precomposeunicode = true`): NFD → NFC 자동 변환. 우리 NFC 정규화로 정합 [source: https://git.vger.kernel.narkive.com/OJcWG1uy/patch-v8-on-mac-os-and-precomposed-unicode].
- **macOS `false` edge case**: NFC/NFD 동일 path 두 개 박힌 vault → NFC 정규화 후 같은 key 충돌 → `Status::Failed` + `failed_reason: "nfd_collision"`. 99% 케이스는 NFC 정규화로 자동 처리, 1% edge case만 detect-only fallback.
- **Case 정책**: case-sensitive 비교 (Unix-style). Windows NTFS는 normalize 안 하고 NFC/NFD 둘 다 박힐 수 있음 [source: https://unicodefyi.com/guide/unicode-in-filenames/] — 우리 도구가 NFC 정규화 + case-sensitive로 통일.
- **Windows NTFS local-side case detection** (clean-context §1): local에 `Foo.txt` + `foo.txt` 두 file 박힌 case → walker가 두 entry catch (NTFS는 case-preserving), case-sensitive 비교로 두 path key 박음. 한쪽 side에 다른 case sibling이 있으면 unmatched side는 `Status::Failed` + `failed_reason: "case_collision"`로 promote (D1, spec-classification.md § edge case + spec-output-schema.md § `failed_reason` 정합 — `failed_reason`은 `Status::Failed` 한정). 검출은 symmetric: (a) canonical case-insensitive volume(1개 entry만 catch + remote 두 case 박음) → unmatched remote-side 1개 promote, (b) local 두 case 모두 박혀있는데 remote 1개 case → unmatched local-side promote, (c) local/remote가 서로 다른 case 1개씩만 박힌 diagonal mismatch → 양쪽 모두 promote.

### `.gitattributes` 정확 재현 (큰 변경)

- v0.1 정책: 항상 LF normalize (모든 텍스트 파일).
- v0.2 (Phase 5) 정책: **conditional LF normalize** + **화이트리스트**:

#### 지원 attribute 화이트리스트 (clean-context §1)

화이트리스트만 지원, 나머지는 `failed_reason: "gitattributes_unsupported"` 마크:

| attribute | 동작 |
|---|---|
| `text=auto` (명시) | 텍스트로 강제 — NUL 휴리스틱 무시, BOM 처리 + LF normalize |
| `binary` (명시) | raw bytes — NUL 휴리스틱 무시, normalize 안 함 |
| `eol=lf` (명시) | LF normalize (`\r\n` → `\n`) |
| `eol=crlf` (명시) | CRLF 보존 — `\r\n` → `\r\n` 그대로, GitHub 측 SHA와 정합 |
| 미명시 (default) | v0.1 정책 그대로 — NUL 휴리스틱 + BOM + LF normalize |

**미지원 (자동 fail mark)**:
- macro attributes (`[attr]binary -text -diff -merge`)
- `working-tree-encoding`
- `ident` / `filter` (smudge / clean filter)
- `text=auto eol=...` 복합 — eol만 적용, text=auto 전체는 화이트리스트 박힘
- `crlf` (legacy) — 화이트리스트 외

이유: 끝없는 git core attribute 정합 — yagni 차단 + 명시적 cover 범위.

#### 파서 (Phase 5 task K1)

- `shared/gitattributes.rs` 박음.
- working tree 한정 (.gitattributes 파일). `.git/info/attributes` / global 미지원 (spec-config.md § `.gitattributes` 위치).
- gitignore-style glob pattern matching.
- 우선순위: 가장 깊은 디렉토리의 `.gitattributes`가 우선 + line-level pattern은 마지막 매칭이 winner.

#### lifetime 계약 (clean-context §3, K2 박음)

```rust
pub fn prepare_for_hash(
    raw: &[u8],
    keep_bom: bool,
    gitattr: &Arc<GitAttributes>,  // shared, 단일 vault scan에서 1회 파싱 + 모든 파일 공유
) -> (Vec<u8>, bool);
```

매 호출마다 reparse 회귀 차단. `Arc<GitAttributes>` 권고 — `Option<&>` 또는 owned는 K2에서 명시 거부.

### 빈 파일 실파일 검증

- v0.1 unit test 통과 정책 그대로 (`SHA-1("blob 0\0") = e69de29bb2d1d6434b8b29ae775ad8c2e48c5391`).
- 추가: integration test fixture로 실제 0-byte 파일 박음 + local ↔ remote identical 분류 검증.

## detect-only 정책 상세

### Encoding 변환 시도 (clean-context §1 — hash 입력 (b) 정책)

- 1차: UTF-8 디코드 시도. 성공 시 `.gitattributes` 정합 normalize 적용.
- 2차 (실패 시): 다른 인코딩 detect (`encoding_rs` Mozilla, task E 결정).
- 3차 (실패 시): `Status::Failed` + `failed_reason: "encoding"`.

**Hash 입력 정책 (b)**: detect 성공해도 hash 입력은 **원본 raw bytes**. UTF-8로 변환된 bytes 박지 않음 — git core가 raw bytes 보존하기 때문 [source: https://www.codestudy.net/blog/how-to-determine-if-git-handles-a-file-as-binary-or-as-text/]. detect는 `failed_reason` 마크 + 사용자 정보 제공 용도만.

### Submodule / Symlink

- Trees mode `160000` (submodule) / `120000` (symlink) entry는 v0.1 비목표 (skip + warning, G-010).
- v0.2 (Phase 5): `Status::Failed` + `failed_reason: "submodule"` 또는 `"symlink"` + mode bit JSON 출력.
- 정확 hash 재현 안 함 (submodule은 외부 repo, symlink는 OS-dependent target).

### LFS pointer (clean-context §2 + advisor BLOCKING fix)

- git-lfs 표준 마커: `.gitattributes`에 `*.psd filter=lfs diff=lfs merge=lfs -text` 형식 [source: https://github.com/git-lfs/git-lfs/blob/main/docs/spec.md].
- **detection 경로 — `.gitattributes` 파싱 시점**:
  - `filter=lfs` 매칭 path는 LFS-tracked로 표시.
  - scan은 blob fetch 안 함 (Phase 4 GraphQL batching 이득 유지). LFS-tracked path는 자동 `Status::Failed` + `failed_reason: "lfs_pointer"` + `lfs_pointer: {oid: "?", size: 0}` 박음 (oid/size는 blob fetch 안 했으므로 unknown).
- **Defence-in-depth (옵션)**: `diff` 명령은 이미 blob fetch — pointer text 첫 줄 `version https://git-lfs.github.com/spec/v1` 시그니처 추가 검증 박음. oid + size 정확히 파싱.
- 처리: `Status::Failed` + `failed_reason: "lfs_pointer"` + `lfs_pointer: {oid, size}` JSON. 호출자가 LFS fetch 결정 입력으로 사용.

**왜 blob fetch 의존 제거**:
- scan은 Trees + Commits API만 호출 (per-file fetch는 차이 있는 path만). blob fetch는 N×subprocess + N×round-trip 추가 → Phase 4 batching 무효화.
- `.gitattributes` 1회 파싱으로 LFS-tracked 전부 detect 가능 → cost 0.
- diff 명령은 blob fetch 박혀있어 추가 시그니처 검증으로 pointer text 정확 파싱 가능.

### Windows long path / 예약 파일명

- 260자+ path: `\\?\` prefix 박지 않은 path는 Windows API 한도. local walker fail.
- 예약 파일명 (CON, PRN, NUL, AUX, COM1-9, LPT1-9): GitHub remote(Linux origin)에는 가능, Windows local에는 불가.
- 처리: `Status::Failed` + `failed_reason: "long_path"`.

### 실행 권한 mode bit

- Trees mode `100755` (executable) vs `100644` (regular)는 content 차이 아님.
- 정책: drift로 판정 안 함, content 같으면 `Status::Identical`. JSON `mode` 필드에 박음 (호출자가 mode 차이 정책 결정).

### BOM 처리

- UTF-8 BOM (`EF BB BF`): v0.1 정책 그대로 strip (text=auto + 미명시). `--keep-bom` 시 보존.
- UTF-16 BOM (`FF FE` LE / `FE FF` BE): detect → `Status::Failed` + `failed_reason: "encoding"` (UTF-16은 v0.2 비목표).

## v0.1 vs v0.2 회귀 정의 (clean-context §3 보강)

### 정확화 vs 회귀 분류

T task (vault dogfooding before/after)에서 v0.1 vs v0.2 출력 차이를 자동 분류:

**정확화 (의도된 변화 — 회귀 아님)**:
- v0.1에서 우연히 `Identical`로 박힌 binary가 v0.2에서 mismatch (`drift` 또는 `failed`)로 박힘 — `.gitattributes` 정확 재현으로 정확화.
- v0.1에서 LFS pointer를 raw text로 박아 mismatch한 entry가 v0.2에서 `failed_reason: "lfs_pointer"` 명시 박힘.
- v0.1에서 NFC/NFD 다른 path key로 박혀 false drift였던 entry가 v0.2 NFC 정규화로 `Identical` 박힘.

**회귀 (예상 외 변화 — 차단)**:
- 위 정확화 화이트리스트 외 status 변화.
- 새 `failed` 박힘인데 reason이 spec § failed_reason enum 외.
- schema_version 박혀있는데 v1.0 호출자 파싱 실패 (backward-compat 위반).

검증: W task (`docs/research/phase5-regression.md`)에서 정확화 화이트리스트 vs 회귀 자동 비교.

### Backward compatibility 적용 vault 범위

`.gitattributes` 미존재 vault (예: 우리 v0.1 vault 검증 baseline 356 files)에서:
- v0.1 정책 = v0.2 default 정책 (LF normalize + BOM strip).
- 결과 동일 — backward-compat 100% 유지.

`.gitattributes` 존재 vault에서:
- v0.1 결과 ≠ v0.2 결과 무조건 발생 (text=auto / binary / eol 분기 박음).
- 정확화로 분류, 회귀 아님.

## 검증 환경

### Windows 1차 + 실용 근사

- macOS NFD 검증은 raw bytes injection unit test fixture + **NTFS 실파일 fixture** 둘 다 가능 (clean-context §5 fact check):
  - NTFS는 normalize 안 함, UTF-16LE 그대로 박음 → NFD/NFC 파일 직접 생성 가능 [source: https://unicodefyi.com/guide/unicode-in-filenames/].
  - compose 한글 (`가` = `\u{AC00}`) vs decompose (`가` = `\u{1100}\u{1161}`) 둘 다 실파일 + raw bytes 시나리오 박음.
- symlink는 Windows에서 unprivileged process 박을 수 없음 — Trees API mock 응답에 mode `120000` entry 박아 검증.
- submodule도 mock 응답으로 검증 (실제 submodule 박지 않음).
- 인코딩 fixture는 byte literal로 EUC-KR / Shift_JIS / Latin-1 박음.
- LFS pointer fixture는 byte literal로 박음.

### 부속 검증 task

- **Y task**: `encoding_rs` binary size 사후 측정 (clean-context §5 — 정확 size impact 미확인). cargo-bloat dry-run + dependency tree 분석.
- **X task**: `.gitattributes` parser performance gate (큰 vault 10K+ × 100+ 룰 P95 측정).
- **R3 task**: large vault scale fixture (10K/100K 파일).

### Cross-platform CI는 Phase 5 범위 외

macOS/Linux runner 추가는 별도 phase. 현재 Windows runner 그대로.

## v0.1 baseline 영향

### task A 측정 결과 (2026-05-09)

- **vault 부재**: 머신 환경(`C:\Users\dasgut`)에서 vault path(`C:\Users\admin\iCloudDrive\iCloud~md~obsidian`) 접근 불가. dogfood target은 KneShell/gitless-sync 자체 repo (92 files) 한정 — Rust 프로젝트라 함정 surface ~0건 예상 + 실제로 그렇게 측정됨.
- **측정 결과**: 92 files (90 identical / 2 local_only_changed / 0 drift / 0 failed). 2건 local_only_changed는 scan 자체 stdout/stderr redirect race noise (도메인 함정 아님).
- **함정 0건 surface**: KneShell/gitless-sync는 NFD path / `.gitattributes` / LFS / submodule / symlink / 비-UTF-8 / Windows long path 모두 결여 — 함정 surface 측정 부적절.
- **상세**: `docs/research/phase5-vault-baseline.md`.

### 우선순위 박음 (이론 + spec 매핑 + fact check 3축)

vault 데이터 부재 + KneShell/gitless-sync surface 0건이라, 우선순위 입력은 (1) spec § 함정 처리 정책 매핑 + (2) task A fact check 3건 + (3) downstream task 결과 누적의 3축으로 박음.

**등급 정의**:
- **P0**: scan 정의 자체 — 함정 처리 외 (scan 범위 contract).
- **P1**: 일반 vault 운영(markdown / 미디어 / 다른 OS) 시 1순위 false drift 원인 또는 정확화 큰 변경.
- **P2**: 특수 vault 환경(EUC-KR / NTFS case-insensitive volume / depth 깊은 nested)에서 surface.
- **P3**: 일반 vault 0건 또는 detect-only로 충분, 정확 hash 재현 안 함.

| 등급 | 함정 | task A baseline | 우선순위 근거 |
|---|---|---|---|
| **P0** | `.gitignore` 무시 정책 | scan 정의 자체 (A2 task) | scan 범위 contract — 함정 처리 외 |
| **P1** | NFD vs NFC | NTFS NFC/NFD 공존 fact check 통과 (#3) | macOS↔Windows vault 운영 시 1순위 false drift 원인. NFC 정규화 hash 기반 |
| **P1** | `.gitattributes` | KneShell/gitless-sync 부재 | 화이트리스트 박힌 vault에서 1순위 정확화 (큰 변경 — conditional normalize) |
| **P1** | LFS pointer | 미surface | 미디어 vault에서 1순위 — `.gitattributes filter=lfs` 매칭 path 자동 fail mark |
| **P2** | 비-UTF-8 인코딩 | encoding_rs 미박힘 (size delta unknown — Y task) | EUC-KR vault에서 2순위 — 변환 시도 + raw bytes (b) hash |
| **P2** | NTFS case-insensitive 충돌 | KneShell/gitless-sync에 case 충돌 없음 | NTFS case-insensitive volume에서 1 entry 누락 (D1 task) |
| **P3** | submodule (`160000`) | 미surface | 일반 vault 0건 가능성 높음, detect-only 충분 |
| **P3** | symlink (`120000`) | 미surface (Windows unprivileged 박을 수 없음) | mock 검증으로 cover |
| **P3** | 실행 권한 (`100755` vs `100644`) | 미surface | content 같으면 Identical 유지, false drift 원인 아님 |
| **P3** | UTF-16 BOM | 미surface | UTF-8 BOM은 v0.1 처리, UTF-16은 detect-only |
| **P3** | Windows long path / 예약 파일명 | 미surface | depth 깊은 nested vault에서 surface 가능 |
| **N/A** | 빈 파일 | 미surface | unit test 이미 cover (`hash::tests::empty_blob_matches_git`) |

### 등급 vs implement 순서 분리

본 표의 P0~P3 등급은 **함정별 우선순위 정보**(vault 운영 시 false drift 영향 크기 + 정확화 필요성)이고, 실제 task implement 순서는 `docs/ralph/implementation-plan.md` § 의존 순서가 정의 (acceptance 의존 + spec cascade + fixture 박힘 시점). 등급과 implement 순서는 1대1 매핑 아님.

### Phase 5 baseline 진입점

- **v0.1 vault 검증** (356 파일, 0 drift, 2026-04-29, ureq): 당시 vault에 함정이 surface하지 않았던 이유 — vault markdown 위주 ASCII-friendly + v0.1 코드가 함정을 detect 못 해도 통과시키는 정책 두 효과 분리 측정 불가 (vault 접근 없이는). Phase 5 처리 후 vault dogfooding은 task T가 담당.
- **Phase 6 완료 시점 baseline**: 244 tests pass + tarpaulin 88.31% (project-ops.md § Coverage 게이트 80%).
- **Phase 5 진행 후 baseline**: vault dogfooding false drift 0건 (의도된 detect-only drift 제외) + 회귀 0건 (정확화 화이트리스트 외).

## Acceptance Criteria

(generic — task별 매핑은 `docs/ralph/implementation-plan.md`)

- 함정 처리 정책 implement 완료 (8 핵심 + 4 추가).
- 함정별 unit test fixture 박음 (Windows 환경 — raw bytes injection / NTFS 실파일 / mock 응답).
- vault dogfooding 통과 — false drift 0건 + 회귀 0건 (W task 자동 비교).
- 영향 받는 spec 갱신: `spec-hash-and-normalize.md` (큰 변경) + `spec-classification.md` + `spec-error-contracts.md` + `spec-output-schema.md` + `spec-config.md`.
- Phase 6 hard gate (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출) 모두 deny active 유지.
- tarpaulin 80% 게이트 유지 — Phase 5 새 코드 cover 정책 박음.
- CHANGELOG.md v0.2 박음 (V1 task).
