# Spec — Domain Pitfalls

> Phase 5 박제 (2026-05-09). v0.1 비목표였던 "도메인 함정 정리"를 영구 박음. 박제 expiration: Phase 진입마다 재검토 (CLAUDE.md § 박제 expiration 정책).

## 목적

GitHub repo와 로컬 디렉토리 간 비교에서 OS / 인코딩 / git 메타데이터 차이로 발생하는 false drift / silent drift 8 함정을 정확히 처리. v0.1 vault 검증(356 파일, 0 drift) 위에 누락된 도메인 케이스 박음.

## 8 함정 처리 정책 매핑

| 함정 | 정책 | 영향 spec |
|---|---|---|
| **NFD vs NFC** (path) | 정확 hash 재현 — path 정규화 NFC | spec-classification.md |
| **대소문자 충돌** (Windows) | 정확 hash 재현 — case-sensitive 비교 (Unix-style), OS layer는 그대로 | spec-classification.md |
| **비-UTF-8 인코딩** | 변환 시도 후 detect-only — 1차 UTF-8 → 2차 인코딩 detect → 3차 Status::Failed | spec-output-schema.md / spec-error-contracts.md / spec-hash-and-normalize.md |
| **submodule (`160000`)** | detect-only — Status::Failed + reason "submodule" + mode bit 보고 | spec-github-api.md / spec-output-schema.md |
| **symlink (`120000`)** | detect-only — Status::Failed + reason "symlink" + mode bit 보고 | spec-github-api.md / spec-output-schema.md |
| **빈 파일** | 정확 hash + 실파일 검증 — v0.1 unit test 통과 정책 그대로 + integration test fixture 추가 | spec-hash-and-normalize.md |
| **실행 권한 (`100755` vs `100644`)** | detect-only — content 같으면 Status::Identical 유지, mode bit 정보 보고 | spec-output-schema.md |
| **`.gitattributes`** | **정확 hash 재현 (큰 변경)** — 파싱 + LF/CRLF 경계 + binary attribute 정확 적용 | spec-hash-and-normalize.md (구조 변경) |

## 정확 hash 재현 정책 상세

### Path 정규화 (NFD / case)

- **NFD → NFC**: 모든 path를 NFC로 정규화 후 비교 key 박음. macOS HFS+/APFS는 NFD로 저장하지만 GitHub은 NFC 보존 → 정규화 안 하면 `가.txt` (NFC: U+AC00) vs `가.txt` (NFD: U+1100 U+1161)이 다른 path key로 박혀 false drift 발생.
- 라이브러리: `unicode-normalization` crate (well-established).
- **Case 정책**: case-sensitive 비교 (Unix-style). `README.md` vs `Readme.md`는 다른 path key. Windows OS는 case-insensitive로 동일 file 취급하지만 도구는 case-sensitive 그대로 박음 — drift로 표면화하는 게 정합.

### `.gitattributes` 정확 재현 (큰 변경)

- v0.1 정책: 항상 LF normalize (모든 텍스트 파일).
- v0.2 (Phase 5) 정책: **conditional LF normalize**:
  - `text=auto` 명시: LF normalize.
  - `binary` 명시: raw bytes (normalize 안 함, NUL byte 휴리스틱 무시).
  - `eol=lf` 명시: LF normalize.
  - `eol=crlf` 명시: CRLF 보존 (`\r\n` → `\r\n` 그대로, `\n` → `\r\n` 변환 안 함, GitHub 측 SHA와 일치).
  - 미명시: default `text=auto` 적용 (기존 v0.1 정책 그대로).
- 파서: project root + 하위 디렉토리의 `.gitattributes` 파일 로드 + glob pattern matching (`gitignore-style`).
- 우선순위: 가장 깊은 디렉토리의 `.gitattributes`가 우선 (gitignore convention과 동일).

### 빈 파일

- v0.1 unit test 통과 정책 그대로 (`SHA-1("blob 0\0") = e69de29bb2d1d6434b8b29ae775ad8c2e48c5391`).
- 추가: integration test fixture로 실제 0-byte 파일 박음 + local ↔ remote identical 분류 검증.

## detect-only 정책 상세

### 인코딩 변환 시도

- 1차: UTF-8 디코드 시도. 성공 시 LF normalize 적용 (또는 `.gitattributes` 정합).
- 2차 (실패 시): 다른 인코딩 detect (`encoding_rs` 또는 동등 라이브러리, task E에서 결정).
- 3차 (실패 시): Status::Failed + reason "encoding" + binary 취급 (현재 G-006 정책 계승).

### Submodule / Symlink

- Trees mode `160000` (submodule) / `120000` (symlink) entry는 v0.1 비목표 (skip + warning, G-010).
- v0.2 (Phase 5): detect-only로 박음.
  - Status::Failed + reason "submodule" 또는 "symlink".
  - JSON 출력에 mode bit (`160000` / `120000`) 박음.
  - 정확 hash 재현 안 함 (submodule은 외부 repo, symlink는 OS-dependent target).

### 실행 권한 mode bit

- Trees mode `100755` (executable) vs `100644` (regular)는 content 차이 아님.
- 정책: drift로 판정 안 함, 다만 정보 보고 (warning level).
- JSON 출력에 mode bit 박음 (호출자가 mode 차이 정책 결정).

## 검증 환경

### Windows 1차 + 실용 근사

- macOS NFD 검증은 raw bytes injection unit test fixture로 대체:
  - compose 한글 (`가` = `\u{AC00}`) vs decompose (`가` = `\u{1100}\u{1161}`).
  - `walker.rs`에 path bytes 박힌 후 NFC 정규화 적용 검증.
- symlink는 Windows에서 unprivileged process 박을 수 없음 — Trees API mock 응답에 mode `120000` entry 박아 검증.
- submodule도 mock 응답으로 검증 (실제 submodule 박지 않음).
- 인코딩 fixture는 byte literal로 EUC-KR / Shift_JIS / Latin-1 박음.

### Cross-platform CI는 Phase 5 범위 외

- macOS/Linux runner 추가는 별도 phase. 현재 Windows runner 그대로.
- 사용자 macOS 실기 검증은 1회성 — 없으면 unit test fixture로 충분.

## v0.1 baseline 영향

- v0.1 vault 검증 (356 파일, 0 drift, ureq 시절): 위 8 함정이 vault에 얼마나 영향 미쳤는지 vault scan 재실행 + drift 근원 분석으로 측정 (task A).
- Phase 6 완료 시점 baseline: 244 tests pass + tarpaulin 88.31%.
- Phase 5 진행 후 baseline: vault dogfooding false drift 0건 (의도된 detect-only drift 제외).

## Acceptance Criteria

(generic — task별 매핑은 `docs/ralph/implementation-plan.md`)

- 8 함정 처리 정책 implement 완료.
- 함정별 unit test fixture 박음 (Windows 환경 — raw bytes injection / mock 응답).
- vault dogfooding 통과 — false drift 0건 (의도된 detect-only drift 제외).
- 영향 받는 spec 갱신: `spec-hash-and-normalize.md` (큰 변경) + `spec-classification.md` + `spec-error-contracts.md` + `spec-output-schema.md`.
- Phase 6 hard gate (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출) 모두 deny active 유지.
