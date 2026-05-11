# Spec: 4-State Classification

## 목적
로컬과 원격의 (자체) SHA + 시간 메타로부터 파일을 4상태로 분류한다. 분류는 도구의 본질 기능이며, 결과는 호출자(사람 또는 AI)가 다음 액션 결정의 입력으로 사용한다.

## 현재 상태
- `crates/gitless-sync/src/commands/scan/compare.rs::Status` enum 정의 완료 (`Identical`, `LocalOnlyChanged`, `RemoteOnlyChanged`, `Drift`, `Failed`).
- `FileEntry` 구조체 + serde 직렬화 완료.
- `classify` 함수 구현됨 — spec § 판정 로직 의사코드와 정합 (양쪽 SHA 동률→Identical / 한쪽 None→Local|RemoteOnlyChanged / 양쪽 다른 SHA + mtime 비교 / 동률 또는 None→Drift / 양쪽 None→panic).
- **NFC 정규화 구현됨 (Phase 5 task C)**: `shared/path.rs::to_nfc` + `walker.rs::relative_path` (line 92, local) + `shared/github/trees/classify.rs::to_nfc`. 양쪽 boundary에서 normalize → 비교 key align.
- **case_collision 구현됨 (Phase 5 task D/D1)**: `compare.rs::FailedReason::CaseCollision` + `case_collision.rs::detect` (canonical/diagonal/local-both 3 시나리오 symmetric).

## 작업 범위

### 상태 정의

| Status | 조건 |
|--------|------|
| `identical` | 양쪽 자체 SHA 동일 |
| `local_only_changed` | 원격에 없거나 SHA 다름 + `remote_last_commit_at ≤ local_mtime`. push 후보. |
| `remote_only_changed` | 로컬에 없거나 SHA 다름 + `local_mtime ≤ remote_last_commit_at`. pull 후보. |
| `drift` | 양쪽 SHA 다름 + 시간 비교로 한쪽 우위 판단 불가. 충돌 의심. |
| `failed` | 해시 계산 실패 등 부분 실패 (`spec-error-contracts.md`). |

### `classify` 함수 시그니처 (구현됨)
```rust
pub fn classify(
    local_sha: Option<&str>,
    remote_sha: Option<&str>,
    local_mtime: Option<DateTime<Utc>>,
    remote_last_commit_at: Option<DateTime<Utc>>,
) -> Status
```

### 판정 로직 (의사코드)
```
match (local_sha, remote_sha) {
    (Some(a), Some(b)) if a == b => Identical
    (Some(_), None)  => LocalOnlyChanged   // 원격 없음
    (None, Some(_))  => RemoteOnlyChanged  // 로컬 없음
    (Some(_), Some(_)) => {                // 양쪽 있고 SHA 다름
        match (local_mtime, remote_last_commit_at) {
            (Some(l), Some(r)) if r < l => LocalOnlyChanged
            (Some(l), Some(r)) if l < r => RemoteOnlyChanged
            _                            => Drift  // 동률 또는 한쪽 None
        }
    }
    (None, None) => unreachable! (호출 측 책임)
}
```

### 시간 비교 함정 (G-005)
- `local_mtime == remote_last_commit_at` 동률은 무조건 `Drift`.
- 로컬 mtime은 touch / 복사 / iCloud 메타로 갱신되어 단조성 없음. 시간 비교는 휴리스틱일 뿐.

### Path 정규화 (Phase 5)

비교 key가 되는 path는 다음 정규화 거침:

- **NFC 정규화**: 모든 path bytes를 Unicode NFC로 정규화 후 비교 key 생성. macOS HFS+/APFS NFD 저장(default `core.precomposeunicode = true`로 NFD → NFC 자동 변환) + GitHub Trees API path bytes 그대로 반환 — 우리 NFC 정규화로 양쪽 align.
- **case-sensitive 비교**: Unix-style. `README.md` ≠ `Readme.md` (다른 path key). Windows NTFS는 case-insensitive로 동일 file 취급하지만 도구는 case-sensitive 그대로 적용 — drift로 표면화하는 게 정합.
- **edge case**:
  - macOS `core.precomposeunicode = false` + NFC/NFD 동일 path 두 개 (예: `가.txt` NFC + `가.txt` NFD) → NFC 정규화 후 두 path 같은 key 충돌 → `Status::Failed` + `failed_reason: "nfd_collision"`. 구현됨 (Phase 5.13 task AA, `commands/scan/nfd_collision.rs::detect` walker output Vec에서 같은 NFC key count ≥ 2 group-by + `pipeline::try_short_circuit_failed` cascade 첫 분기).
  - Windows NTFS local-side에서 같은 case-folded name 두 file (`Foo.txt` + `foo.txt`) → `Status::Failed` + `failed_reason: "case_collision"` (구현됨).

자세한 처리 정책은 `docs/specs/spec-domain-pitfalls.md` § Path 정규화 참조.

### `.gitignore` 무시 정책 (Phase 5)

scan 범위는 다음 ignore 룰의 합집합 외 path:

- `.gitignore` (project root + 하위 디렉토리)
- `--ignore` CLI 인자 (gitignore-style pattern)
- 도구 내장 (`.git/`, `target/`, `node_modules/`)

ignored path는 비교 대상 자체에서 제외 — `summary` 카운트에도 포함 안 함. spec은 `spec-ignore-policy.md`.

### Cascade priority (Phase 5.13.1 task FF)

`Status::Failed` 격하는 두 단계로 나뉜다 — **pre-hash cascade**와 **post-read encoding**. 동일 path가 둘 다 surface 가능한 경우 항상 **cascade가 우선**이다.

**Pre-hash cascade** — `commands/scan/pipeline/short_circuit.rs::try_short_circuit_failed`. local read 전에 path/mode/`.gitattributes` 메타만 보고 격하. 우선순위는 코드의 if-else chain 순서 그대로:

| priority | reason | trigger |
|---|---|---|
| 1 | `nfd_collision` | `cctx.nfd_collisions` 멤버 |
| 2 | `case_collision` | `cctx.case_collisions` 멤버 |
| 3 | `long_path` | `long_path::is_invalid` (DOS 예약명 또는 260자+) |
| 4 | `submodule` | `remote.mode == "160000"` |
| 5 | `symlink` | `remote.mode == "120000"` 또는 `local.is_symlink` |
| 6 | `lfs_pointer` | `.gitattributes` `AttributeMatch::LfsPointer` |
| 7 | `gitattributes_unsupported` | `.gitattributes` `AttributeMatch::Unsupported` |

위 7 arm 중 가장 먼저 매칭되는 reason이 surface — cascade는 단일 if-else chain으로 priority 1이 가장 강함. 두 arm이 동시 fire 가능한 fixture (예: priority 3 long_path + priority 6 lfs_pointer)는 priority 3이 surface. cascade는 이 우선순위를 코드 구조 자체로 enforce — 추가 정렬/감리 layer 없음.

**Post-read encoding** — `commands/scan/hash_local.rs::try_hash_local`. cascade가 `None`을 반환해야만 진입. raw bytes 읽은 후 `try_decode_text` 결과가 `Utf16Bom` 또는 `Unknown`이면 `failed_reason: "encoding"` 마크.

**Encoding은 cascade 외부 (구조적 priority)**:

- **위치**: encoding detection은 raw read 이후에만 가능 (decoder 입력이 bytes 본체 필요). cascade는 raw read **전**에 동작 — 두 단계는 시점이 다르다.
- **invariant**: cascade가 `Some(reason)` 반환 → `build_one_pre_entry`가 `try_hash_local` 호출 자체를 차단 → encoding은 영영 measure되지 않음. 즉 cascade 7 reason 중 어느 것이라도 surface하면 encoding은 같은 path에서 동시 surface 불가.
- **의도**: cascade reason은 path/mode/`.gitattributes` 메타로 충분히 결정되는 함정 — local read 비용을 굳이 발생시킬 이유 없음. encoding은 본질적으로 raw read 결과 — cascade에 끼워 넣을 데이터가 없음 (`try_short_circuit_failed`의 `cctx`에 raw bytes 없음).

**호출자가 알아야 할 사실**:
- 동일 path에서 cascade reason과 encoding이 동시 가능한 fixture (예: `*.psd filter=lfs` + UTF-16 BOM raw bytes)에서는 항상 cascade reason이 wire JSON에 surface. encoding은 누락.
- 이 priority는 spec 정의 — `try_short_circuit_failed` cascade arm 순서 변경 또는 encoding을 cascade 안으로 옮기는 변경은 spec § Cascade priority 갱신 동반.

자세한 구현 정합은 `commands/scan/pipeline/{short_circuit, hash_pass}.rs` 참조. encoding 정책 본체는 `spec-domain-pitfalls.md` § Encoding 변환 시도.

## Acceptance Criteria
- `[AUTO]` PRD 검증 시나리오 1: 양쪽 SHA 동일 → `Identical`.
- `[AUTO]` PRD 검증 시나리오 2: 로컬 변경(원격 last_commit < 로컬 mtime) → `LocalOnlyChanged`.
- `[AUTO]` PRD 검증 시나리오 3: 원격 변경(로컬 mtime < 원격 last_commit) → `RemoteOnlyChanged`.
- `[AUTO]` PRD 검증 시나리오 4: 양쪽 다른 SHA + 시간 동률 → `Drift`.
- `[AUTO]` 로컬만 있는 파일 (`remote_sha == None`) → `LocalOnlyChanged`.
- `[AUTO]` 원격만 있는 파일 (`local_sha == None`) → `RemoteOnlyChanged`.
- `[AUTO]` 양쪽 다른 SHA + `local_mtime == remote_last_commit_at` → `Drift` (G-005).
- `[AUTO]` 양쪽 다른 SHA + 시간 정보 누락(한쪽 None) → `Drift` (시간 비교 불가).
- `[AUTO]` 모든 케이스에 대해 unit test (`compare::tests::*`) 작성. 라인 커버리지에 기여.
- `[AUTO]` Cascade priority — `pipeline/short_circuit.rs` cascade arm 순서가 spec § Cascade priority 표와 정합. priority 간 동시 fire 가능 fixture에서 상위 priority가 surface (예: priority 3 long_path > priority 6 lfs_pointer, priority 1 nfd_collision > priority 6 lfs_pointer). lock test는 `pipeline/short_circuit.rs::tests`.
- `[AUTO]` Encoding cascade 외부 — `try_short_circuit_failed`가 LFS filter 매칭 path에서 `Some(LfsPointer)` 반환 → `build_one_pre_entry`가 `try_hash_local` 미호출 → encoding은 같은 path에서 동시 surface 불가. lock test는 `pipeline/short_circuit.rs::tests` (`lfs_pointer_via_cascade_locks_out_post_read_encoding`).
