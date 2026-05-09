# Spec: Output JSON Schema v1.1

## 목적
`scan` 명령어가 stdout으로 출력하는 결과 JSON의 안정적 스키마. AI 호출자가 파싱·소비할 수 있도록 버전 보장.

> **Phase 5 갱신 (2026-05-09)**: schema_version 1.0 → **1.1** (minor bump). 새 필드 `mode` + `failed_reason` + `lfs_pointer` 추가. 기존 필드 변경 없음 — 호출자 backward-compat 유지.

## 현재 상태
- `crates/gitless-sync/src/commands/scan/output.rs::{ScanReport, Summary}` 구조체 + serde 직렬화 완료 (v1.0).
- `crates/gitless-sync/src/commands/scan/compare.rs::{FileEntry, Status, FailedReason, LfsPointer}` 정의됨 (v1.1 신규 필드 mode/failed_reason/lfs_pointer 포함).
- `SCHEMA_VERSION = "1.1"` 상수 정의 (Phase 5에서 갱신).
- `serialize(report, pretty)` 함수 구현 완료.

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
- `failed_reason` enum 동결 정책: 추가는 minor, 제거·이름 변경은 major. Phase 5에서 정의된 9 reason (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported`) 모두 구현 (Phase 5.13 task AA, 2026-05-09 — `compare.rs::FailedReason` 8 variant + `None` special case `hash_io`).
- 시간 필드: 모두 ISO-8601 UTC (`Z` suffix). 로컬 타임존 출력 금지.
- null 정책:
  - 원격 only 파일: `local_sha=null`, `local_mtime=null`.
  - 로컬 only 파일: `remote_sha=null`, `remote_last_commit_at=null`.
  - identical 파일: 정상 SHA + 시간.
  - failed 파일: `local_sha`, `remote_sha`, `local_mtime`, `remote_last_commit_at` 모두 가능 (failed_reason에 따라). `mode`는 가능하면 포함 (remote tree mode 가용 시).
- `is_binary` 정책 (Phase 5.13.1 task EE 명시):
  - 의미 — local bytes의 NUL byte 휴리스틱 측정값.
  - `Hashed` (Identical / LocalOnlyChanged / RemoteOnlyChanged / Drift) entry: local read가 일어난 경우 측정값. 로컬 파일이 없는 remote-only 경우 `false` (no measurement, default).
  - `Failed` entry — `failed_reason == "encoding"`만 measured: hash IO read는 성공했고 normalize 시도가 인코딩 실패 (UTF-16 BOM 등)로 격하된 경로 → `try_hash_local`의 NUL 휴리스틱 결과 그대로 보존 (raw-bytes hash policy 정합).
  - `Failed` entry — 그 외 reason (`submodule` / `symlink` / `long_path` / `case_collision` / `nfd_collision` / `lfs_pointer` / `gitattributes_unsupported` / `hash_io`): local read 전 short-circuit 또는 IO error → 측정 없음, 항상 `false`.
  - 호출자는 `is_binary == true` + `failed_reason == "encoding"` 조합을 "raw bytes에 NUL 포함된 비-UTF-8 텍스트" 신호로 해석 가능. 그 외 Failed entry의 `is_binary == false`는 정보 부재의 default — true 의미를 추론하면 안 됨.

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
- `[AUTO]` `Status::Failed` entry에 `failed_reason` 필드 포함. enum 9 값 중 하나 — 구현 8 variant (`CaseCollision / Submodule / Symlink / LongPath / LfsPointer / Encoding / NfdCollision / GitattributesUnsupported`) + `None` special case (`hash_io`) = 9 cover. Phase 5.13 task AA 구현 완료 (2026-05-09).
- `[AUTO]` `failed_reason == "lfs_pointer"` entry에 `lfs_pointer` 필드 포함 (`{oid, size}` 형식).
- `[AUTO]` `failed_reason != "lfs_pointer"` entry는 `lfs_pointer` 필드 omit.
- `[AUTO]` `Status` 외 entry (Identical / LocalOnlyChanged 등)는 `failed_reason` 필드 omit.
- `[AUTO]` `mode == "100755"` + content 동일 → `Status::Identical` (mode 차이는 drift로 판정 안 함, spec-domain-pitfalls.md § 실행 권한).
- `[AUTO]` v1.0 호출자가 v1.1 JSON 파싱 시 추가 필드 무시 + 기존 필드 정상 동작 (backward-compat 검증).
- `[AUTO]` `failed_reason == "encoding"` entry는 `try_hash_local`의 NUL 휴리스틱 결과(`is_binary`)를 wire JSON에 보존. UTF-16 BOM 입력 (`FF FE` 또는 `FE FF` + payload)은 NUL 포함 → `is_binary: true` 유지 (Phase 5.13.1 task EE regression pin).
- `[AUTO]` `failed_reason` 가 `"submodule"` / `"symlink"` / `"long_path"` / `"case_collision"` / `"nfd_collision"` / `"lfs_pointer"` / `"gitattributes_unsupported"` 또는 `null` (`hash_io`)인 entry는 short-circuit 또는 IO error로 local read 전 격하 → `is_binary: false` (no measurement default, Phase 5.13.1 task EE).
