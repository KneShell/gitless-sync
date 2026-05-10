# ADR 0012: 큰 파일 임계치 — file_too_large (100MB) + memory_exceeded (50MB)

- **Status**: Accepted
- **Date**: 2026-05-10
- **Related**: `docs/specs/spec-hash-and-normalize.md` § Phase 7 — 큰 파일 처리, `docs/specs/spec-error-contracts.md` § Per-file Pitfall Reasons, `docs/specs/spec-output-schema.md` § v1.2 신규, `docs/research/phase7-vague.md`

## Context

v0.2.x까지는 파일 size 한도 미정. local fs::read는 RAM 한계까지 동작 + remote fetch_blob은 gh subprocess 한도까지 동작 — 큰 파일 (10MB+) 발생 시 메모리 사용량 측정 + 임계치 결정 필요 (`docs/roadmap.md` § Open Questions).

GitHub Blobs API는 100MB 단일 파일 hard limit (fact check 2026-05-10, [source: https://docs.github.com/en/rest/git/blobs]). tool 메모리 안전 임계는 raw bytes + base64 encoded + SHA-1 buffer 3중 사용 worst case 기준 50MB.

## Decision

reason enum 분리: `file_too_large` (100MB API 한도) + `memory_exceeded` (50MB tool 메모리 임계). schema v1.1 → v1.2 minor bump.

### 임계치

| reason | 임계치 | 근거 |
|---|---|---|
| `file_too_large` | 100 MB | GitHub Blobs API hard limit (fact check). 100 MB 초과 파일은 도구 비교 불가 — remote 자체 fetch 불가능. |
| `memory_exceeded` | 50 MB | raw + base64 + SHA-1 buffer 3중 메모리 사용 → 50 MB raw → 약 200 MB 메모리 worst case. 1 GB RAM 머신 안전 cap. |

50 MB 임계는 추정값 — Phase 8+ 측정 trigger 발생 시 조정 가능.

### 검출 시점

- local: `fs::metadata().len()` pre-flight (file read 자체 회피).
- remote: Trees response size field pre-flight (fetch_blob 호출 자체 회피).
- post-flight 검증은 yagni — Trees response는 size field 항상 포함이라 pre-flight로 99% cover (memory `feedback_quality_vs_complexity.md` 정합).

### Cascade 우선순위

LFS pointer > size check (Phase 5 spec 정합). 100MB 미만 LFS pointer text는 Phase 5 spec대로 `lfs_pointer` 분류 — size check는 LFS 미감지 entry에 한해 적용.

## Consequences

- spec-hash-and-normalize.md § Phase 7 — 큰 파일 처리 신규 §.
- spec-error-contracts.md § Per-file Pitfall Reasons 표에 2 row 추가.
- spec-output-schema.md schema v1.1 → v1.2 minor bump (failed_reason enum 9 → 11 + size_bytes diagnostic field).
- 신규 unit test 4 시나리오 (49MB / 51MB / 101MB / LFS 우선순위).
- backward-compat 검증 — v1.0/v1.1 호출자가 v1.2 JSON 파싱 시 추가 필드 + 추가 enum 무시 + 기존 필드 정상 동작.

## References

- `docs/specs/spec-hash-and-normalize.md` § Phase 7 — 큰 파일 처리
- `docs/specs/spec-error-contracts.md` § Per-file Pitfall Reasons
- `docs/specs/spec-output-schema.md` § v1.2 신규
- `docs/research/phase7-vague.md`
- [source: https://docs.github.com/en/rest/git/blobs] (2026-05-10 fact check)
