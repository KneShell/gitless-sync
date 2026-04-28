# Spec: Hash & Normalize

## 목적
로컬 파일과 원격 blob을 의미적 동일성("줄바꿈·BOM 차이는 동일로 간주") 기준으로 비교하기 위한 자체 정의 SHA-1 해시 계산. **git 표준 blob SHA가 아니다** (G-001).

## 현재 상태
- `crates/gitless-sync/src/shared/hash.rs::blob_hash` 구현 완료 (SHA-1 + git blob header 형식). empty blob 테스트 통과.
- `crates/gitless-sync/src/shared/normalize.rs::{is_binary, normalize_text, prepare_for_hash}` 구현 완료.
- 추가 테스트 필요 (CRLF, BOM, binary 케이스).

## 작업 범위

### 해시 정의
- 텍스트: `SHA-1("blob <size>\0<LF-normalized + BOM-stripped content>")`
- 바이너리: `SHA-1("blob <size>\0<raw bytes>")`
- size는 normalize 후의 바이트 크기.

### Normalize 규칙
1. **바이너리 판별**: 첫 8000 바이트 안에 NUL 바이트 (`0x00`)가 있으면 바이너리. 바이너리는 normalize 안 함.
2. **BOM 처리**: 텍스트가 UTF-8 BOM (`EF BB BF`)으로 시작하면 제거. 단 `--keep-bom` 플래그 시 보존.
3. **개행 정규화**: `\r\n` → `\n`. 단독 `\r`은 그대로 둠 (Mac Classic 시대 유물; 흔치 않음).

### 원격 측 비교
GitHub Trees API가 반환하는 blob SHA는 working tree 바이트의 해시이며 `core.autocrlf` / `.gitattributes` 영향을 받음. 본 도구는 `.gitattributes`를 파싱하지 않으므로 원격 blob을 다운로드(`spec-github-api.md`) → 동일한 normalize 적용 → 자체 SHA 재계산해서 비교한다. **Trees API SHA는 무시**.

### 함정 (G-001)
- empty blob (`e69de29bb2d1d6434b8b29ae775ad8c2e48c5391`)이 git 상수와 일치하는 건 우연. 다른 파일은 일치 보장 안 됨.
- 호출자가 `git hash-object` 결과와 비교하지 않도록 문서로 강조 (CLAUDE.md + 본 spec).

## Acceptance Criteria
- `[AUTO]` `hash::blob_hash(&[])` == `"e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"` (이미 통과).
- `[AUTO]` `hash::blob_hash(b"hello\n")` == `hash::blob_hash(&normalize::normalize_text(b"hello\r\n", false))` — CRLF가 LF로 정규화된 후 같은 해시.
- `[AUTO]` `normalize::normalize_text(&[0xEF, 0xBB, 0xBF, b'a'], false)` == `b"a"` — BOM 제거.
- `[AUTO]` `normalize::normalize_text(&[0xEF, 0xBB, 0xBF, b'a'], true)` == `&[0xEF, 0xBB, 0xBF, b'a']` — `--keep-bom` 시 BOM 보존.
- `[AUTO]` `normalize::is_binary(&[0u8, 1, 2])` == `true`. `normalize::is_binary(b"plain text")` == `false`.
- `[AUTO]` `prepare_for_hash`가 binary 입력에는 normalize 안 하고 raw 그대로 반환 + `(_, true)` 플래그.
- `[AUTO]` `prepare_for_hash`가 text 입력에는 normalize 적용 + `(_, false)` 플래그.
- `[AUTO]` PRD 검증 시나리오 5: CRLF로 저장된 로컬 파일과 LF로 저장된 동일 내용 원격 blob의 자체 SHA가 일치 (단위 테스트로).
- `[AUTO]` PRD 검증 시나리오 6: BOM 있는 로컬과 BOM 없는 원격이 `--keep-bom` 미지정 시 동일 SHA.
- `[AUTO]` PRD 검증 시나리오 7: `--keep-bom` 시 BOM 차이가 다른 SHA를 만든다.
- `[AUTO]` PRD 검증 시나리오 8: 동일 바이너리 파일은 동일 SHA (raw byte 해시).
