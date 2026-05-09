# Spec: Output JSON Schema v1.1

## 목적
`scan` 명령어가 stdout으로 출력하는 결과 JSON의 안정적 스키마. AI 호출자가 파싱·소비할 수 있도록 버전 보장.

> **Phase 5 갱신 (2026-05-09)**: schema_version 1.0 → **1.1** (minor bump). 새 필드 `mode` + `failed_reason` + `lfs_pointer` 추가. 기존 필드 변경 없음 — 호출자 backward-compat 유지.

## 현재 상태
- `crates/gitless-sync/src/commands/scan/output.rs::{ScanReport, Summary}` 구조체 + serde 직렬화 완료 (v1.0).
- `crates/gitless-sync/src/commands/scan/compare.rs::{FileEntry, Status, FailedReason, LfsPointer}` 정의됨 (v1.1 신규 필드 mode/failed_reason/lfs_pointer 포함).
- `SCHEMA_VERSION = "1.1"` 상수 정의 (Phase 5에서 갱신).
- `serialize(report, pretty)` 함수 구현 완료.

### O-task audit (2026-05-09)

본 spec과 실 구현(`commands/scan/output.rs` / `commands/scan/compare.rs` / `commands/scan/pipeline.rs::pre_entry_to_file`) 정합 검증. 구현 vs 미구현 + drift surface — fix는 본 task scope 안 항목만 처리. 외 drift는 follow-up task로 분리.

**구현 정합**:
- `SCHEMA_VERSION = "1.1"` 상수 — `output.rs::SCHEMA_VERSION` 정의됨.
- `ScanReport` 7 v1.0 필드 (`schema_version` / `scanned_at` / `repo` / `branch` / `local_root` / `summary` / `files`) — `output.rs::ScanReport` line 18~27 정의됨. `files: Option<Vec<FileEntry>>` + `#[serde(skip_serializing_if = "Option::is_none")]` → `--summary-only` 시 자동 omit.
- `FileEntry` v1.0 필드 (`path` / `status` / `local_sha` / `remote_sha` / `local_mtime` / `remote_last_commit_at` / `is_binary`) — `compare.rs::FileEntry` line 39~56 정의됨. v1.0 Optional 필드는 모두 `#[serde(skip_serializing_if = "Option::is_none")]` 적용.
- `FileEntry` v1.1 신규 필드 — `mode: String` (Option 아님 — 모든 entry 항상 포함) + `failed_reason: Option<FailedReason>` + `lfs_pointer: Option<LfsPointer>` 모두 `#[serde(skip_serializing_if = "Option::is_none")]` 적용 → v1.0 호출자 backward-compat (필드 부재 시 v1.0 baseline 동작).
- `Status` 5 variant + serde `rename_all = "snake_case"` — `compare.rs::Status` line 4~12 정의됨 (Phase 5에서 새 status 미추가 정합).
- `LfsPointer { oid: String, size: u64 }` 정의됨 — `compare.rs::LfsPointer` line 32~36.
- 모든 `Status::Failed` entry가 mode 포함 (`pipeline.rs::pre_entry_to_file` line 217~228). Hashed entry도 mode 포함 (line 250~261).
- Hashed 분기 `failed_reason: None / lfs_pointer: None` 정합 (`pipeline.rs` line 259~260) → 비-Failed entry는 v1.1 신규 필드 omit 정합.
- `failed_reason == "lfs_pointer"` 한정 `lfs_pointer` 필드 포함 — `pipeline.rs` line 226 `lfs::placeholder_pointer_for(failed_reason)` + `lfs.rs::placeholder_pointer_for` 구현됨 (`Some(LfsPointer { oid: "?", size: 0 })` for `LfsPointer` reason, `None` 외). `pipeline_tests_lfs.rs` line 87~130 검증 통과.
- spec § acceptance line 117 `mode == "100755"` + content 동일 → `Status::Identical` — `pipeline_tests_modes.rs::assemble_entries_keeps_identical_when_only_mode_differs_executable` 검증됨.

**미구현 (Phase 5 후속, hedge marker — task N drift mirror)**:
- `failed_reason` 9 reason 중 3건 `enum-spec'd-but-unimplemented` align (task N audit과 동일 — fix scope follow-up):
  - `encoding` — `shared/decode.rs::try_decode_text` sniff 구현됨이지만 `compare.rs::FailedReason`에 variant 미정의 + `pipeline.rs` surface mapping 미구현.
  - `nfd_collision` — `walker.rs::relative_path` NFC normalize 구현됨이지만 NFD collision detect (precomposeunicode false 환경) 미구현.
  - `gitattributes_unsupported` — `shared/gitattributes::AttributeMatch::Unsupported` variant 정의됨 + `prepare_for_hash` defensive fall-through 구현됨이지만 `pipeline.rs` `Status::Failed` mapping plumbing 미구현.
- `compare.rs::FailedReason` 5 variant (`CaseCollision / Submodule / Symlink / LongPath / LfsPointer`) + `None` special case (`hash_io` v1.0 baseline) = 6 cover. 9 reason 중 3건 enum-spec'd-but-unimplemented는 task N의 fix scope follow-up과 동일 — 본 task 코드 fix 안 함.

**Spec self-consistency fix (본 task에서 진행)**:
- § Acceptance Criteria § v1.1 신규 `enum 9 값 중 하나` line + § 안정성 보장 `Phase 5에서 정의된 9 reason` line 양쪽 hedge marker 추가 — 구현 5 variant + None special case 정합 + 3 enum-spec'd-but-unimplemented (task N audit drift mirror, fix scope follow-up). § 안정성 보장 동결 정책 자체는 그대로 (호출자 backward-compat 정책 line은 변경 없음 — 9 reason enum 동결은 spec contract 그대로).

## 작업 범위

### 스키마 v1.1 (전체)
```json
{
  "schema_version": "1.1",
  "scanned_at": "2026-05-09T10:30:00Z",
  "repo": "owner/name",
  "branch": "main",
  "local_root": "/path/to/dir",
  "summary": {
    "identical": 120,
    "local_only_changed": 3,
    "remote_only_changed": 0,
    "drift": 1,
    "failed": 2
  },
  "files": [
    {
      "path": "notes/foo.md",
      "status": "drift",
      "local_sha": "abc...",
      "remote_sha": "def...",
      "local_mtime": "2026-04-26T18:00:00Z",
      "remote_last_commit_at": "2026-04-26T22:30:00Z",
      "is_binary": false,
      "mode": "100644"
    },
    {
      "path": "scripts/build.sh",
      "status": "identical",
      "local_sha": "ghi...",
      "remote_sha": "ghi...",
      "local_mtime": "2026-04-26T18:00:00Z",
      "remote_last_commit_at": "2026-04-26T22:30:00Z",
      "is_binary": false,
      "mode": "100755"
    },
    {
      "path": "vendor/lib.zip",
      "status": "failed",
      "failed_reason": "lfs_pointer",
      "lfs_pointer": {
        "oid": "sha256:4d7a214614ab2935...",
        "size": 12345
      },
      "mode": "100644"
    },
    {
      "path": "ext/dependency",
      "status": "failed",
      "failed_reason": "submodule",
      "mode": "160000"
    }
  ]
}
```

### v1.0 → v1.1 변경 (minor)

추가 필드 (기존 필드 변경 0):
- `files[].mode` — git tree mode bit (`100644` regular / `100755` executable / `160000` submodule / `120000` symlink). 모든 entry에 포함됨. v1.0 호출자가 미사용 시 무시 가능.
- `files[].failed_reason` — `Status::Failed` entry 한정. 함정 종류 enum (spec-error-contracts.md § Per-file Pitfall Reasons). `null`/omit 시 v0.1 baseline `hash_io` 동작.
- `files[].lfs_pointer` — `failed_reason == "lfs_pointer"` 한정. `{oid, size}` 포함. 호출자가 LFS fetch 결정 입력으로 사용.

### 안정성 보장
- `schema_version`: 호환성 깨는 변경 시 major 증가. v0.1은 `"1.0"`, Phase 5는 `"1.1"` (minor).
- `status` enum 동결: `identical` / `local_only_changed` / `remote_only_changed` / `drift` / `failed`. 추가는 minor 버전, 제거·이름 변경은 major. **Phase 5에서 새 status 미추가** — LFS/submodule/symlink는 모두 `failed` + `failed_reason` 분류.
- `failed_reason` enum 동결 정책: 추가는 minor, 제거·이름 변경은 major. Phase 5에서 정의된 9 reason (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported`). **O-task audit hedge (2026-05-09)**: 9 reason 중 6건 구현됨 (`hash_io` None special case + `submodule` / `symlink` / `lfs_pointer` / `long_path` / `case_collision` 5 enum variant), 3건 (`encoding` / `nfd_collision` / `gitattributes_unsupported`) enum-spec'd-but-unimplemented — task N audit drift mirror, fix scope follow-up. spec § 동결 정책은 contract 그대로 (호출자 backward-compat 보호).
- 시간 필드: 모두 ISO-8601 UTC (`Z` suffix). 로컬 타임존 출력 금지.
- null 정책:
  - 원격 only 파일: `local_sha=null`, `local_mtime=null`.
  - 로컬 only 파일: `remote_sha=null`, `remote_last_commit_at=null`.
  - identical 파일: 정상 SHA + 시간.
  - failed 파일: `local_sha`, `remote_sha`, `local_mtime`, `remote_last_commit_at` 모두 가능 (failed_reason에 따라). `mode`는 가능하면 포함 (remote tree mode 가용 시).

### `--summary-only` 출력
위 JSON에서 `files` 필드 자체를 제거 (`null`이 아니라 omit). 다른 필드는 유지.

### `--status` 필터
`--status drift,local_only_changed` 형식. 지정한 status에 해당하는 파일만 `files[]`에 포함. `summary` 카운트는 필터 무관 전체 집계.

## Acceptance Criteria

### v1.0 baseline (Phase 5 후에도 통과 유지)

- `[AUTO]` `serialize(&report, false)`는 한 줄 compact JSON.
- `[AUTO]` `serialize(&report, true)`는 들여쓰기된 pretty JSON.
- `[AUTO]` `Status` serde가 `snake_case`로 출력 (`local_only_changed` 등).
- `[AUTO]` `local_sha == None`인 `FileEntry`는 `local_sha` 필드가 출력 JSON에 omit (`#[serde(skip_serializing_if = "Option::is_none")]` 동작).
- `[AUTO]` `scanned_at`, `local_mtime`, `remote_last_commit_at` 모두 `Z` suffix로 출력 (chrono UTC serde 기본).
- `[AUTO]` `--summary-only` 시 `files` 필드 자체가 출력에서 omit (`Option::None`).
- `[AUTO]` `--status drift` 시 `files[]`에 `Status::Drift`인 항목만 포함 (PRD 검증 시나리오 14).
- `[AUTO]` `--summary-only` 시 stdout 출력에 문자열 `"files"` 미포함 (PRD 검증 시나리오 13).
- `[AUTO]` summary 카운트는 필터와 무관하게 전체 집계 (예: `--status drift` 시에도 `summary.identical` 카운트는 정상).

### v1.1 신규 (Phase 5)

- `[AUTO]` `report.schema_version` == `"1.1"`.
- `[AUTO]` `files[].mode` 필드가 모든 entry에 포함 (`"100644"` / `"100755"` / `"160000"` / `"120000"`).
- `[AUTO]` `Status::Failed` entry에 `failed_reason` 필드 포함. enum 9 값 중 하나. **O-task audit hedge (2026-05-09)**: 구현 5 variant (`CaseCollision / Submodule / Symlink / LongPath / LfsPointer`) + `None` special case (`hash_io`) = 6 cover. `encoding` / `nfd_collision` / `gitattributes_unsupported` 3건 enum-spec'd-but-unimplemented (task N audit drift mirror) — fix scope follow-up.
- `[AUTO]` `failed_reason == "lfs_pointer"` entry에 `lfs_pointer` 필드 포함 (`{oid, size}` 형식).
- `[AUTO]` `failed_reason != "lfs_pointer"` entry는 `lfs_pointer` 필드 omit.
- `[AUTO]` `Status` 외 entry (Identical / LocalOnlyChanged 등)는 `failed_reason` 필드 omit.
- `[AUTO]` `mode == "100755"` + content 동일 → `Status::Identical` (mode 차이는 drift로 판정 안 함, spec-domain-pitfalls.md § 실행 권한).
- `[AUTO]` v1.0 호출자가 v1.1 JSON 파싱 시 추가 필드 무시 + 기존 필드 정상 동작 (backward-compat 검증).
