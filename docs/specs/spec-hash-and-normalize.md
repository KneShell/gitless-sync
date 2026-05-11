# Spec: Hash & Normalize

## 목적
로컬 파일과 원격 blob을 의미적 동일성("줄바꿈·BOM 차이는 동일로 간주") 기준으로 비교하기 위한 자체 정의 SHA-1 해시 계산. **git 표준 blob SHA가 아니다** (G-001).

> **Phase 5 갱신 (2026-05-09)**: v0.1 "항상 LF normalize" 정책에서 **`.gitattributes` 기반 conditional LF normalize**로 큰 구조 변경. 화이트리스트(text/binary/eol=lf|crlf만) + lifetime 계약(`Arc<GitAttributes>`) + encoding 변환 hash 입력 (b) 정책 적용.

## 현재 상태

- `crates/gitless-sync/src/shared/hash.rs::blob_hash` 구현 완료 (SHA-1 + git blob header 형식). empty blob 테스트 통과.
- `crates/gitless-sync/src/shared/normalize.rs::{is_binary, normalize_text, prepare_for_hash}` 구현 완료. `prepare_for_hash`는 K2(2026-05-09) 시점 `.gitattributes` 분기 라우팅 + 5 helper(`apply_text_auto`/`apply_binary`/`apply_eol_lf`/`apply_eol_crlf`/`apply_unspecified`) 분리 적용.
- **Phase 5 K1~K4 + K1.5 구현됨** (2026-05-09):
  - `.gitattributes` 파서 구현 (`shared/gitattributes/` 신규, K1).
  - `AttributeMatch` enum 정의됨 — `TextAuto / Binary / EolLf / EolCrlf / LfsPointer / Unspecified / Unsupported { attribute_name }` 7 variant (K1.5에서 `LfsPointer` variant 추가).
  - `prepare_for_hash` 시그니처 확정 — `(raw, keep_bom, gitattr: &Arc<GitAttributes>, path: &str) -> (Vec<u8>, bool)` 4 인자 (K2). 내부 분기는 7 variant → 5 helper 매핑(`LfsPointer`/`Unsupported`/`Unspecified`는 `apply_unspecified` 공유). caller(`pipeline/short_circuit::try_short_circuit_failed`)가 `.gitattributes` match arm에서 `LfsPointer` → `FailedReason::LfsPointer` + `Unsupported { .. }` → `FailedReason::GitattributesUnsupported`로 단락 (Phase 5.13 task AA에서 `is_lfs` predicate를 `classify_path` match 단일 arm으로 통합).
  - binary attribute 정확 적용 (NUL byte 휴리스틱 무시 + raw bytes 해시, K3).
  - `.gitattributes` 우선순위 (root < sub-dir depth < line-level last-wins) 정합 (K4).
  - encoding (b) 정책 + detector 구현 (`shared/decode.rs::try_decode_text` — UTF-16 BOM detect + non-UTF-8 shortlist). hash는 raw bytes 정합 검증 invariant test 통과. caller plumbing은 Phase 5.13 task AA에서 구현 — `commands/scan/hash_local.rs::try_hash_local`가 raw read 1회 시점에 `try_decode_text` 결과를 분기 (`Utf16Bom { .. }` / `Unknown` → `Some(FailedReason::Encoding)` 반환) + `pipeline::build_one_pre_entry`가 `PreState::Failed` 격상.

## 작업 범위

### 해시 정의

- 텍스트: `SHA-1("blob <size>\0<.gitattributes-conditional normalized content>")`
- 바이너리: `SHA-1("blob <size>\0<raw bytes>")`
- size는 normalize 후의 바이트 크기.

### Normalize 규칙

#### v0.1 Baseline (Phase 5 변경 전 정책)

1. **바이너리 판별**: 첫 8000 바이트 안에 NUL 바이트 (`0x00`)가 있으면 바이너리. 바이너리는 normalize 안 함.
2. **BOM 처리**: 텍스트가 UTF-8 BOM (`EF BB BF`)으로 시작하면 제거. 단 `--keep-bom` 플래그 시 보존.
3. **개행 정규화**: `\r\n` → `\n`. 단독 `\r`은 그대로 둠.

#### Phase 5 — `.gitattributes` Conditional Normalize

`.gitattributes` 매칭 결과로 분기:

| 매칭 attribute | normalize 정책 |
|---|---|
| `text=auto` (명시, **화이트리스트 ✓**) | 텍스트로 강제 — NUL 휴리스틱 무시, BOM 처리 + LF normalize |
| `binary` (명시, **화이트리스트 ✓**) | raw bytes — NUL 휴리스틱 무시, normalize 안 함 |
| `eol=lf` (명시, **화이트리스트 ✓**) | LF normalize (`\r\n` → `\n`) |
| `eol=crlf` (명시, **화이트리스트 ✓**) | CRLF 보존 — `\r\n` → `\r\n` 그대로, `\n` → `\r\n` 변환 안 함 (GitHub 측 SHA와 일치) |
| **`filter=lfs`** (명시, **화이트리스트 ✓**) | LFS-tracked 마커 — `Status::Failed` + `failed_reason: "lfs_pointer"` + `lfs_pointer: {oid, size}` (scan은 unknown, diff는 정확 파싱). `AttributeMatch::LfsPointer` variant. |
| 미명시 (default) | v0.1 정책 그대로 — NUL 휴리스틱 + BOM + LF normalize |
| **화이트리스트 외** (예: `working-tree-encoding`, `ident`, `filter=*` (lfs 외), macro attributes, `crlf` legacy) | `Status::Failed` + `failed_reason: "gitattributes_unsupported"` (spec-domain-pitfalls.md § 지원 attribute 화이트리스트) |

#### Phase 5 — 인코딩 변환 시도 (비-UTF-8)

`prepare_for_hash`가 텍스트로 판별한 후:

1. 1차 UTF-8 디코드 시도 → 성공 시 `.gitattributes` 정합 normalize 적용.
2. 실패 시 다른 인코딩 detect (`encoding_rs` Mozilla, task E 결정 + Y task의 binary size 측정 결과 활용).
3. 변환 실패 시 `Status::Failed` + `failed_reason: "encoding"`.

**Hash 입력 정책 (b)**:
- detect 성공해도 hash 입력은 **원본 raw bytes**. UTF-8로 변환된 bytes 사용 안 함.
- 근거: git core가 raw bytes 보존 — UTF-8 변환 hash는 git core와 mismatch.
- detect는 `failed_reason` 마크 + 사용자 정보 제공 용도. hash 정확성과 무관.

### BOM 처리 (v0.2)

UTF-8 BOM과 UTF-16 BOM을 분기 처리:

| BOM | 처리 |
|---|---|
| UTF-8 BOM (`EF BB BF`) | v0.1 정책 그대로 — `text=auto` + 미명시에서 strip. `--keep-bom` 시 보존. `normalize_text`가 담당. |
| UTF-16 LE BOM (`FF FE`) | detect → `Status::Failed` + `failed_reason: "encoding"` (UTF-16은 v0.2 비목표). |
| UTF-16 BE BOM (`FE FF`) | detect → `Status::Failed` + `failed_reason: "encoding"` (UTF-16은 v0.2 비목표). |

#### 호출 지점

- `try_decode_text` (`shared/decode.rs`)가 UTF-16 BOM 검사 진입점. UTF-16 BOM detected → `TextDecodeResult::Utf16Bom { little_endian: bool }` variant 반환.
- caller-side `Utf16Bom`/`Unknown` variant → `failed_reason: "encoding"` 매핑은 Phase 5.13 task AA에서 구현 — `commands/scan/hash_local.rs::try_hash_local`가 raw read 1회 시점에 `try_decode_text` 결과를 분기해 `Some(FailedReason::Encoding)` 반환 + `pipeline::build_one_pre_entry`가 `PreState::Failed` 격상. cascade는 nfd_collision / case_collision / long_path / submodule / symlink / lfs_pointer / gitattributes_unsupported 7 분기 + encoding (hash_local 단계)으로 8 reason 모두 surface.
- `try_decode_text`는 production code(`hash_local.rs`)에서 호출됨 (Phase 5.13 task AA). detector + decode 결과 invariant test (`utf16_bom_passes_through_unchanged_for_hashing_and_normalize`)는 raw bytes 정합 검증으로 회귀 가드.
- UTF-8 BOM 처리는 `normalize_text`가 담당 (v0.1 그대로).

#### 우선순위

`try_decode_text`는 UTF-16 BOM 검사를 UTF-8 디코드 시도 **앞**에 배치한다. UTF-16 BOM은 첫 2바이트만으로 분기 가능 + UTF-8 디코드는 BOM 자체가 invalid byte sequence라 자연 fall-through되지만, 명시적 BOM 분기로 detection 정보(LE vs BE) 보존.

UTF-8 BOM (`EF BB BF`)은 첫 3바이트가 valid UTF-8 (U+FEFF)이므로 `try_decode_text`는 `Utf8`로 분류. strip 처리는 `normalize_text` 책임.

### `.gitattributes` 정확 재현

#### 파서 (Phase 5 task K1)

- `shared/gitattributes/` 구현.
- **working tree 한정** (`.gitattributes` 파일). `.git/info/attributes` / global 미지원 (spec-config.md § `.gitattributes` 위치).
- project root + 하위 디렉토리의 `.gitattributes` 파일 1회 로드.
- gitignore-style glob pattern matching.
- 우선순위: 가장 깊은 디렉토리의 `.gitattributes`가 우선 + line-level pattern은 위에서 아래로 순회 (마지막 매칭이 winner).

#### Lifetime 계약

```rust
pub struct GitAttributes {
    files: Vec<AttributesFile>,  // project root + sub-dir 로드 결과, depth 정렬
}

pub(crate) fn prepare_for_hash(
    raw: &[u8],
    keep_bom: bool,
    gitattr: &Arc<GitAttributes>,  // 단일 vault scan 1회 파싱 + 모든 파일 공유
    path: &str,                    // working-tree-relative + forward slash, K1.5 classify_path 입력
) -> (Vec<u8>, bool);
```

- 단일 vault scan에서 1회 파싱 + 모든 파일 공유. 매 호출 reparse 회귀 차단.
- `Arc<GitAttributes>` 권고 — `Option<&>` (모든 호출 lifetime 결합) / owned (clone 비용 큼) 둘 다 K2에서 명시 거부.
- `path: &str`는 `gitattr.classify_path(path)` 입력 — 매 파일별 attribute 매핑이 필요하므로 시그니처에 포함.
- `commands/scan/mod.rs`가 vault root 진입 시 1회 `Arc::new(GitAttributes::load(local_root)?)` 호출 (`shared/normalize.rs::prepare_for_hash` → `commands/scan/hash_local.rs::try_hash_local` → `commands/scan/pipeline::assemble_entries` 경로로 reference 전파).

#### 화이트리스트 강제

`.gitattributes` 매칭 결과 외 attribute는 자동 fail:

```rust
pub enum AttributeMatch {
    TextAuto,
    Binary,
    EolLf,
    EolCrlf,
    Unspecified,  // default = v0.1 정책
    Unsupported {  // 화이트리스트 외
        attribute_name: String,
    },
}
```

`Unsupported` 매칭 시 `Status::Failed` + `failed_reason: "gitattributes_unsupported"` + JSON에 `attribute_name` 포함 (사용자가 fix 가능 정보).

#### Binary attribute 정확 적용 (Phase 5 task K3)

- `binary` 명시된 file은 NUL byte 휴리스틱 무시 + raw bytes 해시.
- 효과: NUL byte 없는 binary file (예: 일부 image format header)이 정확 binary로 분류.

### 원격 측 비교

GitHub Trees API가 반환하는 blob SHA는 working tree 바이트의 해시이며 `core.autocrlf` / `.gitattributes` 영향을 받음. 본 도구는 v0.2 (Phase 5)부터 `.gitattributes`를 파싱해 동일 정책 적용 → 자체 SHA 재계산해서 비교한다. **Trees API SHA는 무시** (자체 정의 hash 정책 그대로).

### Phase 7 — 큰 파일 처리

> **공식 한도** (2026-05-10 fact check, [source: https://docs.github.com/en/rest/git/blobs]): GitHub Git Blobs API는 100MB 단일 파일 hard limit. v0.3부터 본 한도를 강제 + tool 메모리 임계 (50MB) 별도 분리.

#### 한도 정의

| reason | 임계치 | 검출 시점 | 처리 |
|---|---|---|---|
| `file_too_large` | 100 MB | local: `fs::metadata().len()` pre-flight + remote: Trees response size field pre-flight | `Status::Failed` + `failed_reason: "file_too_large"` + `size_bytes` field |
| `memory_exceeded` | 50 MB | local: `fs::metadata().len()` pre-flight + remote: Trees response size field pre-flight | `Status::Failed` + `failed_reason: "memory_exceeded"` + `size_bytes` field |

근거:
- 100 MB = GitHub Blobs API hard limit (fact check 2026-05-10). 100 MB 초과 파일은 도구 비교 불가 — remote 자체 fetch 불가능.
- 50 MB = tool 메모리 안전 임계. raw bytes + base64 encoded + SHA-1 buffer 3중 메모리 사용 가정 → 50 MB raw → 약 200 MB 메모리 worst case. 1 GB RAM 머신 안전 cap. 측정 + 조정은 ADR 0012.

#### 우선순위 (cascade 순서)

`pipeline::try_short_circuit_failed` cascade에서 다음 순서 적용 (앞 reason이 win):

1. `nfd_collision` (기존)
2. `case_collision` (기존)
3. `long_path` (기존)
4. `submodule` (기존)
5. `symlink` (기존)
6. `lfs_pointer` (기존, Phase 5)
7. `gitattributes_unsupported` (기존, Phase 5)
8. **`file_too_large` (Phase 7 신규, 100MB 우선)**
9. **`memory_exceeded` (Phase 7 신규, 50MB)**

근거:
- LFS pointer가 size check보다 우선 — 100MB 미만 LFS pointer text가 Phase 5 spec대로 detect되어야 함 (raw pointer text가 본문 size 측정 시 30MB라도 lfs_pointer로 분류).
- file_too_large > memory_exceeded — 둘 다 size 기반이지만 100MB 초과는 remote fetch 자체 불가 (더 fatal). 50MB 초과는 tool 메모리 한정.

#### 검출 알고리즘

```rust
const FILE_TOO_LARGE_BYTES: u64 = 100 * 1024 * 1024;
const MEMORY_EXCEEDED_BYTES: u64 = 50 * 1024 * 1024;

fn try_hash_local_with_size_gate(path: &Path) -> Result<HashOutcome, FailedReason> {
    let meta = fs::metadata(path)?;
    let size = meta.len();

    // pre-flight: 100 MB 초과 → 즉시 fail (file read 자체 회피)
    if size > FILE_TOO_LARGE_BYTES {
        return Err(FailedReason::FileTooLarge);
    }
    // pre-flight: 50 MB 초과 → 즉시 fail (메모리 임계 사전 차단)
    if size > MEMORY_EXCEEDED_BYTES {
        return Err(FailedReason::MemoryExceeded);
    }

    // 정상 path: raw read + hash
    let raw = fs::read(path)?;
    Ok(hash(raw))
}

fn fetch_blob_with_size_gate(client, repo, sha, expected_size: u64) -> Result<Vec<u8>, GitlessError> {
    // pre-flight: Trees response size field 사용 (Trees response는 size 항상 포함)
    if expected_size > FILE_TOO_LARGE_BYTES {
        return Err(GitlessError::Http(format!("blob {sha} too large: {expected_size} bytes")));
    }
    if expected_size > MEMORY_EXCEEDED_BYTES {
        return Err(GitlessError::Http(format!("blob {sha} exceeds memory threshold: {expected_size} bytes")));
    }

    // 정상 path: gh api 호출 + base64 디코드
    fetch_blob(client, repo, sha)
}
```

#### LFS pointer 분기 (Phase 5 spec 정합)

- 100 MB 미만 LFS pointer text는 본 § 임계치 통과 (size pre-flight 정상). 별도 LFS detection은 spec-domain-pitfalls.md § LFS pointer + spec-hash-and-normalize.md § LFS pointer 그대로 처리.
- LFS pointer detect 우선순위: LFS check → size check (cascade에서 LFS가 먼저 short-circuit). 큰 파일 size check는 LFS 미감지 entry에 한해 적용.

#### 단위 테스트 시나리오

- `[AUTO]` 50 MB 직전 (예: 49 MB) local file → 정상 hash + Status::Identical (remote 동일 가정).
- `[AUTO]` 50 MB 직후 (예: 51 MB) local file → Status::Failed + failed_reason: "memory_exceeded" + size_bytes: 53477376.
- `[AUTO]` 100 MB 직후 (예: 101 MB) local file → Status::Failed + failed_reason: "file_too_large" + size_bytes: 105906176.
- `[AUTO]` Trees response size field 50 MB 초과 entry → fetch_blob 호출 0회 + Status::Failed + failed_reason: "memory_exceeded" (pre-flight skip).
- `[AUTO]` 30 MB LFS pointer text file → Status::Failed + failed_reason: "lfs_pointer" (LFS 우선순위, size check 미실행).

### 함정 (G-001)

- empty blob (`e69de29bb2d1d6434b8b29ae775ad8c2e48c5391`)이 git 상수와 일치하는 건 우연. 다른 파일은 일치 보장 안 됨.
- 호출자가 `git hash-object` 결과와 비교하지 않도록 문서로 강조 (CLAUDE.md + 본 spec).

## Acceptance Criteria

### v0.1 baseline 시나리오 (Phase 5 후에도 통과 유지)

- `[AUTO]` `hash::blob_hash(&[])` == `"e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"` (이미 통과).
- `[AUTO]` PRD 검증 시나리오 5: CRLF로 저장된 로컬 파일과 LF로 저장된 동일 내용 원격 blob의 자체 SHA가 일치 (단위 테스트로) — `.gitattributes` 미명시 기본 정책 적용.
- `[AUTO]` PRD 검증 시나리오 6: BOM 있는 로컬과 BOM 없는 원격이 `--keep-bom` 미지정 시 동일 SHA.
- `[AUTO]` PRD 검증 시나리오 7: `--keep-bom` 시 BOM 차이가 다른 SHA를 만든다.
- `[AUTO]` PRD 검증 시나리오 8: 동일 바이너리 파일은 동일 SHA (raw byte 해시).
- `[AUTO]` `normalize::is_binary(&[0u8, 1, 2])` == `true`. `normalize::is_binary(b"plain text")` == `false`.

### Phase 5 시나리오 (`.gitattributes` 정확 재현 + 화이트리스트)

- `[AUTO]` `.gitattributes`에 `*.txt text=auto` 적용 상태에서 NUL byte 없는 텍스트 파일이 LF normalize 적용 (기존 휴리스틱과 동일).
- `[AUTO]` `.gitattributes`에 `*.bin binary` 적용 상태에서 NUL byte 0개 file도 binary 취급 (raw bytes 해시).
- `[AUTO]` `.gitattributes`에 `*.txt eol=crlf` 적용 상태에서 LF로 저장된 로컬 파일과 CRLF로 저장된 원격 blob이 다른 SHA.
- `[AUTO]` `.gitattributes`에 `*.txt eol=lf` 적용 상태에서 CRLF/LF 차이 무시.
- `[AUTO]` 가장 깊은 디렉토리 `.gitattributes`가 root보다 우선.
- `[AUTO]` `.gitattributes` 미존재 시 v0.1 정책 그대로 적용.
- `[AUTO]` 화이트리스트 외 attribute (예: `*.foo working-tree-encoding=UTF-16`) 매칭 시 `GitAttributes::classify_path` 결과가 `AttributeMatch::Unsupported { attribute_name: "working-tree-encoding" }`. caller-side `Status::Failed` + `failed_reason: "gitattributes_unsupported"` 매핑은 Phase 5.13 task AA에서 구현 — `pipeline::try_short_circuit_failed`의 `.gitattributes` match arm이 `Unsupported { .. }` → `FailedReason::GitattributesUnsupported` (`prepare_for_hash`는 v0.1 default fall-through 그대로 — defensive, caller가 short-circuit으로 surface).

### Phase 5 시나리오 (Lifetime 계약)

- `[AUTO]` 단일 vault scan에서 `GitAttributes::load(root)` 1회 호출 + `prepare_for_hash` N번 호출 시 reparse 0회 (lifetime 계약 — `&Arc<GitAttributes>` 시그니처가 reparse를 컴파일러 차원에서 차단). `lifetime_contract_one_load_n_calls_no_clone_leak` 테스트가 N=5 호출 후 `Arc::strong_count == 1` 검증.
- `[AUTO]` `Arc<GitAttributes>` 시그니처가 future shared access 보장 — 현재 `commands/scan/pipeline/hash_pass::build_pre_entries`는 sequential `.iter().map()` (REST rayon backend는 commits API 한정, ADR 0003; GraphQL backend는 rayon 미사용, ADR 0005). cross-thread clone 시나리오는 1000+ path scale에서 hash pass 병렬화 필요 시 활성화 — `Arc::clone()` 호출 변경 0건.

### Phase 5 시나리오 (인코딩 변환 — hash 입력 (b))

- `[AUTO]` UTF-8 valid 파일은 1차 디코드 통과 + LF normalize.
- `[AUTO]` EUC-KR 인코딩 파일은 1차 fail → 2차 detect 통과 → **hash 입력은 원본 raw bytes** (변환된 UTF-8 사용 안 함).
- `[AUTO]` EUC-KR 동일 파일이 local + remote 둘 다 존재하면 `Status::Identical` (raw bytes 동일).
- `[AUTO]` 인코딩 detect 실패 시 `Status::Failed` + `failed_reason: "encoding"`.

### Phase 5 시나리오 (BOM)

- `[AUTO]` UTF-8 BOM 처리는 v0.1 그대로 (`--keep-bom` 미명시 시 strip).
- `[AUTO]` UTF-16 BOM (`FF FE` 또는 `FE FF`) detect 시 `Status::Failed` + `failed_reason: "encoding"`.

### Phase 5 시나리오 (NFD path — Windows NTFS 실파일 + raw bytes injection 둘 다)

- `[AUTO]` raw bytes injection — NFD 한글 path (`\u{1100}\u{1161}.txt`)가 NFC 정규화 (`\u{AC00}.txt`) 후 동일 path key.
- `[AUTO]` Windows NTFS 실파일 fixture — NFD/NFC 둘 다 실제 파일 생성 + walker가 NFC 정규화 후 동일 key.

### Phase 5 시나리오 (빈 파일 실파일)

- `[AUTO]` integration test에서 0-byte 파일 생성 + local ↔ remote empty blob → `Status::Identical`.
