# Spec: Hash & Normalize

## 목적
로컬 파일과 원격 blob을 의미적 동일성("줄바꿈·BOM 차이는 동일로 간주") 기준으로 비교하기 위한 자체 정의 SHA-1 해시 계산. **git 표준 blob SHA가 아니다** (G-001).

> **Phase 5 갱신 (2026-05-09, vague + clean-context 외부 시각 보강)**: v0.1 "항상 LF normalize" 정책에서 **`.gitattributes` 기반 conditional LF normalize**로 큰 구조 변경. 화이트리스트(text/binary/eol=lf|crlf만) + lifetime 계약(`Arc<GitAttributes>`) + encoding 변환 hash 입력 (b) 정책 박음.

## 현재 상태

- `crates/gitless-sync/src/shared/hash.rs::blob_hash` 구현 완료 (SHA-1 + git blob header 형식). empty blob 테스트 통과.
- `crates/gitless-sync/src/shared/normalize.rs::{is_binary, normalize_text, prepare_for_hash}` 구현 완료 (v0.1 항상 LF normalize 정책).
- **Phase 5에서 갱신 예정**:
  - `.gitattributes` 파서 추가 (`shared/gitattributes.rs` 신규).
  - `prepare_for_hash` 시그니처 변경 — `gitattr: &Arc<GitAttributes>` 인자 추가 (lifetime 계약).
  - conditional LF normalize 박음 (text=auto / binary / eol=lf / eol=crlf / 미명시 5 분기).
  - 화이트리스트 박음 — 외 attribute는 `failed_reason: "gitattributes_unsupported"` 마크.
  - encoding 변환 hash 입력 (b) 정책 — detect는 reason만, hash는 raw bytes.

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
| **`filter=lfs`** (명시, **화이트리스트 ✓**, advisor BLOCKING fix) | LFS-tracked 마커 — `Status::Failed` + `failed_reason: "lfs_pointer"` + `lfs_pointer: {oid, size}` (scan은 unknown, diff는 정확 파싱). `AttributeMatch::LfsPointer` variant. |
| 미명시 (default) | v0.1 정책 그대로 — NUL 휴리스틱 + BOM + LF normalize |
| **화이트리스트 외** (예: `working-tree-encoding`, `ident`, `filter=*` (lfs 외), macro attributes, `crlf` legacy) | `Status::Failed` + `failed_reason: "gitattributes_unsupported"` (spec-domain-pitfalls.md § 지원 attribute 화이트리스트) |

#### Phase 5 — 인코딩 변환 시도 (비-UTF-8)

`prepare_for_hash`가 텍스트로 판별한 후:

1. 1차 UTF-8 디코드 시도 → 성공 시 `.gitattributes` 정합 normalize 적용.
2. 실패 시 다른 인코딩 detect (`encoding_rs` Mozilla, task E 결정 + Y task 박음 binary size 측정).
3. 변환 실패 시 `Status::Failed` + `failed_reason: "encoding"`.

**Hash 입력 정책 (b)** (clean-context §1):
- detect 성공해도 hash 입력은 **원본 raw bytes**. UTF-8로 변환된 bytes 박지 않음.
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

- `try_decode_text` (`shared/normalize.rs`)가 UTF-16 BOM 검사 진입점. UTF-16 BOM detected → `TextDecodeResult::Utf16Bom { little_endian: bool }` variant 반환.
- caller (`compare.rs` / `gitattributes` 매핑, K1.5 / K2 단계)는 `Utf16Bom` variant를 `failed_reason: "encoding"`으로 매핑.
- UTF-8 BOM 처리는 `normalize_text`가 담당 (v0.1 그대로).

#### 우선순위

`try_decode_text`는 UTF-16 BOM 검사를 UTF-8 디코드 시도 **앞**에 박는다. UTF-16 BOM은 첫 2바이트만으로 분기 가능 + UTF-8 디코드는 BOM 자체가 invalid byte sequence라 자연 fall-through되지만, 명시적 BOM 분기로 detection 정보(LE vs BE) 보존.

UTF-8 BOM (`EF BB BF`)은 첫 3바이트가 valid UTF-8 (U+FEFF)이므로 `try_decode_text`는 `Utf8`로 분류. strip 처리는 `normalize_text` 책임.

### `.gitattributes` 정확 재현

#### 파서 (Phase 5 task K1)

- `shared/gitattributes.rs` 박음.
- **working tree 한정** (`.gitattributes` 파일). `.git/info/attributes` / global 미지원 (spec-config.md § `.gitattributes` 위치).
- project root + 하위 디렉토리의 `.gitattributes` 파일 1회 로드.
- gitignore-style glob pattern matching.
- 우선순위: 가장 깊은 디렉토리의 `.gitattributes`가 우선 + line-level pattern은 위에서 아래로 박힘 (마지막 매칭이 winner).

#### Lifetime 계약 (clean-context §3, K2 박음)

```rust
pub struct GitAttributes {
    rules: Vec<AttributeRule>,
    // ... project root + sub-dir 로드 결과 통합
}

pub fn prepare_for_hash(
    raw: &[u8],
    keep_bom: bool,
    gitattr: &Arc<GitAttributes>,  // 단일 vault scan 1회 파싱 + 모든 파일 공유
) -> (Vec<u8>, bool);
```

- 단일 vault scan에서 1회 파싱 + 모든 파일 공유. 매 호출 reparse 회귀 차단.
- `Arc<GitAttributes>` 권고 — `Option<&>` (모든 호출 lifetime 결합) / owned (clone 비용 큼) 둘 다 K2 박지 않음.
- `walker.rs` 또는 `scan/pipeline.rs`가 vault root 진입 시 1회 `Arc::new(GitAttributes::load(root)?)` 박음.

#### 화이트리스트 강제 (clean-context §1, K1.5 sub-task)

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

`Unsupported` 매칭 시 `Status::Failed` + `failed_reason: "gitattributes_unsupported"` + JSON에 `attribute_name` 박음 (사용자가 fix 가능 정보).

#### Binary attribute 정확 적용 (Phase 5 task K3)

- `binary` 명시된 file은 NUL byte 휴리스틱 무시 + raw bytes 해시.
- 효과: NUL byte 없는 binary file (예: 일부 image format header)이 정확 binary로 분류.

### 원격 측 비교

GitHub Trees API가 반환하는 blob SHA는 working tree 바이트의 해시이며 `core.autocrlf` / `.gitattributes` 영향을 받음. 본 도구는 v0.2 (Phase 5)부터 `.gitattributes`를 파싱해 동일 정책 적용 → 자체 SHA 재계산해서 비교한다. **Trees API SHA는 무시** (자체 정의 hash 박는 정책 그대로).

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

- `[AUTO]` `.gitattributes`에 `*.txt text=auto` 박힌 상태에서 NUL byte 없는 텍스트 파일이 LF normalize 적용 (기존 휴리스틱과 동일).
- `[AUTO]` `.gitattributes`에 `*.bin binary` 박힌 상태에서 NUL byte 0개 file도 binary 취급 (raw bytes 해시).
- `[AUTO]` `.gitattributes`에 `*.txt eol=crlf` 박힌 상태에서 LF로 저장된 로컬 파일과 CRLF로 저장된 원격 blob이 다른 SHA.
- `[AUTO]` `.gitattributes`에 `*.txt eol=lf` 박힌 상태에서 CRLF/LF 차이 무시.
- `[AUTO]` 가장 깊은 디렉토리 `.gitattributes`가 root보다 우선.
- `[AUTO]` `.gitattributes` 미존재 시 v0.1 정책 그대로 적용.
- `[AUTO]` 화이트리스트 외 attribute (예: `*.foo working-tree-encoding=UTF-16`) 매칭 시 `Status::Failed` + `failed_reason: "gitattributes_unsupported"` + `attribute_name: "working-tree-encoding"` 박음.

### Phase 5 시나리오 (Lifetime 계약)

- `[AUTO]` 단일 vault scan에서 `GitAttributes::load(root)` 1회 호출 + `prepare_for_hash` N번 호출 시 reparse 0회 (counter 검증).
- `[AUTO]` `Arc<GitAttributes>` clone 박음 — 모든 worker thread (rayon backend)에서 공유.

### Phase 5 시나리오 (인코딩 변환 — hash 입력 (b))

- `[AUTO]` UTF-8 valid 파일은 1차 디코드 통과 + LF normalize.
- `[AUTO]` EUC-KR 인코딩 파일은 1차 fail → 2차 detect 통과 → **hash 입력은 원본 raw bytes** (변환된 UTF-8 박지 않음).
- `[AUTO]` EUC-KR 동일 파일이 local + remote 둘 다 박혀있으면 `Status::Identical` (raw bytes 동일).
- `[AUTO]` 인코딩 detect 실패 시 `Status::Failed` + `failed_reason: "encoding"`.

### Phase 5 시나리오 (BOM)

- `[AUTO]` UTF-8 BOM 처리는 v0.1 그대로 (`--keep-bom` 미명시 시 strip).
- `[AUTO]` UTF-16 BOM (`FF FE` 또는 `FE FF`) detect 시 `Status::Failed` + `failed_reason: "encoding"`.

### Phase 5 시나리오 (NFD path — Windows NTFS 실파일 + raw bytes injection 둘 다)

- `[AUTO]` raw bytes injection — NFD 한글 path (`\u{1100}\u{1161}.txt`)가 NFC 정규화 (`\u{AC00}.txt`) 후 동일 path key.
- `[AUTO]` Windows NTFS 실파일 fixture — NFD/NFC 둘 다 실제 파일 박음 + walker가 NFC 정규화 후 동일 key.

### Phase 5 시나리오 (빈 파일 실파일)

- `[AUTO]` integration test에서 0-byte 파일 박음 + local ↔ remote empty blob → `Status::Identical`.
