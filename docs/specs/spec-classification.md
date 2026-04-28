# Spec: 4-State Classification

## 목적
로컬과 원격의 (자체) SHA + 시간 메타로부터 파일을 4상태로 분류한다. 분류는 도구의 본질 기능이며, 결과는 호출자(사람 또는 AI)가 다음 액션 결정의 입력으로 사용한다.

## 현재 상태
- `crates/gitless-sync/src/commands/scan/compare.rs::Status` enum 정의 완료 (`Identical`, `LocalOnlyChanged`, `RemoteOnlyChanged`, `Drift`, `Failed`).
- `FileEntry` 구조체 + serde 직렬화 완료.
- `classify` 함수는 시그니처만 있음 (`todo!()`).

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
