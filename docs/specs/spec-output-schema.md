# Spec: Output JSON Schema v1.0

## 목적
`scan` 명령어가 stdout으로 출력하는 결과 JSON의 안정적 스키마. AI 호출자가 파싱·소비할 수 있도록 버전 보장.

## 현재 상태
- `crates/gitless-sync/src/commands/scan/output.rs::{ScanReport, Summary, FileEntry}` 구조체 + serde 직렬화 완료.
- `SCHEMA_VERSION = "1.0"` 상수 박힘.
- `serialize(report, pretty)` 함수 구현 완료.
- 빠진 것: `--summary-only` / `--status` 필터 적용은 `scan::run` 오케스트레이터에서.

## 작업 범위

### 스키마 v1.0 (전체)
```json
{
  "schema_version": "1.0",
  "scanned_at": "2026-04-27T10:30:00Z",
  "repo": "owner/name",
  "branch": "main",
  "local_root": "/path/to/dir",
  "summary": {
    "identical": 120,
    "local_only_changed": 3,
    "remote_only_changed": 0,
    "drift": 1,
    "failed": 0
  },
  "files": [
    {
      "path": "notes/foo.md",
      "status": "drift",
      "local_sha": "abc...",
      "remote_sha": "def...",
      "local_mtime": "2026-04-26T18:00:00Z",
      "remote_last_commit_at": "2026-04-26T22:30:00Z",
      "is_binary": false
    }
  ]
}
```

### 안정성 보장
- `schema_version`: 호환성 깨는 변경 시 major 증가. v0.1은 `"1.0"`.
- `status` enum 동결: `identical` / `local_only_changed` / `remote_only_changed` / `drift` / `failed`. 추가는 minor 버전, 제거·이름 변경은 major.
- 시간 필드: 모두 ISO-8601 UTC (`Z` suffix). 로컬 타임존 출력 금지.
- null 정책:
  - 원격 only 파일: `local_sha=null`, `local_mtime=null`.
  - 로컬 only 파일: `remote_sha=null`, `remote_last_commit_at=null`.
  - identical 파일: 정상 SHA + 시간.

### `--summary-only` 출력
위 JSON에서 `files` 필드 자체를 제거 (`null`이 아니라 omit). 다른 필드는 유지.

### `--status` 필터
`--status drift,local_only_changed` 형식. 지정한 status에 해당하는 파일만 `files[]`에 포함. `summary` 카운트는 필터 무관 전체 집계.

## Acceptance Criteria
- `[AUTO]` `serialize(&report, false)`는 한 줄 compact JSON.
- `[AUTO]` `serialize(&report, true)`는 들여쓰기된 pretty JSON.
- `[AUTO]` `report.schema_version` == `"1.0"`.
- `[AUTO]` `Status` serde가 `snake_case`로 출력 (`local_only_changed` 등).
- `[AUTO]` `local_sha == None`인 `FileEntry`는 `local_sha` 필드가 출력 JSON에 omit (`#[serde(skip_serializing_if = "Option::is_none")]` 동작).
- `[AUTO]` `scanned_at`, `local_mtime`, `remote_last_commit_at` 모두 `Z` suffix로 출력 (chrono UTC serde 기본).
- `[AUTO]` `--summary-only` 시 `files` 필드 자체가 출력에서 omit (`Option::None`).
- `[AUTO]` `--status drift` 시 `files[]`에 `Status::Drift`인 항목만 포함 (PRD 검증 시나리오 14).
- `[AUTO]` `--summary-only` 시 stdout 출력에 문자열 `"files"` 미포함 (PRD 검증 시나리오 13).
- `[AUTO]` summary 카운트는 필터와 무관하게 전체 집계 (예: `--status drift` 시에도 `summary.identical` 카운트는 정상).
