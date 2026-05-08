# Spec: Hash & Normalize

## 목적
로컬 파일과 원격 blob을 의미적 동일성("줄바꿈·BOM 차이는 동일로 간주") 기준으로 비교하기 위한 자체 정의 SHA-1 해시 계산. **git 표준 blob SHA가 아니다** (G-001).

> **Phase 5 갱신 (2026-05-09)**: v0.1 "항상 LF normalize" 정책에서 **`.gitattributes` 기반 conditional LF normalize**로 큰 구조 변경. 본 spec § Normalize 규칙 + § `.gitattributes` 정확 재현 갱신.

## 현재 상태

- `crates/gitless-sync/src/shared/hash.rs::blob_hash` 구현 완료 (SHA-1 + git blob header 형식). empty blob 테스트 통과.
- `crates/gitless-sync/src/shared/normalize.rs::{is_binary, normalize_text, prepare_for_hash}` 구현 완료 (v0.1 항상 LF normalize 정책).
- **Phase 5에서 갱신 예정**:
  - `.gitattributes` 파서 추가 (`shared/gitattributes.rs` 신규).
  - `prepare_for_hash` 시그니처 변경 — `.gitattributes` 매칭 결과 인자 추가.
  - conditional LF normalize 박음 (text=auto / binary / eol=lf / eol=crlf 분기).

## 작업 범위

### 해시 정의

- 텍스트: `SHA-1("blob <size>\0<.gitattributes-conditional normalized content>")`
- 바이너리: `SHA-1("blob <size>\0<raw bytes>")`
- size는 normalize 후의 바이트 크기.

### Normalize 규칙

#### v0.1 Baseline (Phase 5 변경 전 정책)

1. **바이너리 판별**: 첫 8000 바이트 안에 NUL 바이트 (`0x00`)가 있으면 바이너리. 바이너리는 normalize 안 함.
2. **BOM 처리**: 텍스트가 UTF-8 BOM (`EF BB BF`)으로 시작하면 제거. 단 `--keep-bom` 플래그 시 보존.
3. **개행 정규화**: `\r\n` → `\n`. 단독 `\r`은 그대로 둠 (Mac Classic 시대 유물).

#### Phase 5 — `.gitattributes` Conditional Normalize

`.gitattributes` 매칭 결과로 분기:

| 매칭 attribute | normalize 정책 |
|---|---|
| `text=auto` (명시) | 텍스트로 강제 — NUL 휴리스틱 무시, BOM 처리 + LF normalize |
| `binary` (명시) | raw bytes — NUL 휴리스틱 무시, normalize 안 함 |
| `eol=lf` (명시) | LF normalize (`\r\n` → `\n`) |
| `eol=crlf` (명시) | CRLF 보존 — `\r\n` → `\r\n` 그대로, `\n` → `\r\n` 변환 안 함 (GitHub 측 SHA와 일치) |
| 미명시 (default) | v0.1 정책 그대로 — NUL 휴리스틱 + BOM + LF normalize |

#### Phase 5 — 인코딩 변환 시도 (비-UTF-8)

`prepare_for_hash`가 텍스트로 판별한 후:

1. 1차 UTF-8 디코드 시도 → 성공 시 LF normalize 적용 (또는 `.gitattributes` 정합).
2. 실패 시 다른 인코딩 detect (`encoding_rs` 또는 동등 라이브러리, task E에서 결정).
3. 변환 실패 시 Status::Failed + reason "encoding" + binary 취급 (G-006 정책 계승).

### `.gitattributes` 정확 재현

#### 파서 (Phase 5 task K1)

- `shared/gitattributes.rs` 박음.
- project root + 하위 디렉토리의 `.gitattributes` 파일 1회 로드.
- gitignore-style glob pattern matching.
- 우선순위: 가장 깊은 디렉토리의 `.gitattributes`가 우선 + line-level pattern은 위에서 아래로 박힘 (마지막 매칭이 winner).

#### 적용 (Phase 5 task K2)

- `prepare_for_hash` 시그니처 변경:
  ```rust
  pub fn prepare_for_hash(
      raw: &[u8],
      keep_bom: bool,
      gitattr: &GitAttributesMatch,  // 신규 인자
  ) -> (Vec<u8>, bool);
  ```
- `GitAttributesMatch`는 path별 매칭 결과 enum (TextAuto / Binary / EolLf / EolCrlf / Unspecified).

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

### Phase 5 시나리오 (`.gitattributes` 정확 재현)

- `[AUTO]` `.gitattributes`에 `*.txt text=auto` 박힌 상태에서 NUL byte 없는 텍스트 파일이 LF normalize 적용 (기존 휴리스틱과 동일).
- `[AUTO]` `.gitattributes`에 `*.bin binary` 박힌 상태에서 NUL byte 0개 file도 binary 취급 (raw bytes 해시).
- `[AUTO]` `.gitattributes`에 `*.txt eol=crlf` 박힌 상태에서 LF로 저장된 로컬 파일과 CRLF로 저장된 원격 blob이 다른 SHA — `eol=crlf`는 변환 안 함, GitHub 측과 정합.
- `[AUTO]` `.gitattributes`에 `*.txt eol=lf` 박힌 상태에서 CRLF/LF 차이 무시 (LF로 통일).
- `[AUTO]` 가장 깊은 디렉토리 `.gitattributes`가 root보다 우선 (예: `subdir/.gitattributes`의 `*.txt binary`가 root의 `*.txt text` 덮어씀).
- `[AUTO]` `.gitattributes` 미존재 시 v0.1 정책 그대로 적용 (모든 텍스트 LF normalize + BOM 제거).

### Phase 5 시나리오 (인코딩 변환)

- `[AUTO]` UTF-8 valid 파일은 1차 디코드 통과 + LF normalize.
- `[AUTO]` EUC-KR 인코딩 파일은 1차 fail → 2차 detect 통과 + LF normalize.
- `[AUTO]` 인코딩 detect 실패 시 Status::Failed + reason "encoding".

### Phase 5 시나리오 (NFD path)

- `[AUTO]` raw bytes injection — NFD 한글 path (`\u{1100}\u{1161}.txt`)가 NFC 정규화 (`\u{AC00}.txt`) 후 동일 path key.

### Phase 5 시나리오 (빈 파일 실파일)

- `[AUTO]` integration test에서 0-byte 파일 박음 + local ↔ remote empty blob → Status::Identical.
