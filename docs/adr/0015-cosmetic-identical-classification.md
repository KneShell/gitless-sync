# ADR 0015: cosmetic SHA drift → Identical classification

- **Status**: Accepted
- **Date**: 2026-05-11
- **Related**: `docs/specs/spec-classification.md` § Status 정의, `docs/specs/spec-output-schema.md` § v1.4, `docs/specs/spec-hash-and-normalize.md` § 원격 측 비교, `crates/gitless-sync/src/commands/scan/compare/decisions.rs::classify`, `crates/gitless-sync/src/commands/scan/pipeline/normalize_pass.rs::fetch_normalize_equal_map`, GitHub issue #1

## Context

Issue #1: byte 단위로 완전 동일한 파일이 `status: local_only_changed` (또는 `remote_only_changed`) + `presence: both` + `diff_meaningful: false`로 분류됨. spec-hash-and-normalize.md § 목적 ("byte 동일 → 동일 SHA → identical")과 어긋남.

근원: spec/code drift.

- spec 의도 (spec-hash-and-normalize.md § 원격 측 비교): "Trees API SHA 무시, 자체 정의 hash 정책 그대로 비교".
- code 실제: `remote_sha = TreeEntry.sha` (Trees API의 raw blob SHA). `local_sha = blob_hash(prepare_for_hash(local_raw))` (자체 hash). 두 hash 정책이 비대칭.
- 결과: byte 동일 파일도 normalize 정책 차이 (UTF-8 BOM / CRLF→LF / `.gitattributes`)로 raw SHA mismatch → LocalOnlyChanged 분류.

Phase 8 task I에서 `pipeline::normalize_pass`가 sha-differ Hashed entry 한정 fetch_blob + 자체 hash 재계산을 도입했으나, 결과는 `compare()` 함수의 `diff_meaningful` field 결정에만 들어갔고 `classify()` 함수의 status 결정에는 안 들어갔음 — 두 함수의 비대칭 처리.

## Decision

`classify` 함수에 `normalize_equal: Option<bool>` 인자 추가 + sha-differ + `Some(true)` 케이스에 `Status::Identical` arm 추가.

```rust
pub fn classify(
    local_sha: Option<&str>,
    remote_sha: Option<&str>,
    local_mtime: Option<DateTime<Utc>>,
    remote_last_commit_at: Option<DateTime<Utc>>,
    normalize_equal: Option<bool>,  // 신규
) -> Status {
    match (local_sha, remote_sha) {
        (Some(a), Some(b)) if a == b => Status::Identical,
        (Some(_), Some(_)) if normalize_equal == Some(true) => Status::Identical,  // 신규 arm
        ...
    }
}
```

`hashed_to_file_entry` caller가 `normalize_eq_map.get(path)` 결과를 classify에 전달.

## Alternatives 검토

### A. Full blob fetch per file (모든 sha 비교 시 fetch)
- 장점: spec 정합 100%, classify 변경 없음.
- 단점: per-file fetch + Phase 4 GraphQL batching 무효화. vault 1000 file이면 1000 blob fetch.
- 거부 이유: 정상 (byte 동일 + raw SHA 동일) 케이스에서 추가 비용 발생. wasteful.

### B. Spec rewrite (Trees SHA 그대로 사용 + status 정의 변경)
- 장점: 코드 변경 0.
- 단점: caller가 "byte 동일인데 LocalOnlyChanged" 의문 영구. v1.3 presence + diff_meaningful로 cover 가능하지만 신뢰도 ↓.
- 거부 이유: spec-hash-and-normalize.md § 목적 ("byte 동일 → identical")과 어긋남.

### C. Hybrid (현 채택): 1차 raw SHA 비교, mismatch 시 fetch + 자체 hash 재계산 → classify에 반영
- 장점: spec 정합 100% + 정상 케이스 비용 0 (mismatch 케이스만 fetch). caller 입장에서 SHA의 의미가 항상 동일 (자체 hash). v1.3 field semantic 그대로.
- 단점: vault 전체 cosmetic drift 시 N개 fetch (사용자 vault 6 file 수준이면 무시 가능).
- 채택 이유: 정합 + caller 친화 + 비용 균형 최우.

### D. `.gitattributes` 매칭 시만 자체 hash
- 거부: default 케이스 (markdown 등) 가 issue #1 핫스팟이라 본 issue 안 풀음.

## Consequences

### Code

- `compare/decisions.rs::classify` signature change (5번째 인자 `normalize_equal: Option<bool>`).
- `pipeline/finalize/pre_entry.rs::hashed_to_file_entry` caller 갱신 (normalize_equal 전달).
- `output.rs::SCHEMA_VERSION` 1.3 → 1.4 minor bump.
- `hash_remote.rs` 코멘트 line 4-5 정정 (outdated, normalize_pass에서 fetch_blob 사용).

### Spec

- `spec-classification.md` § Status 정의 + § classify 시그니처 + § 판정 로직 + § Acceptance Criteria 갱신.
- `spec-output-schema.md` § 안정성 보장 + § v1.4 신규 acceptance section 추가.
- `spec-hash-and-normalize.md` § 원격 측 비교 정확화 (1차 raw SHA + mismatch 시 자체 hash 재계산 흐름).

### Backward compatibility

- v1.0 / v1.1 / v1.2 / v1.3 caller 모두 v1.4 JSON 파싱 정상.
- `status` enum 동결 (변경 없음).
- 의미 변화: cosmetic drift 케이스가 `Identical` 분류 (이전 LocalOnlyChanged/RemoteOnlyChanged/Drift). caller 입장에서 더 정확한 결과.
- additive 의미 정확화 — breaking change 아님.

### Performance

- 정상 케이스 (raw SHA 동일): 추가 fetch 0.
- Cosmetic drift 케이스 (raw SHA 다름 + normalize-equal): mismatch entry당 1 blob fetch (`pipeline::normalize_pass` sequential).
- Real semantic drift 케이스 (raw SHA 다름 + normalize-diff): 같은 mismatch entry당 1 blob fetch (이미 normalize_pass에서 발생, 추가 비용 0).
- 측정 — vault 1000 file 중 cosmetic drift 6 file 수준이면 6 blob fetch (~6초 wall clock).

## References

- GitHub issue #1: byte-identical files classified as cosmetic
- `docs/specs/spec-classification.md` § Status 정의 (v1.4)
- `docs/specs/spec-output-schema.md` § v1.4 신규
- `docs/specs/spec-hash-and-normalize.md` § 원격 측 비교
