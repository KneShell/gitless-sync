# Phase 6 Baseline Metrics

> Snapshot at task R commit time (2026-05-08). 분할 전 측정. Phase 6 진행 중 task M에서 LOC/fan-out/cycle 등 추가 metric을 본 파일에 누적, J 직후 분할 후 측정으로 갱신.

## Panic Escape Hatch — `unwrap_used` / `expect_used` / `panic`

### Measurement Setup

- workspace `Cargo.toml` `[workspace.lints.clippy]`:
  - `unwrap_used = "warn"`
  - `expect_used = "warn"`
  - `panic = "warn"`
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
| `clippy::expect_used` | **2** (`#[allow]` 임시 박힘) | (file-level allow로 면제) | 2 |
| `clippy::panic` | 0 | (file-level allow로 면제) | 0 |

**Total production 위반: 2건** (전부 `expect_used`, 임시 `#[allow(clippy::expect_used)]` attr로 suppressed).

### 위반 위치 (production only, task S 처리 대상)

| # | 위치 | 호출 | 사유 |
|---|---|---|---|
| 1 | `crates/gitless-sync/src/commands/scan/mod.rs:70` | `output::serialize(&report, args.pretty).expect("ScanReport serialization is total")` | `Serialize` 타입은 fallible impl 없음 — 호출자 입장에선 total. 그러나 `expect`는 panic 경로 — task S에서 `?` + `anyhow::Context` 또는 `unreachable!()` 대체 검토. |
| 2 | `crates/gitless-sync/src/commands/scan/mod.rs:419` | `rayon::ThreadPoolBuilder::new().num_threads(MAX_COMMITS_CONCURRENCY).build().expect("rayon thread pool build")` | rayon thread pool build 실패는 OS 자원 고갈 등 hard environmental issue — task S에서 `GitlessError::Config` 또는 새 variant 검토. |

위 2건은 `#[allow(clippy::expect_used)]` 임시 attribute가 같은 줄 위에 박혀 있어 clippy가 silently 통과. task S에서 진짜 fix + allow 제거 + task T에서 warn → deny 전환.

### Test 면제 검증

production code (`#[cfg(test)] mod tests` 외부) grep으로 unwrap/expect/panic 후보 line 추출 후 test mod start line과 비교. 모든 후보가 `#[cfg(test)] mod tests { ... }` 블록 내부 또는 위 2건의 production allow 안에 위치 — 면제 누수 0건.

```
crates/gitless-sync/src/commands/diff/mod.rs        — #[cfg(test)] @ 141 (이후 모두 test)
crates/gitless-sync/src/commands/init/mod.rs        — #[cfg(test)] @ 59
crates/gitless-sync/src/commands/scan/mod.rs        — #[cfg(test)] @ 434 (70/419는 < 434, production)
crates/gitless-sync/src/commands/scan/walker.rs     — #[cfg(test)] @ 85
crates/gitless-sync/src/commands/scan/graphql.rs    — #[cfg(test)] @ 203
crates/gitless-sync/src/shared/github.rs            — #[cfg(test)] @ 225
crates/gitless-sync/src/shared/gh.rs                — #[cfg(test)] @ 75/81/94/106 (MockGhClient gating + 107@mod tests)
crates/gitless-sync/src/shared/config.rs            — #[cfg(test)] @ 36
crates/gitless-sync/src/shared/ignore.rs            — #[cfg(test)] @ 68
```

### 다음 단계

- **task S**: 위 2건 production `expect` 진짜 fix + `#[allow]` 제거. baseline 위반 0건 도달.
- **task T**: workspace lint warn → deny 전환 (`unwrap_used = "deny"` etc.). `cargo clippy -D warnings` 통과.

## (placeholder) 분할 metrics — task M에서 박음

- file 수, total LOC, max LOC, fan-out (file별 import count), cycle count: F-I 분할 직전/직후 비교를 위해 task M이 본 파일에 누적.
