# Spec: Output JSON Schema v1.5

## 목적
`scan` 명령어가 stdout으로 출력하는 결과 JSON의 안정적 스키마. AI 호출자가 파싱·소비할 수 있도록 버전 보장.

> **Phase 5 갱신 (2026-05-09)**: schema_version 1.0 → **1.1** (minor bump). 새 필드 `mode` + `failed_reason` + `lfs_pointer` 추가. 기존 필드 변경 없음 — 호출자 backward-compat 유지.
>
> **Phase 7 갱신 (2026-05-10)**: schema_version 1.1 → **1.2** (minor bump). `failed_reason` enum 9 → 11 (`file_too_large` + `memory_exceeded` 추가) + 신규 field `size_bytes` (Failed entry size 진단). 기존 필드 변경 없음 — 호출자 backward-compat 유지.
>
> **Phase 8 갱신 (2026-05-10)**: schema_version 1.2 → **1.3** (minor bump). 신규 field `diff_meaningful: Option<bool>` (F1 — scan/diff 비교 기준 불일치 해소) + `presence: "local_only" | "both" | "remote_only"` (F2 — `local_only_changed` 의미 모호 해소). 4-state status (`identical` / `local_only_changed` / `remote_only_changed` / `drift` / `failed`) 그대로 유지 — backward compat 보장. 결정 trail은 `docs/adr/0014-scan-diff-metadata-contract.md`.
>
> **v0.4.2 갱신 (2026-05-11)**: schema_version 1.3 → **1.4** (minor bump). `Identical` 정의 정확화 — sha-differ + `normalize_equal == Some(true)` (cosmetic drift, F1 케이스) 도 `Identical` 분류. 기존 caller 코드는 그대로 작동 — Identical 카운트가 더 정확해지고 LocalOnlyChanged/RemoteOnlyChanged 카운트는 cosmetic drift만큼 감소. backward compat 보장 (additive 의미 정확화). issue #1 regression. 결정 trail은 `docs/adr/0015-cosmetic-identical-classification.md`.
>
> **v0.5.0 갱신 (2026-05-12)**: schema_version 1.4 → **1.5** (minor bump). `--summary-only` 모드 출력 contract 확장 — failed status entry 한정 `files[]`에 minimal entry (path + presence + failed_reason) emit. failed 0건이면 v1.4 baseline 유지 (`files` 필드 omit). 그 외 status entry (identical / local_only_changed / remote_only_changed / drift)는 summary-only에서 emit 안 함. post-v0.4.2 vault dogfood feedback F3 motivation (한 호출로 어떤 파일이 실패했는지 확인) 직접 해소. 신규 field/enum 0이지만 caller-visible behavior change이므로 minor. 결정 trail은 git history (`git log --grep="Phase 9"`) + `CHANGELOG.md` § [0.5.0].

## 현재 상태
- `crates/gitless-sync/src/commands/scan/output.rs::{ScanReport, Summary}` 구조체 + serde 직렬화 완료 (v1.0).
- `crates/gitless-sync/src/commands/scan/compare.rs::{FileEntry, Status, FailedReason, LfsPointer}` 정의됨 (v1.1 신규 필드 mode/failed_reason/lfs_pointer 포함, v1.2 신규 size_bytes 포함).
- `SCHEMA_VERSION = "1.4"` 상수 정의 — Phase 8에서 `"1.3"` bump, v0.4.2에서 `"1.4"` bump (cosmetic Identical fix). Phase 9 task J에서 `"1.5"` bump 예정 (summary-only failed visibility 확장, 코드 변경은 task J~L scope).
- `serialize(report, pretty)` 함수 구현 완료.
- `FileEntry`에 `diff_meaningful` + `presence` field는 Phase 8 task F~I에서 신규 추가 예정.

## 작업 범위

### 스키마 v1.5 (전체)

> 아래 sample은 `--summary-only` 미지정 시 전체 mode 출력. v1.5는 전체 mode wire shape 변경 0 — 이전 v1.4와 byte-identical (`schema_version` 값만 다름). summary-only mode shape는 § `--summary-only` 출력 참조.

```json
{
  "schema_version": "1.5",
  "scanned_at": "2026-05-10T10:30:00Z",
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
      "presence": "both",
      "diff_meaningful": true,
      "local_sha": "abc...",
      "remote_sha": "def...",
      "local_mtime": "2026-04-26T18:00:00Z",
      "remote_last_commit_at": "2026-04-26T22:30:00Z",
      "is_binary": false,
      "mode": "100644"
    },
    {
      "path": "notes/bom-only-drift.md",
      "status": "drift",
      "presence": "both",
      "diff_meaningful": false,
      "local_sha": "111...",
      "remote_sha": "222...",
      "local_mtime": "2026-04-26T18:00:00Z",
      "remote_last_commit_at": "2026-04-26T22:30:00Z",
      "is_binary": false,
      "mode": "100644"
    },
    {
      "path": "scripts/build.sh",
      "status": "identical",
      "presence": "both",
      "diff_meaningful": false,
      "local_sha": "ghi...",
      "remote_sha": "ghi...",
      "local_mtime": "2026-04-26T18:00:00Z",
      "remote_last_commit_at": "2026-04-26T22:30:00Z",
      "is_binary": false,
      "mode": "100755"
    },
    {
      "path": "drafts/local-new.md",
      "status": "local_only_changed",
      "presence": "local_only",
      "local_sha": "jkl...",
      "local_mtime": "2026-05-10T09:00:00Z",
      "is_binary": false,
      "mode": "100644"
    },
    {
      "path": "remote/orphan.md",
      "status": "remote_only_changed",
      "presence": "remote_only",
      "remote_sha": "mno...",
      "remote_last_commit_at": "2026-05-09T22:00:00Z",
      "is_binary": false,
      "mode": "100644"
    },
    {
      "path": "vendor/lib.zip",
      "status": "failed",
      "presence": "both",
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
      "presence": "both",
      "failed_reason": "submodule",
      "mode": "160000"
    },
    {
      "path": "media/big-video.mp4",
      "status": "failed",
      "presence": "both",
      "failed_reason": "file_too_large",
      "size_bytes": 157286400,
      "mode": "100644"
    },
    {
      "path": "data/largish-archive.tar",
      "status": "failed",
      "presence": "both",
      "failed_reason": "memory_exceeded",
      "size_bytes": 62914560,
      "mode": "100644"
    }
  ]
}
```

### v1.0 → v1.1 변경 (minor)

추가 필드 (기존 필드 변경 0):
- `files[].mode` — git tree mode bit (`100644` regular / `100755` executable / `160000` submodule / `120000` symlink). 모든 entry에 포함됨. v1.0 호출자가 미사용 시 무시 가능.
- `files[].failed_reason` — `Status::Failed` entry 한정. 함정 종류 enum (spec-error-contracts.md § Per-file Pitfall Reasons). `null`/omit 시 v0.1 baseline `hash_io` 동작.
- `files[].lfs_pointer` — `failed_reason == "lfs_pointer"` 한정. `{oid, size}` 포함. 호출자가 LFS fetch 결정 입력으로 사용.

### v1.1 → v1.2 변경 (minor)

추가 필드 (기존 필드 변경 0):
- `files[].size_bytes` — Failed entry size 진단 field. `file_too_large` / `memory_exceeded` reason entry 한정 (그 외 omit). u64 byte 단위. 호출자 디버깅 + 사용자 surface 용도.

추가 enum 값 (`failed_reason` 9 → 11):
- `file_too_large` — local 또는 remote file size가 100 MB (GitHub Blobs API hard limit) 초과.
- `memory_exceeded` — local 또는 remote file size가 50 MB (tool 메모리 안전 임계) 초과.

### v1.2 → v1.3 변경 (minor)

LLM-as-caller usability eval (2026-05-10) 7 friction 중 P0 2건 (F1 + F2) 해소. 결정 trail은 `docs/adr/0014-scan-diff-metadata-contract.md`.

추가 필드 (기존 필드 변경 0):

- `files[].diff_meaningful` (F1 — scan과 diff 비교 기준 불일치 해소) — `Option<bool>`. caller에게 "이 entry가 `diff` 호출했을 때 의미 있는 결과 나오는지" hint. 4-case lock:

  | 시나리오 | 값 | 근거 |
  |---|---|---|
  | Identical (sha same, presence=both) | `false` | normalize 전후 동일. diff 호출 stdout 0 bytes 확정. |
  | sha differ + normalize-equal (presence=both) | `false` | F1 케이스 본체 — BOM/encoding 차이만 있는 sha drift. diff 호출 stdout 0 bytes. |
  | sha differ + normalize-diff (presence=both) | `true` | 진짜 의미 차이. diff 호출 unified text 출력 expected. |
  | LocalOnly / RemoteOnly / Failed | omit (`None`) | 비교 대상 한쪽 부재 또는 비교 불가 — diff_meaningful 정의 자체가 N/A. |

  계산 근거 — `docs/specs/spec-hash-and-normalize.md` § Normalize 규칙 재사용. compare 시점에 sha 비교 후 differ면 normalize-equal 검증 1회 추가. `Option::is_none` 시 `#[serde(skip_serializing_if = "Option::is_none")]`로 wire JSON에서 omit.

- `files[].presence` (F2 — `local_only_changed` 의미 모호 해소) — `"local_only" | "both" | "remote_only"` enum (`#[serde(rename_all = "snake_case")]`). 직교: status는 액션 분류(push/pull/conflict 후보), presence는 존재성 분류. 모든 entry에 포함됨 (Failed 포함).

  | local exists | remote exists | presence |
  |---|---|---|
  | yes | yes | `both` |
  | yes | no | `local_only` |
  | no | yes | `remote_only` |

  status (`local_only_changed` / `remote_only_changed`)와 직교 — 같은 status가 (i) "한쪽만 존재" + (ii) "양쪽 존재 + 한쪽만 변경" 둘 다 cover했던 모호함을 presence가 1차 분기로 해소. caller는 `presence == "local_only"` → push 후보, `presence == "both"` + `status == "local_only_changed"` → conflict 후보처럼 분기 가능.

기각된 대안 (ADR 0014):
- F2 status 4→6 split (`local_only_added` / `local_only_modified` 등) — breaking change + 호출자 분기 늘어남 + status semantics(액션 분류)와 presence(존재성)를 한 enum에 묶어 직교성 깨짐.
- F1 diff stderr hint — 호출 2회 필요. scan 1회로 모든 정보 받게 함이 caller-decides 본성에 더 정합.

### v1.4 → v1.5 변경 (minor)

`--summary-only` 모드 출력 contract 확장 (v0.5.0 = post-v0.4.2 vault dogfood feedback F3 motivation 해소). 결정 trail은 git history (`git log --grep="Phase 9"`) + `CHANGELOG.md` § [0.5.0].

변경 범위 (`--summary-only` 모드 한정):

- 기존 (v1.4까지) — `files` 필드 자체를 결과 JSON에서 omit. 호출자가 어떤 파일이 failed인지 확인하려면 `--status failed`로 재호출 필요.
- v1.5 — failed status entry 한정 `files[]`에 minimal entry emit. entry 한 개는 `path` + `presence` + `failed_reason` 세 field만 (`sha` / `size` / `mode` / `diff_meaningful` / `lfs_pointer` / `size_bytes` 등 detail field는 모두 omit). summary-only 정체성 = "카운트 + 무엇이 실패했나 명단"으로 유지. `failed_reason == hash_io` (코드상 `Option::None` 특수 케이스) 인 entry는 v1.1 contract 정합 — `failed_reason` 필드가 omit되어 entry가 `path` + `presence` 두 field가 됨 (key 부재로 `hash_io` 의미 표현).
- failed 0건이면 `files` 필드 omit — v1.4 baseline 동작 유지.
- 그 외 status entry (`identical` / `local_only_changed` / `remote_only_changed` / `drift`)는 summary-only에서 emit 안 함 — v1.4 동작 유지.

신규 field 0 / 신규 enum 0:
- `failed_reason` 필드는 v1.2까지 정의된 11 cover (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported` / `file_too_large` / `memory_exceeded`, 코드 10 variant + None 특수 케이스) 그대로 유지. Phase 9에서 enum 추가/제거 0.
- `presence` 필드는 v1.3에서 도입된 enum 3 variant (`local_only` / `both` / `remote_only`) 그대로.

전체 모드 (`--summary-only` 미지정 시) 동작 변경 0 — v1.4와 byte-identical (`schema_version` 값만 `"1.4"` → `"1.5"`).

`--status` filter 상호작용:
- v1.4 baseline — `--summary-only` 와 `--status` 동시 명시 시 summary-only 정체성 우선, status filter 무시 (PRD 검증 시나리오 13).
- v1.5 동일 — summary-only 가 emit하는 failed entry는 `--status` filter 무관 등장 (filter override).

backward-compat:
- 추가 필드 0 + 추가 enum 0이라 v1.0~v1.3 호출자가 v1.5 전체 모드 JSON 파싱 시 영향 0.
- v1.4까지 caller 코드가 `--summary-only` 응답에서 `files == null` 또는 `"files"` key 부재를 가정한 분기가 있다면 v1.5에서 깨질 수 있음 — failed 발생 케이스에서 `files: [...]` 배열이 등장한다.

  | v1.4 caller 분기 | v1.5 동작 |
  |---|---|
  | `"files" in resp` → 전체 mode로 판단 | failed 0건이면 false (v1.4 동일), failed N건이면 true (변경됨) |
  | `resp.files == null` → summary-only mode 확정 | failed 0건이면 true (v1.4 동일), failed N건이면 false (변경됨, 배열 등장) |
  | summary-only mode 판단에 caller 자신의 `--summary-only` argument 기준 | 영향 0 (caller alignment) |

  caller migration: summary-only mode 판단에 응답의 `files` 부재 단서를 쓰지 않고 caller 자신의 `--summary-only` argument로 분기 — v1.5 동작과 정합.

### 안정성 보장
- `schema_version`: 호환성 깨는 변경 시 major 증가. v0.1은 `"1.0"`, Phase 5는 `"1.1"`, Phase 7은 `"1.2"`, Phase 8은 `"1.3"`, v0.4.2는 `"1.4"`, v0.5.0은 `"1.5"` (모두 minor — 신규 field 추가 또는 `Identical` 분류 정확화 또는 `--summary-only` 출력 contract 확장, 기존 필드 변경 0).
- `status` enum 동결: `identical` / `local_only_changed` / `remote_only_changed` / `drift` / `failed`. 추가는 minor 버전, 제거·이름 변경은 major. **Phase 5에서 새 status 미추가** — LFS/submodule/symlink는 모두 `failed` + `failed_reason` 분류.
- `failed_reason` enum 동결 정책: 추가는 minor, 제거·이름 변경은 major. Phase 5에서 정의된 9 reason (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported`) 모두 구현 (Phase 5.13 task AA, 2026-05-09 — `compare.rs::FailedReason` 8 variant + `None` special case `hash_io`). Phase 7에서 2 reason 추가 — `file_too_large` + `memory_exceeded` (총 11 reason, `compare.rs::FailedReason` 10 variant + `None` special case).
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

v1.5부터 동작 (v1.4와의 차이는 § v1.4 → v1.5 변경 참조):

- 기본 — 전체 mode JSON에서 `files` 필드 자체를 omit (`null` 아니라 키 부재). `schema_version` / `scanned_at` / `repo` / `branch` / `local_root` / `summary` 등 다른 필드는 유지.
- failed status entry 존재 시 (`summary.failed > 0`) — `files[]`에 failed entry 한정 minimal 형식 emit. entry 한 개는 `path` + `presence` + `failed_reason` 세 field만 (sha/size/mode/diff_meaningful/lfs_pointer/size_bytes 등 detail field 모두 omit). 그 외 status entry (identical / local_only_changed / remote_only_changed / drift)는 emit 안 함.
- `--status` filter 동시 지정 시 — summary-only 정체성 우선, status filter 무시 (summary 카운트 + failed entry 명단 contract 유지). 즉 `--summary-only --status drift` 호출도 동일하게 failed entry만 emit.
- `failed_reason == hash_io` (코드상 `Option::None` 특수 케이스) entry는 v1.1 contract 정합 — `failed_reason` 필드 omit, entry가 `path` + `presence` 두 field가 됨. key 부재로 `hash_io` 의미 표현.

minimal entry shape 예시 (failed 1건 + `failed_reason == lfs_pointer` 케이스):

```json
{
  "schema_version": "1.5",
  "scanned_at": "2026-05-12T10:30:00Z",
  "repo": "owner/name",
  "branch": "main",
  "local_root": "/path/to/dir",
  "summary": { "identical": 120, "local_only_changed": 3, "remote_only_changed": 0, "drift": 1, "failed": 1 },
  "files": [
    {
      "path": "vendor/lib.zip",
      "presence": "both",
      "failed_reason": "lfs_pointer"
    }
  ]
}
```

`status` 필드도 minimal entry에서 omit — summary-only `files[]` entry는 정의상 failed (`failed_reason` 필드 자체 또는 부재가 failed signal). 호출자 contract: "summary-only 응답의 `files[]` entry는 모두 failed로 해석".

caller-visible behavior change 영향은 § v1.4 → v1.5 변경 § backward-compat 표 참조.

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

### v1.2 신규 (Phase 7)

- `[AUTO]` `report.schema_version` == `"1.2"`.
- `[AUTO]` `failed_reason` enum 11 값 cover (구현은 `compare.rs::FailedReason` 10 variant + `None` special case `hash_io`).
- `[AUTO]` `failed_reason == "file_too_large"` entry에 `size_bytes` 필드 포함 (u64 byte). 100 MB 초과 size.
- `[AUTO]` `failed_reason == "memory_exceeded"` entry에 `size_bytes` 필드 포함 (u64 byte). 50 MB 초과 size, 100 MB 미만.
- `[AUTO]` `failed_reason` 가 `file_too_large` / `memory_exceeded` 외 entry는 `size_bytes` 필드 omit (`#[serde(skip_serializing_if = "Option::is_none")]`).
- `[AUTO]` `file_too_large` / `memory_exceeded` entry는 `is_binary: false` (size pre-flight short-circuit, local read 전 격하, Phase 5.13.1 task EE 정합).
- `[AUTO]` v1.0 / v1.1 호출자가 v1.2 JSON 파싱 시 추가 필드 (`size_bytes`) + 추가 enum 값 (`file_too_large` / `memory_exceeded`) 무시 + 기존 필드 정상 동작 (backward-compat 검증).

### v1.3 신규 (Phase 8)

ADR 0014 결정 trail. 코드 변경은 Phase 8.2 (task F~M) + 8.3 (task N~R) scope — 본 § 는 spec authoritative.

- `[AUTO]` `report.schema_version` == `"1.3"`.
- `[AUTO]` `files[].presence` 필드가 모든 entry에 포함 (`"local_only"` / `"both"` / `"remote_only"` 중 하나). Failed entry 포함.
- `[AUTO]` local 존재 + remote 부재 → `presence == "local_only"`.
- `[AUTO]` local 부재 + remote 존재 → `presence == "remote_only"`.
- `[AUTO]` local 존재 + remote 존재 → `presence == "both"`.
- `[AUTO]` Identical entry (presence=both, sha same): `diff_meaningful == false` (wire JSON에 `"diff_meaningful": false` 포함).
- `[AUTO]` Drift entry (presence=both, sha differ) + normalize-equal: `diff_meaningful == false`. F1 evidence 케이스 (BOM/encoding 차이만 있는 sha drift) — caller는 `diff` 호출 stdout 0 bytes 예측 가능.
- `[AUTO]` Drift entry (presence=both, sha differ) + normalize-diff: `diff_meaningful == true`. caller는 `diff` 호출 unified text 출력 expected.
- `[AUTO]` LocalOnlyChanged entry (presence=both case ii — 양쪽 존재 + local만 변경): `diff_meaningful` 필드 emit (`true` or `false`, normalize 결과 따라). status semantics 그대로 — status는 액션 분류, presence + diff_meaningful는 caller 분기 hint.
- `[AUTO]` LocalOnly / RemoteOnly entry (presence ≠ both): `diff_meaningful` 필드 omit (`#[serde(skip_serializing_if = "Option::is_none")]`). 비교 대상 한쪽 부재라 diff 의미 자체가 N/A.
- `[AUTO]` Failed entry: `diff_meaningful` 필드 omit (presence 값 무관). 비교 불가.
- `[AUTO]` `presence` 필드는 `Failed` entry에서도 누락 안 함 — 호출자가 "Failed인데 어느 쪽이 존재해서 fail인가" 분기 가능 (예: `presence == "local_only"` + `failed_reason == "lfs_pointer"` → local LFS pointer 만 있는 케이스).
- `[AUTO]` v1.0 / v1.1 / v1.2 호출자가 v1.3 JSON 파싱 시 추가 필드 (`presence` / `diff_meaningful`) 무시 + 기존 필드 정상 동작 (backward-compat 검증, Phase 7.2 task P 패턴 — `tests/scan_output_backward_compat.rs` V10/V11/V12 client 정합).

### v1.4 신규 (v0.4.2)

ADR 0015 결정 trail (issue #1 regression). 코드 변경: `classify` 함수에 `normalize_equal: Option<bool>` 인자 추가 + sha-differ + `Some(true)` → `Status::Identical` arm.

- `[AUTO]` `report.schema_version` == `"1.4"`.
- `[AUTO]` Hashed entry (presence=both, sha differ) + `normalize_equal == Some(true)`: `Status::Identical` (cosmetic drift — UTF-8 BOM / LF-CRLF / `.gitattributes` 정책 차이만 있는 byte-동일 케이스).
- `[AUTO]` Hashed entry (presence=both, sha differ) + `normalize_equal == Some(false)`: 기존 timestamp 분기 유지 (LocalOnlyChanged / RemoteOnlyChanged / Drift).
- `[AUTO]` Hashed entry (presence=both, sha differ) + `normalize_equal == None` (compute 실패 또는 single-side): 기존 timestamp 분기 유지 (default).
- `[AUTO]` v1.0 / v1.1 / v1.2 / v1.3 호출자가 v1.4 JSON 파싱 시 status enum 그대로 + Identical 카운트가 더 정확해지고 LocalOnlyChanged/RemoteOnlyChanged 카운트는 cosmetic drift만큼 감소 (additive 의미 정확화, breaking change 아님).
- `[AUTO]` `presence` enum 동결 정책: 추가는 minor, 제거·이름 변경은 major. `local_only` / `both` / `remote_only` 3 값으로 시작.

### v1.5 신규 (v0.5.0)

v0.5.0 (Phase 9.3) post-v0.4.2 vault dogfood F3 결정 trail. 본 § 는 spec authoritative — 코드 변경은 git history + `CHANGELOG.md` § [0.5.0] 참조.

- `[AUTO]` `report.schema_version` == `"1.5"`.
- `[AUTO]` `--summary-only` + `summary.failed == 0`: `files` 필드 omit (v1.4 baseline 동작 유지, key 부재 — `null` 아님).
- `[AUTO]` `--summary-only` + `summary.failed == N` (N > 0): `files[]`에 N entry 포함, 모두 failed status entry — 그 외 status entry (`identical` / `local_only_changed` / `remote_only_changed` / `drift`) 는 emit 안 함.
- `[AUTO]` `--summary-only` `files[]` entry는 `path` + `presence` + `failed_reason` 세 field만 emit. 그 외 detail field (`status` / `local_sha` / `remote_sha` / `local_mtime` / `remote_last_commit_at` / `is_binary` / `mode` / `diff_meaningful` / `lfs_pointer` / `size_bytes`) 모두 omit. `failed_reason` 가 `Option::None` 특수 케이스 (`hash_io` signal) 인 entry는 `failed_reason` 필드도 omit — entry가 `path` + `presence` 두 field 만 (key 부재로 `hash_io` 의미 표현, v1.1 contract 정합).
- `[AUTO]` `--summary-only` + `--status <filter>` 동시 명시 시 summary-only 정체성 우선 → status filter 무시. `files[]` 는 failed entry 명단 그대로 emit (PRD 검증 시나리오 13 정합, summary-only 가 emit 하는 failed entry 는 status filter 무관 등장).

## diff sub-schema

`diff --json` 출력 JSON의 authoritative 스키마. `spec-cli-interface.md` § diff --json 출력 형식의 참조 대상.

### 구조

`diff <path> --json` 실행 시 stdout 한 줄 JSON 객체. `--json` 명시 시 stderr side marker 미출력.

```json
{"side": "both" | "local_only" | "remote_only", "unified": string | null, "raw": string | null, "binary": bool}
```

### 필드 정의

| field | JSON type | 의미 |
|-------|-----------|------|
| `side` | `"both"` \| `"local_only"` \| `"remote_only"` | 파일 존재 위치. scan `presence` enum과 동일 semantics. 항상 emit. |
| `unified` | `string \| null` | normalize 후 unified diff 텍스트. `side == "both"` + non-binary 전용 — normalize-equal이면 `""`, normalize-diff이면 diff 텍스트. 그 외 `null`. |
| `raw` | `string \| null` | 단일 사이드 원본 파일 텍스트. `side != "both"` + non-binary 전용. 그 외 `null`. |
| `binary` | `bool` | `true` 이면 바이너리 — `unified` + `raw` 모두 `null` 강제. |

### 케이스별 stdout

| 케이스 | stdout |
|--------|--------|
| side=both + normalize-equal | `{"side":"both","unified":"","raw":null,"binary":false}` |
| side=both + normalize-diff | `{"side":"both","unified":"--- a/…\n+++ b/…\n@@ … @@\n…","raw":null,"binary":false}` |
| side=local_only (text) | `{"side":"local_only","unified":null,"raw":"<file content>","binary":false}` |
| side=remote_only (text) | `{"side":"remote_only","unified":null,"raw":"<file content>","binary":false}` |
| binary (any side) | `{"side":"<side>","unified":null,"raw":null,"binary":true}` |

### null 정책

| 조건 | `unified` | `raw` |
|------|-----------|-------|
| `binary == true` | `null` | `null` |
| `side == "both"` + non-binary | string (빈 문자열 or diff 텍스트) | `null` |
| `side != "both"` + non-binary | `null` | string |

### Acceptance Criteria

- `[AUTO]` `diff <path> --json` stdout이 한 줄 JSON + `side` / `unified` / `raw` / `binary` 4 field 포함.
- `[AUTO]` side=both + normalize-equal → `{"side":"both","unified":"","raw":null,"binary":false}` 정확 일치.
- `[AUTO]` side=both + normalize-diff → `unified` 필드 non-empty diff 텍스트 + `raw == null` + `binary == false`.
- `[AUTO]` side=local_only (text) → `{"side":"local_only","unified":null,"raw":"<content>","binary":false}` — `raw` 필드에 원본 파일 내용.
- `[AUTO]` binary (any side) → `unified == null` + `raw == null` + `binary == true`.
