# Spec: 4-State Classification

## 목적
로컬과 원격의 (자체) SHA + 시간 메타로부터 파일을 4상태로 분류한다. 분류는 도구의 본질 기능이며, 결과는 호출자(사람 또는 AI)가 다음 액션 결정의 입력으로 사용한다.

## 현재 상태
- `crates/gitless-sync/src/commands/scan/compare.rs::Status` enum 정의 완료 (`Identical`, `LocalOnlyChanged`, `RemoteOnlyChanged`, `Drift`, `Failed`).
- `FileEntry` 구조체 + serde 직렬화 완료.
- `classify` 함수 박힘 — spec § 판정 로직 의사코드와 정합 (양쪽 SHA 동률→Identical / 한쪽 None→Local|RemoteOnlyChanged / 양쪽 다른 SHA + mtime 비교 / 동률 또는 None→Drift / 양쪽 None→panic).
- **NFC 정규화 박힘 (Phase 5 task C)**: `shared/path.rs::to_nfc` + `walker.rs::relative_path` (line 92, local) + `shared/github/trees.rs` (line 63/75/87, remote 3 mode). 양쪽 boundary에서 normalize → 비교 key align.
- **case_collision 박힘 (Phase 5 task D/D1)**: `compare.rs::FailedReason::CaseCollision` + `case_collision.rs::detect` (canonical/diagonal/local-both 3 시나리오 symmetric).

## 작업 범위

### 상태 정의

| Status | 조건 |
|--------|------|
| `identical` | 양쪽 자체 SHA 동일 |
| `local_only_changed` | 원격에 없거나 SHA 다름 + `remote_last_commit_at ≤ local_mtime`. push 후보. |
| `remote_only_changed` | 로컬에 없거나 SHA 다름 + `local_mtime ≤ remote_last_commit_at`. pull 후보. |
| `drift` | 양쪽 SHA 다름 + 시간 비교로 한쪽 우위 판단 불가. 충돌 의심. |
| `failed` | 해시 계산 실패 등 부분 실패 (`spec-error-contracts.md`). |

### `classify` 함수 시그니처 (이미 박힘)
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

비교 key 박는 path는 다음 정규화 거침:

- **NFC 정규화**: 모든 path bytes를 Unicode NFC로 정규화 후 비교 key 박음. macOS HFS+/APFS NFD 저장(default `core.precomposeunicode = true`로 NFD → NFC 자동 변환) + GitHub Trees API path bytes 그대로 반환 — 우리 NFC 정규화로 양쪽 align.
- **case-sensitive 비교**: Unix-style. `README.md` ≠ `Readme.md` (다른 path key). Windows NTFS는 case-insensitive로 동일 file 취급하지만 도구는 case-sensitive 그대로 박음 — drift로 표면화하는 게 정합.
- **edge case**:
  - macOS `core.precomposeunicode = false` + NFC/NFD 동일 path 두 개 (예: `가.txt` NFC + `가.txt` NFD) → NFC 정규화 후 두 path 같은 key 충돌 → `Status::Failed` + `failed_reason: "nfd_collision"`. **99% 케이스는 NFC 정규화로 자동 처리, 1% edge case detect-only는 Phase 5 후속** (`compare.rs::FailedReason::NfdCollision` enum variant + `pipeline.rs::try_short_circuit_failed` 매핑 미박힘 — task N (`spec-error-contracts.md` `failed_reason` enum) 박힌 후 implement task로 박음). spec-domain-pitfalls.md § Path 정규화 hedge 정합.
  - Windows NTFS local-side에서 같은 case-folded name 두 file 박힌 case (`Foo.txt` + `foo.txt`) → `Status::Failed` + `failed_reason: "case_collision"` (박힘).

자세한 처리 정책은 `docs/specs/spec-domain-pitfalls.md` § Path 정규화 참조.

### `.gitignore` 무시 정책 (Phase 5)

scan 범위는 다음 ignore 룰의 합집합 외 path:

- `.gitignore` (project root + 하위 디렉토리)
- `--ignore` CLI 인자 (gitignore-style pattern)
- 도구 내장 (`.git/`, `target/`, `node_modules/`)

ignored path는 비교 대상 자체에서 제외 — `summary` 카운트에도 박지 않음. spec은 `spec-ignore-policy.md`.

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
