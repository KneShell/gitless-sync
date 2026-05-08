# Phase 6 Baseline Metrics

> Snapshot at task R commit time (2026-05-08). 분할 전 측정. Phase 6 진행 중 task M에서 LOC/fan-out/cycle 등 추가 metric을 본 파일에 누적, J 직후 분할 후 측정으로 갱신.
>
> **Update (2026-05-08, task S 완료)**: production `expect` 2건 모두 `Config` map_err로 fix → baseline 위반 0건 도달. § Baseline Violation Count + § 위반 위치 표 갱신. task T (warn → deny 전환) 즉시 진행 가능.
>
> **Update (2026-05-08, task T 완료)**: workspace lint `unwrap_used`/`expect_used`/`panic` warn → **deny** 전환. `cargo clippy --workspace --all-targets -- -D warnings` 통과 + 233 tests pass + tarpaulin 87.77%. 향후 production 위반은 `-D warnings` 의존 없이 빌드 fail로 박힘.

## Panic Escape Hatch — `unwrap_used` / `expect_used` / `panic`

### Measurement Setup

- workspace `Cargo.toml` `[workspace.lints.clippy]` (task T 이후):
  - `unwrap_used = "deny"` (warn 시점 baseline 측정 후 task T에서 deny 전환)
  - `expect_used = "deny"` (동일)
  - `panic = "deny"` (동일)
- Test 면제 게이트 (file 상단):
  - `crates/gitless-sync/src/lib.rs:6` — `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]`
  - `crates/gitless-sync/src/main.rs:1` — `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]`
  - `crates/gitless-sync/tests/integration.rs:1` — `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`
- 검증 명령: `cargo clippy --workspace --all-targets -- -D warnings`

### Verification

`cargo clippy --workspace --all-targets -- -D warnings` 통과 (2026-05-08 hard gate). 워크스페이스 lint warn level이지만 `-D warnings`가 production unsuppressed 위반을 즉시 fail로 바꿈. 통과는 = production 노출 위반 0건 의미.

### Baseline Violation Count

| lint | production 위반 (suppressed) | tests/binary tests | 합계 |
|---|---|---|---|
| `clippy::unwrap_used` | 0 | (file-level allow로 면제) | 0 |
| `clippy::expect_used` | **0** (task S에서 fix 완료) | (file-level allow로 면제) | 0 |
| `clippy::panic` | 0 | (file-level allow로 면제) | 0 |

**Total production 위반: 0건** (task S 완료, 2026-05-08 — task T warn→deny 전환 게이트 통과).

### 위반 위치 (production only, task S 처리 결과)

| # | 위치 | 처리 전 | 처리 후 (task S) |
|---|---|---|---|
| 1 | `crates/gitless-sync/src/commands/scan/mod.rs:70` | `output::serialize(&report, args.pretty).expect("ScanReport serialization is total")` + `#[allow(clippy::expect_used)]` | `output::serialize(&report, args.pretty).map_err(\|e\| GitlessError::Config(format!("ScanReport JSON serialization failed: {e}")))?` (allow 제거). 호출자 view에서 doc `# Panics` 블록 제거 + `# Errors`로 흡수. |
| 2 | `crates/gitless-sync/src/commands/scan/mod.rs:415` | `rayon::ThreadPoolBuilder::new().num_threads(MAX_COMMITS_CONCURRENCY).build().expect("rayon thread pool build")` + `#[allow(clippy::expect_used)]` | `.build().map_err(\|e\| GitlessError::Config(format!("rayon thread pool build failed: {e}")))?` (allow 제거). spec § Config "환경 문제" 정합 — gh CLI 미설치 매핑과 동일 카테고리. |

**선택 근거** (advisor §1 reconcile + spec-architecture.md § Panic escape hatch):
- Row 1 (serialize)은 spec table이 `unreachable!()` 또는 `Err(...)` 둘 다 안전한 alternative로 박음. `Config` map_err는 Row 2와 일관 + 미래 schema 변경 시 silent panic 차단.
- Row 2 (rayon)는 baseline 표가 명시적으로 `Config` 권고. exit 1 (Config) = "기타" 환경 실패 매핑 일관.
- 두 Row 모두 `Io(io::Error::other(...))` 합성 대안은 기각 — `Io` spec은 "로컬 디렉토리 walk / 파일 read 시 IO 실패"로 도메인 좁고, 두 케이스 모두 IO 아닌 in-memory/system 자원 실패.

### Test 면제 검증

production code (`#[cfg(test)] mod tests` 외부) grep으로 unwrap/expect/panic 후보 line 추출 후 test mod start line과 비교. 모든 후보가 `#[cfg(test)] mod tests { ... }` 블록 내부에 위치 — 면제 누수 0건. (task S에서 production allow 2건 모두 제거됨.)

```
crates/gitless-sync/src/commands/diff/mod.rs        — #[cfg(test)] @ 141 (이후 모두 test)
crates/gitless-sync/src/commands/init/mod.rs        — #[cfg(test)] @ 59
crates/gitless-sync/src/commands/scan/mod.rs        — #[cfg(test)] @ 434 (이후 모두 test, production allow 0건)
crates/gitless-sync/src/commands/scan/walker.rs     — #[cfg(test)] @ 85
crates/gitless-sync/src/commands/scan/graphql.rs    — #[cfg(test)] @ 203
crates/gitless-sync/src/shared/github.rs            — #[cfg(test)] @ 225
crates/gitless-sync/src/shared/gh.rs                — #[cfg(test)] @ 75/81/94/106 (MockGhClient gating + 107@mod tests)
crates/gitless-sync/src/shared/config.rs            — #[cfg(test)] @ 36
crates/gitless-sync/src/shared/ignore.rs            — #[cfg(test)] @ 68
```

### 다음 단계

- **task S**: ✅ 완료 (2026-05-08). production `expect` 2건 → `Config` map_err로 fix + `#[allow]` 제거. baseline 위반 0건 도달.
- **task T**: ✅ 완료 (2026-05-08). workspace lint warn → deny 전환 박음. `cargo clippy --workspace --all-targets -- -D warnings` 통과 + 233 tests pass + tarpaulin 87.77%. Phase 6.3 panic 검출 trilogy (R → S → T) 완결.

## (placeholder) 분할 metrics — task M에서 박음

- file 수, total LOC, max LOC, fan-out (file별 import count), cycle count: F-I 분할 직전/직후 비교를 위해 task M이 본 파일에 누적.
