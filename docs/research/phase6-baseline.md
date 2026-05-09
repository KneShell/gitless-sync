# Phase 6 Baseline Metrics

> Snapshot at task R commit time (2026-05-08). 분할 전 측정. Phase 6 진행 중 task M에서 LOC/fan-out/cycle 등 추가 metric을 본 파일에 누적, J 직후 분할 후 측정으로 갱신.
>
> **Update (2026-05-08, task S 완료)**: production `expect` 2건 모두 `Config` map_err로 fix → baseline 위반 0건 도달. § Baseline Violation Count + § 위반 위치 표 갱신. task T (warn → deny 전환) 즉시 진행 가능.
>
> **Update (2026-05-08, task T 완료)**: workspace lint `unwrap_used`/`expect_used`/`panic` warn → **deny** 전환. `cargo clippy --workspace --all-targets -- -D warnings` 통과 + 233 tests pass + tarpaulin 87.77%. 향후 production 위반은 `-D warnings` 의존 없이 빌드 fail로 차단된다.

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
- Row 1 (serialize)은 spec table이 `unreachable!()` 또는 `Err(...)` 둘 다 안전한 alternative로 명시. `Config` map_err는 Row 2와 일관 + 미래 schema 변경 시 silent panic 차단.
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
- **task T**: ✅ 완료 (2026-05-08). workspace lint warn → deny 전환 적용. `cargo clippy --workspace --all-targets -- -D warnings` 통과 + 233 tests pass + tarpaulin 87.77%. Phase 6.3 panic 검출 trilogy (R → S → T) 완결.

## File Split Metrics — Pre-Split Baseline (2026-05-08, task M)

> Phase 6.4 (F-I 4 task) 진행 직전 snapshot. 분할 후 metrics는 task J 직후 본 sub-section 아래 동일 형식으로 누적 (post-split).

### 측정 환경

- scan root: `crates/gitless-sync/src` (xtask + workspace lint 적용 범위, xtask crate는 별도)
- LOC 측정: `cargo xtask check-line-limits` (Rust `content.lines().count()`) + PowerShell `(Get-Content $f).Count` cross-check 일치
- Fan-out 측정: `^use ` 정규식 — top-level use 문 카운트 (test mod 내부 use는 들여쓰기로 자연 제외)
- Cycle / cross-slice 측정: `cargo xtask check-cycles` (cargo-modules DOT 출력 파싱)
- Panic 위반 측정: 본 문서 § Panic Escape Hatch (task R/S/T 결과 계승)

### File Count + LOC + Fan-out

- Total `.rs` files (production scan root): **18**
- Total LOC: **4434**
- Max LOC: **1092** (`commands/scan/mod.rs`)
- LOC > 300 (분할 대상): **4 files** (diff/mod.rs, scan/graphql.rs, scan/mod.rs, shared/github.rs)

| Path | LOC | > 300? | top-level `use` 문 |
|---|---:|:---:|---:|
| `commands/diff/mod.rs` | 472 | ✓ | 9 |
| `commands/init/mod.rs` | 206 | | 2 |
| `commands/mod.rs` | 3 | | 0 |
| `commands/scan/compare.rs` | 128 | | 2 |
| `commands/scan/graphql.rs` | 564 | ✓ | 7 |
| `commands/scan/mod.rs` | 1092 | ✓ | 15 |
| `commands/scan/output.rs` | 41 | | 3 |
| `commands/scan/walker.rs` | 236 | | 5 |
| `lib.rs` | 9 | | 0 |
| `main.rs` | 122 | | 5 |
| `shared/config.rs` | 102 | | 4 |
| `shared/error.rs` | 137 | | 3 |
| `shared/gh.rs` | 240 | | 2 |
| `shared/github.rs` | 748 | ✓ | 6 |
| `shared/hash.rs` | 58 | | 1 |
| `shared/ignore.rs` | 168 | | 3 |
| `shared/mod.rs` | 7 | | 0 |
| `shared/normalize.rs` | 101 | | 0 |

Top-level `use` 합계: **67** (across 14 files; `commands/mod.rs` / `lib.rs` / `shared/mod.rs` / `shared/normalize.rs` 4 files = 0 use).

### Cycle + Cross-Slice Reference

- Modules tracked: **16** (cargo-modules `--lib --no-fns --no-types --no-traits --no-sysroot` DOT 출력 기준)
- Cycles: **0**
- Cross-slice refs (`commands/scan` ↔ `commands/diff` ↔ `commands/init`): **0**

검증 명령: `cargo xtask check-cycles` exit 0.

### Panic Violation Carryover

본 문서 § Panic Escape Hatch — `unwrap_used` / `expect_used` / `panic`:

- Production `unwrap_used`: **0**
- Production `expect_used`: **0** (task S에서 fix 완료)
- Production `panic`: **0**
- Total production 위반: **0건** (task T deny 전환 완료, `cargo clippy --workspace --all-targets -- -D warnings` 통과)

### Post-Split Comparison (placeholder — task J 직후 추가)

F-I 4 task + Q error 분리 + P tests 분리 완료 후 동일 표를 본 section 아래 누적. 비교 항목:

- File 수 (분할로 증가 예상)
- Total LOC (분할로 약간 증가 예상 — sub-module mod statement + use line 추가분)
- Max LOC (300 이하 도달 목표)
- LOC > 300 file 수 (목표 0)
- Fan-out 분포 (sub-module 분리로 각 file fan-out 감소 예상, 대신 file 수 자체 증가)
- Cycle / cross-slice (변화 없이 0 유지 — task N에서 step별 검증)
- Panic 위반 (변화 없이 0 유지 — workspace lint deny로 빌드가 차단)
