# Spec: Output JSON Schema v1.6

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
>
> **v0.6.0 갱신 (2026-05-12)**: schema_version 1.5 → **1.6** (minor bump). `failed_reason == hash_io` entry 의 wire 형식 정합화 — v1.5 까지 `failed_reason` 필드를 omit (`Option::None` 특수 케이스 sentinel) 하던 hash_io entry 가 v1.6 부터 다른 reason entry 와 동일하게 `failed_reason: "hash_io"` 명시 emit. summary-only 모드 minimal entry shape 가 `path + presence + failed_reason` 3 field 로 일관 (v1.5 의 hash_io 2 field special case 제거). 전체 mode 에서도 hash_io entry wire 형식 변경 (sentinel omit → explicit emit). 신규 field 0 / 신규 enum 0 — `hash_io` 는 v1.2 부터 정의돼 있었으나 wire 형식만 변경. post-v0.5.0 clean-context audit Finding 2 motivation (`Option::None` sentinel 가 분기 모호 + caller 가 missing-key 분기를 신호 sentinel 로 오해 위험) 직접 해소. caller-visible wire shape change 이므로 minor. 결정 trail은 git history (`git log --grep="Phase 10"`) + `CHANGELOG.md` § [0.6.0].
>
> **v0.8.0 갱신 (2026-05-19)**: schema_version 1.6 → **1.7** (minor bump). 신규 최상위 field `renames: [RenamePair]` — 폴더 재배치/파일 이동 시나리오 hint. `local_only_changed` + `remote_only_changed` 쌍 중 (a) raw sha 동일(`local_sha == remote_sha`) 또는 (b) normalize 후 동일(cross-path normalize-equal — issue #1 cosmetic identical 검사를 cross-path 로 mirror)한 쌍을 도구가 후처리로 emit. caller(사람 또는 AI)가 hint 보고 `diff --remote-path <from> --remote-path <to>` 검증. `renames` 는 항상 emit (`--summary-only` 한정 omit) — 빈 배열도 key 존재 보장. RenamePair shape: `{from, to, sha, raw_equal: bool}` — `raw_equal == true` Case A(same-sha), `false` Case B(normalize-equal cross-path). caller 가 두 케이스 구분 분기 가능. 기존 field 변경 0 — backward-compat 보장. issue #15 regression. 결정 trail은 `C:\Users\admin\.claude\plans\staged-cuddling-deer.md` + `CHANGELOG.md` § [0.8.0].

## 현재 상태
- `crates/gitless-sync/src/commands/scan/output.rs::{ScanReport, Summary}` 구조체 + serde 직렬화 완료 (v1.0).
- `crates/gitless-sync/src/commands/scan/compare.rs::{FileEntry, Status, FailedReason, LfsPointer}` 정의됨 (v1.1 신규 필드 mode/failed_reason/lfs_pointer 포함, v1.2 신규 size_bytes 포함).
- `SCHEMA_VERSION = "1.5"` 상수 정의 — Phase 8에서 `"1.3"` bump, v0.4.2에서 `"1.4"` bump (cosmetic Identical fix), v0.5.0/Phase 9 task J에서 `"1.5"` bump (summary-only failed visibility 확장). Phase 10 task F에서 `"1.6"` bump 예정 (hash_io explicit emit, 코드 변경은 task F~L scope).
- `serialize(report, pretty)` 함수 구현 완료.
- `FileEntry`에 `diff_meaningful` + `presence` field는 Phase 8 task F~I에서 신규 추가 예정.

## 작업 범위

### 스키마 v1.7 (전체)

> 아래 sample은 `--summary-only` 미지정 시 전체 mode 출력. v1.7 는 신규 최상위 field `renames: [RenamePair]` 추가 — 그 외 entry/field wire 형식은 v1.6 과 byte-identical (`schema_version` 값만 다름). summary-only mode shape 는 § `--summary-only` 출력 참조 (`renames` 필드 omit).

```json
{
  "schema_version": "1.7",
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
  ],
  "renames": [
    {
      "from": "tree-a/sub-x/file.md",
      "to":   "tree-c/sub-x/file.md",
      "sha":  "ghi...",
      "raw_equal": true
    },
    {
      "from": "old/has-bom.md",
      "to":   "new/no-bom.md",
      "sha":  "jkl...",
      "raw_equal": false
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

**SemVer 면제 근거** — 위 backward-compat 표에 정리된 v1.4 caller `files == null` 또는 `"files"` key 부재로 summary-only mode 판단하는 분기는 v1.4 시점 도입된 신규 가정이라 SemVer 보호 대상 아님. v1.5 의 minor 라벨은 wire-shape 기준 정합 (신규 field 0 + enum 0, summary-only mode 한정 contract 확장).

### v1.5 → v1.6 변경 (minor)

post-v0.5.0 clean-context audit Finding 2 motivation 해소 — `failed_reason == hash_io` entry 의 wire 형식 정합화. 결정 trail은 git history (`git log --grep="Phase 10"`) + `CHANGELOG.md` § [0.6.0].

변경 범위 (전체 mode + `--summary-only` mode 양쪽 적용):

- 기존 (v1.5까지) — `failed_reason == hash_io` entry 는 `failed_reason` 필드 자체를 omit (`Option::None` 특수 케이스 sentinel). caller 는 key 부재로 hash_io 의미 판단. summary-only mode 의 minimal entry shape 가 `path` + `presence` 두 field 로 격하 (다른 reason entry 의 3 field 와 발산).
- v1.6 — `failed_reason == hash_io` entry 도 다른 reason entry 와 동일하게 `failed_reason: "hash_io"` 명시 emit. summary-only mode 의 minimal entry shape 가 `path` + `presence` + `failed_reason` 3 field 로 일관 (v1.5 의 2 field special case 제거). 전체 mode 에서도 hash_io entry 가 `failed_reason: "hash_io"` 필드 포함하여 emit — v1.5 의 omit 동작 제거.
- 그 외 reason entry (`encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported` / `file_too_large` / `memory_exceeded`) 의 wire 형식 변경 0 — v1.5 와 byte-identical.
- 비-Failed entry (`identical` / `local_only_changed` / `remote_only_changed` / `drift`) 의 wire 형식 변경 0 — v1.5 와 byte-identical.

신규 field 0 / 신규 enum 0:
- `failed_reason` 필드는 v1.2 부터 정의된 11 cover (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported` / `file_too_large` / `memory_exceeded`) 그대로. `hash_io` 는 v1.2 부터 enum 으로 정의돼 있었으나 v1.5 까지 wire 에서 omit 되던 special case — v1.6 는 wire 형식만 변경, enum 값 추가/제거 0.
- `presence` 필드는 v1.3 에서 도입된 enum 3 variant (`local_only` / `both` / `remote_only`) 그대로.

내부 구현 정합 (informative — wire spec 영향 X):
- `FailedReason` enum 정의에서 `Option::None == hash_io signal` 특수 케이스 제거 + 명시 `HashIo` variant 추가 + serde rename `"hash_io"` 정합. `Option<FailedReason>` 시그니처는 유지 (Failed 외 entry 는 여전히 None) — variant 만 변경. 자세한 enum 정의는 `docs/specs/spec-error-contracts.md` § FailedReason 정의.

hash_io entry wire 예시 (v1.6, 전체 mode):

```json
{
  "path": "broken/permission.md",
  "status": "failed",
  "presence": "both",
  "failed_reason": "hash_io",
  "mode": "100644"
}
```

hash_io entry wire 예시 (v1.6, summary-only mode minimal entry):

```json
{
  "path": "broken/permission.md",
  "presence": "both",
  "failed_reason": "hash_io"
}
```

backward-compat:

- 추가 필드 0 + 추가 enum 0 이라 v1.0~v1.5 호출자가 v1.6 wire JSON 파싱 시 enum 매칭은 영향 0 (`Option<String>` 또는 `Option<FailedReason>` 시그니처는 `hash_io` 값 정상 deserialize). 단 hash_io entry 의 wire 등장 자체는 v1.5 와 다름.
- v1.5 까지 caller 코드가 hash_io entry 의 `failed_reason` 필드 omit 을 sentinel 로 가정한 분기가 있다면 v1.6 에서 깨질 수 있음 — hash_io entry 에서도 `failed_reason: "hash_io"` 필드가 등장한다.

  | v1.5 caller 분기 | v1.6 동작 |
  |---|---|
  | `"failed_reason" in entry` → hash_io 외 reason 으로 가정 (key 존재 = explicit reason 의미) | hash_io entry 도 true (변경됨, `entry.failed_reason == "hash_io"` value 추가 등장) |
  | `"failed_reason" not in entry` → hash_io 로 판단 (key 부재 sentinel) | hash_io entry 에서 false (변경됨, 명시 emit 되어 key 부재 가짜 sentinel 사라짐) |
  | `entry.failed_reason == "hash_io"` 명시 분기 | 정상 동작 (v1.6 권장 패턴, v1.5 에서는 dead code 였음) |

  caller migration: missing-key sentinel 금지, `failed_reason == "hash_io"` 명시 분기로 전환. v1.6 동작과 정합. enum 11 cover (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported` / `file_too_large` / `memory_exceeded`) 전부 명시 match arm 으로 분기.

**SemVer 면제 근거** — 위 backward-compat 표에 정리된 v1.5 caller 의 `failed_reason` 필드 부재 = hash_io sentinel 분기는 v1.5 시점 도입된 신규 가정이라 SemVer 보호 대상 아님 (`Option::None` 특수 케이스 omit 동작이 v1.5 까지 wire spec 이긴 했으나, sentinel 로서의 caller 코드 분기는 v1.5 의 minimal entry shape 본문과 함께 도입된 보조 가정). v1.6 의 minor 라벨은 wire-shape 기준 정합 (신규 field 0 + enum 0, hash_io entry 의 wire 형식 정합화 — sentinel omit → explicit emit).

**공통 면제 표 (모든 schema bump 적용)** — task A 의 v1.4→v1.5 면제 근거와 동일 패턴 mirror. 매 schema bump 마다 caller 가 "신규 시점 도입된 가정"을 분기 sentinel 로 사용한 경우 SemVer 보호 대상 아님 — 공통 면제 logic 일반화. 본 표는 v1.4→v1.5, v1.5→v1.6 둘 다 적용:

| schema bump | 신규 가정 분기 (sentinel) | 면제 근거 |
|---|---|---|
| v1.4 → v1.5 | `--summary-only` 응답의 `files` 필드 부재 → summary-only mode 판단 | v1.4 시점 도입된 가정 (caller 가 v1.4 baseline wire 동작 의존) |
| v1.5 → v1.6 | summary-only `files[]` entry 의 `failed_reason` 필드 부재 → hash_io 판단 | v1.5 시점 도입된 가정 (caller 가 v1.5 minimal entry omit 동작 의존) |
| v1.N → v1.(N+1) (forward-applicable) | 매 schema minor bump 직전 도입된 신규 wire 가정 (omit / sentinel / byte-shape 등) 을 caller 가 분기 sentinel 로 채택한 경우 | 다음 minor bump 시점에 자동 면제 — 신규 도입 시점부터 다음 bump 까지 caller migration window 1 bump 보장 |

향후 bump 도 동일 logic 일반화 — caller 자신의 명시 enum value 분기 (e.g., 명시 enum match arm, 명시 field value check) 또는 wire 형식 stable signal (e.g., `schema_version` 자체 분기) 에 의존하는 분기는 영향 0 — 면제 표 적용 대상 자체가 아니다 (매 bump 마다 정상 동작 보장).

### v1.6 → v1.7 변경 (minor)

v0.8.0 (issue #15) 폴더 재배치/파일 이동 hint 도입. 결정 trail은 `C:\Users\admin\.claude\plans\staged-cuddling-deer.md` + `CHANGELOG.md` § [0.8.0].

추가 필드 (기존 필드 변경 0):

- `renames: [RenamePair]` — 최상위 (envelope 레벨, `files` 다음). `local_only_changed` + `remote_only_changed` 쌍 중 도구가 hash-join 으로 매칭한 rename/move 후보 명단. 항상 emit (빈 배열도 key 존재 보장) — `--summary-only` mode 한정 omit. caller(사람 또는 AI) 가 hint 보고 `diff --remote-path <from> --remote-path <to>` 호출로 검증. 도구는 사실만 제공 (매핑 정합성/방향 판단은 caller).

RenamePair shape:

| field | JSON type | 의미 |
|---|---|---|
| `from` | `string` | `local_only_changed` entry 경로 (forward-slash 정규화 — `files[].path` 와 동일 정책). |
| `to` | `string` | `remote_only_changed` entry 경로 (forward-slash 정규화). |
| `sha` | `string` | 매칭의 키가 된 hash. Case A 는 `local_only.local_sha` (= `remote_only.remote_sha`), Case B 는 normalize 후 도출된 hash. |
| `raw_equal` | `bool` | `true` = Case A (raw `local_sha == remote_sha`), `false` = Case B (raw 다르지만 normalize 후 동일 — issue #1 cosmetic identical 검사를 cross-path 로 mirror). caller 가 두 케이스 구분 분기 가능한 1-bit 신호. |

매칭 정책:

- **Case A (same-sha)**: `local_only.local_sha == remote_only.remote_sha` hash-join. 추가 비용 0.
- **Case B (normalize-equal cross-path)**: Case A 로 unmatched 인 `remote_only_changed` entry 한정 — remote blob bytes 를 fetch → `prepare_for_hash` 동일 normalize 정책 적용 → hash 비교. 한 scan 내 동일 blob 중복 fetch 0 (기존 fetcher batch 동작 그대로 사용).

비용 정책 (회귀 가드):

- `local_only_changed` 개수 0 또는 `remote_only_changed` 개수 0 → Case A/B 둘 다 skip, blob fetch 0.
- unmatched `remote_only_changed` 개수 임계(256) 초과 → Case B skip + stderr 진단 한 줄 (`"renames: Case B skipped, unmatched remote_only_changed=<N> exceeds cap=256"`). `renames` 는 Case A 결과만 emit.
- 동기 fetch — 기존 fetcher concurrency 동작 그대로 사용 (별도 옵션 0).

1:N / N:1 충돌:

- 동일 hash 가 multiple `local_only_changed` 또는 multiple `remote_only_changed` 에 매칭되면 모든 쌍 emit. 도구는 사실만, 매핑 정합성 판단은 caller.

backward-compat:

- 추가 필드 1 + 추가 enum 0. v1.0~v1.6 호출자가 v1.7 JSON 파싱 시 `renames` 필드 무시 + 기존 필드 정상 동작. v1.6 까지 caller 코드가 `renames` 필드 부재 가정 분기를 도입하지 않았다면 영향 0.

`--summary-only` 상호작용:

- `--summary-only` 모드는 `renames` 필드 omit — summary-only 정체성 ("카운트 + 무엇이 실패했나 명단") 유지. PRD 검증 시나리오 13 (`"files"` 미포함) 동등 강도로 `"renames"` 미포함 보장.

`--status` 필터 상호작용:

- `renames` 는 `files[]` 와 직교 — `--status` 필터 무관 emit. `files[]` 가 필터링되어 일부 entry 가 빠지더라도 `renames` 의 `from`/`to` 는 원본 분류 (`local_only_changed`/`remote_only_changed`) 기준 그대로.

**SemVer 면제 근거** — `renames` 필드 부재를 caller 가 분기 sentinel 로 사용한 경우 v1.6 시점 도입된 신규 가정이라 SemVer 보호 대상 아님 (§ 공통 면제 표 정합). 단 본 PR 시점 v1.6 까지 caller 가 `renames` 필드를 분기 sentinel 로 채택한 사례 0 으로 가정 — caller migration window 본 bump 가 첫 도입.

### 안정성 보장
- `schema_version`: 호환성 깨는 변경 시 major 증가. v0.1은 `"1.0"`, Phase 5는 `"1.1"`, Phase 7은 `"1.2"`, Phase 8은 `"1.3"`, v0.4.2는 `"1.4"`, v0.5.0은 `"1.5"`, v0.6.0은 `"1.6"`, v0.8.0은 `"1.7"` (모두 minor — 신규 field 추가 또는 `Identical` 분류 정확화 또는 `--summary-only` 출력 contract 확장 또는 hash_io entry wire 형식 정합화 또는 `renames` hint 추가, 기존 필드 변경 0).
- `status` enum 동결: `identical` / `local_only_changed` / `remote_only_changed` / `drift` / `failed`. 추가는 minor 버전, 제거·이름 변경은 major. **Phase 5에서 새 status 미추가** — LFS/submodule/symlink는 모두 `failed` + `failed_reason` 분류.
- `failed_reason` enum 동결 정책: 추가는 minor, 제거·이름 변경은 major. Phase 5에서 정의된 9 reason (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported`) 모두 구현 (Phase 5.13 task AA, 2026-05-09 — `compare.rs::FailedReason` 8 variant + `None` special case `hash_io`). Phase 7에서 2 reason 추가 — `file_too_large` + `memory_exceeded` (총 11 reason, `compare.rs::FailedReason` 10 variant + `None` special case). v0.6.0/Phase 10에서 `HashIo` explicit variant 추가 + `None` special case 제거 — 총 11 reason, `compare.rs::FailedReason` 11 variant + 0 special case (Finding 2 결정 trail, § v1.5 → v1.6 변경 + `spec-error-contracts.md` § Per-file Pitfall Reasons 정합).
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

v1.7부터 동작 (v1.6과의 차이: `renames` 필드도 omit. v1.6 까지 동작은 § v1.5 → v1.6 변경 참조):

- 기본 — 전체 mode JSON에서 `files` 필드와 `renames` 필드 둘 다 omit (`null` 아니라 키 부재). `schema_version` / `scanned_at` / `repo` / `branch` / `local_root` / `summary` 등 다른 필드는 유지.
- failed status entry 존재 시 (`summary.failed > 0`) — `files[]`에 failed entry 한정 minimal 형식 emit. entry 한 개는 `path` + `presence` + `failed_reason` 세 field만 (sha/size/mode/diff_meaningful/lfs_pointer/size_bytes 등 detail field 모두 omit). 그 외 status entry (identical / local_only_changed / remote_only_changed / drift)는 emit 안 함.
- `--status` filter 동시 지정 시 — summary-only 정체성 우선, status filter 무시 (summary 카운트 + failed entry 명단 contract 유지). 즉 `--summary-only --status drift` 호출도 동일하게 failed entry만 emit.
- `failed_reason == hash_io` entry 도 다른 reason entry 와 동일하게 `failed_reason: "hash_io"` 명시 emit — v1.6 부터 wire 형식 일관화 (v1.5 까지의 omit special case 제거). v1.5 의 2 field special case 와의 차이는 § v1.5 → v1.6 변경 § 참조.

minimal entry shape 예시 (failed 2건 + `failed_reason == lfs_pointer` + `failed_reason == hash_io` 케이스):

```json
{
  "schema_version": "1.6",
  "scanned_at": "2026-05-12T10:30:00Z",
  "repo": "owner/name",
  "branch": "main",
  "local_root": "/path/to/dir",
  "summary": { "identical": 120, "local_only_changed": 3, "remote_only_changed": 0, "drift": 1, "failed": 2 },
  "files": [
    {
      "path": "vendor/lib.zip",
      "presence": "both",
      "failed_reason": "lfs_pointer"
    },
    {
      "path": "broken/permission.md",
      "presence": "both",
      "failed_reason": "hash_io"
    }
  ]
}
```

`status` 필드도 minimal entry에서 omit — summary-only `files[]` entry는 정의상 failed (v1.6 부터 `failed_reason` 필드는 hash_io 포함 모든 failed reason 에 대해 명시 emit, entry 등장 자체가 failed signal). 호출자 contract: "summary-only 응답의 `files[]` entry는 모두 failed로 해석".

**Caller 분기 정책 (Finding 3 강조)** — summary-only `files[]` entry 는 일반 mode entry 와 shape 발산 (`status` / `sha` / `size` / `mode` / `diff_meaningful` 등 detail field omit). caller 는 응답 shape 추론 금지, 자신의 `--summary-only` argument 기준 mode 분기.

caller-visible behavior change 영향은 § v1.4 → v1.5 변경 § + § v1.5 → v1.6 변경 § backward-compat 표 참조.

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

### v1.6 신규 (v0.6.0)

v0.6.0 (Phase 10) post-v0.5.0 clean-context audit Finding 2 결정 trail. 본 § 는 spec authoritative — 코드 변경은 git history + `CHANGELOG.md` § [0.6.0] 참조.

- `[AUTO]` `report.schema_version` == `"1.6"`.
- `[AUTO]` `--summary-only` + `summary.failed == N` (N > 0) + `failed_reason == hash_io` entry: `files[]` entry는 `path` + `presence` + `failed_reason: "hash_io"` 세 field emit. 이전 v1.5 의 `path + presence` 2 field special case (key 부재 sentinel) 제거 — hash_io 도 다른 reason 과 동일 3 field shape.
- `[AUTO]` `--summary-only` + `summary.failed == N` (N > 0) + 그 외 reason entry (`encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported` / `file_too_large` / `memory_exceeded`): `files[]` entry는 `path` + `presence` + `failed_reason: "<reason>"` 세 field 유지 (v1.5 와 byte-identical).
- `[AUTO]` v1.5 caller 가 v1.6 JSON 파싱 시 hash_io entry 정상 deserialize — `failed_reason` 필드 값 `"hash_io"` 명시 등장. v1.5 caller 의 `failed_reason: Option<String>` 시그니처는 `Some("hash_io")` 로 deserialize 정상 (enum 매칭 우회, `tests/scan_output_backward_compat.rs` V15 client × v1.6 sample 패턴).
- `[AUTO]` `--summary-only` `files[]` minimal entry 는 `status` 필드 omit 정책 유지 (Finding 3 강조 정합 — summary-only `files[]` entry 정의상 failed signal, caller 분기는 자신의 `--summary-only` argument 기준). v1.6 wire 형식 변경은 `failed_reason` 필드 한정.

### v1.7 신규 (v0.8.0)

v0.8.0 (issue #15) `renames` hint 도입. 본 § 는 spec authoritative — 코드 변경은 git history + `CHANGELOG.md` § [0.8.0] 참조.

- `[AUTO]` `report.schema_version` == `"1.7"`.
- `[AUTO]` `renames` 필드는 envelope 레벨 (최상위, `files` 다음) 에 항상 emit — `local_only_changed` / `remote_only_changed` 둘 다 0 인 케이스라도 빈 배열 (`renames: []`) 로 emit (key 부재 sentinel 분기 회피).
- `[AUTO]` Case A — `local_only_changed.local_sha` == `remote_only_changed.remote_sha` 인 쌍 → `renames` 에 `{from, to, sha, raw_equal: true}` emit. `from` = local_only 경로, `to` = remote_only 경로, `sha` = 매칭의 키.
- `[AUTO]` Case B — Case A unmatched `remote_only_changed` 의 remote blob bytes 를 fetch + `prepare_for_hash` normalize → `local_only_changed.local_sha` 와 일치하면 `renames` 에 `{from, to, sha, raw_equal: false}` emit. `sha` 는 normalize 후 hash 값.
- `[AUTO]` `from` / `to` 는 `files[].path` 와 동일한 forward-slash 정규화 — backslash 가 OS 원본 경로에 있어도 hint 값은 `/` 정규화 (G-004 + PR #16 `diff --remote-path` 입력 호환).
- `[AUTO]` 1:N / N:1 충돌 — 동일 hash 가 multiple `local_only_changed` 또는 multiple `remote_only_changed` 에 매칭되면 모든 쌍 emit.
- `[AUTO]` short-circuit 1 — `local_only_changed` 개수 0 또는 `remote_only_changed` 개수 0 → Case A/B 둘 다 skip, blob fetch 0회, `renames: []` emit.
- `[AUTO]` short-circuit 2 — unmatched `remote_only_changed` 개수 임계(256) 초과 → Case B skip + stderr 진단 한 줄 (`"renames: Case B skipped, unmatched remote_only_changed=<N> exceeds cap=256"`). `renames` 는 Case A 결과만 포함하여 emit.
- `[AUTO]` `--summary-only` 모드 — `renames` 필드 omit (key 부재). PRD 검증 시나리오 13 등가 — stdout 출력에 문자열 `"renames"` 미포함.
- `[AUTO]` `--status` 필터 — `renames` 는 `files[]` 와 직교, 필터 무관 emit. `files[]` 가 필터링되어 entry 일부가 빠지더라도 `renames.from` / `to` 는 원본 분류 기준 그대로.
- `[AUTO]` v1.0~v1.6 호출자가 v1.7 JSON 파싱 시 `renames` 필드 무시 + 기존 필드 정상 동작 (`tests/scan_output_backward_compat.rs` V16+ client × v1.7 sample 패턴 정합).

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
