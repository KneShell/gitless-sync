# Implementation Plan

## Status
- Last updated: 2026-05-08 (task T — warn → deny 전환 완료)
- Total tasks: 20
- Completed: 8 / 20

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 `docs/specs/spec-architecture.md`와 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).

## Tasks (Phase 6 — Code Quality Strengthening)

### Phase 6.1 — 선행 cascade fix

- [x] **A. github.rs → shared 이전**
  - acceptance: `commands/scan/github.rs` 본체를 `shared/github*.rs` (또는 `shared/github/mod.rs` 폴더)로 이전. `commands/diff/mod.rs`의 `commands::scan::github` import → `shared::github::...`로 갱신. cross-slice ref 위반 1건 0 도달.
  - 검증: `cargo build` + 188 tests pass.
  - spec: `docs/specs/spec-architecture.md` § Cross-slice 직접 ref 금지.

### Phase 6.2 — xtask + 외부 도구 박음

- [x] **B. xtask crate workspace 멤버 박음**
  - acceptance: `xtask/` 폴더 + `xtask/Cargo.toml` + `xtask/src/main.rs` 박음. workspace `members`에 추가. `cargo xtask` alias `.cargo/config.toml`에 박음. `cargo xtask --help` 통과.
  - spec: 없음 (boilerplate).

- [x] **C. cargo-modules + cargo-public-api + cargo-machete 도구 검증**
  - acceptance: 각 도구 `cargo install` 후 1회 dry-run 출력 확인. `docs/ralph/project-ops.md`에 설치 가이드 + 명령어 박음. CI 워크플로 사용한다면 `.github/workflows/` 갱신 (옵션).
  - spec: `docs/specs/spec-architecture.md` § 외부 도구.
  - 검증 결과 (2026-05-08): cargo-modules 0.26.0 + cargo-public-api 0.51.0 + cargo-machete 0.9.2 설치 + dry-run 통과. cargo-public-api는 nightly toolchain 필요 (별도 설치, 본 프로젝트 빌드는 stable 1.95.0 그대로). cargo-machete 현 baseline `anyhow` 1건 unused (task S 이후 자연 해결 예정). `.github/workflows/` 부재 — task O 시점 박힘.

- [x] **D. xtask check-line-limits 박음 (LOC 게이트)**
  - acceptance: `cargo xtask check-line-limits`가 `crates/gitless-sync/src/**/*.rs` LOC 측정. 300줄 초과 file 경고 출력 (warn 단계, 빌드 깨뜨리지 않음). doc comment heavy 면제 룰 박음 (`///` 비중 ≥ X% 시 면제). tests 포함 카운트.
  - spec: `docs/specs/spec-architecture.md` § LOC 임계.
  - 검증 결과 (2026-05-08): `xtask/src/check_line_limits.rs` 박음 + `main.rs`에 dispatch wired. doc-heavy 임계 50% (1/2 정수 비교). `cargo xtask check-line-limits` hand-test 통과 — 4 files exceed 300 LOC (diff/mod.rs 472, scan/graphql.rs 564, scan/mod.rs 1094, shared/github.rs 748). exit 0 (warn stage). 14 unit tests for check_line_limits + 5 main dispatch tests = 191 tests pass (167 unit + 21 integration + 24 xtask). tarpaulin 89.97%.

- [x] **E. xtask check-cycles 박음 (cycle 검출)**
  - acceptance: `cargo xtask check-cycles`가 `cargo modules dependencies --lib --no-fns --no-types --no-traits --no-sysroot` DOT 출력을 파싱해 module-level uses 그래프에서 cycle 1건 이상이면 exit 1. cycle 0건 시 OK 출력. cross-slice ref 검증도 동시 (slice 간 import 금지). cargo-modules `--acyclic`은 type-method edge(`enum ↔ method`) false positive로 직접 사용 안 함.
  - spec: `docs/specs/spec-architecture.md` § Slice 안 acyclic + § Cross-slice 직접 ref 금지.
  - 검증 결과 (2026-05-08): `xtask/src/check_cycles.rs` 박음 (DOT parser + DFS white/gray/black cycle detect + slice prefix cross-slice check). `cargo xtask check-cycles` hand-test 통과 — 16 modules / 0 cycles / 0 cross-slice refs. xtask 19 unit tests + 5 main dispatch tests = 45 tests pass (167 unit + 21 integration + 45 xtask). tarpaulin 87.74% (≥80% gate). spec-architecture.md + project-ops.md cargo-modules 표기 갱신 (cascade closure).

### Phase 6.3 — panic 검출 lint 단계적 도입

- [x] **R. workspace lint warn 박음 + tests 면제 + baseline 측정**
  - acceptance: workspace `Cargo.toml`에 박힘 (`unwrap_used`/`expect_used`/`panic` = warn, 2026-05-08). lib.rs / main.rs 상단 `#![cfg_attr(test, allow(...))]` + tests/integration.rs 상단 `#![allow(...)]` 박힘 (2026-05-08, hard gate fix). `cargo clippy --workspace --all-targets -- -D warnings` 실행 후 production 코드 위반 카운트 측정 → `docs/research/phase6-baseline.md`에 박음. **현 baseline**: production expect 2건 임시 `#[allow(clippy::expect_used)]` 박혀있음 (`commands/scan/mod.rs:70`, `mod.rs:415`) — task S에서 진짜 fix + allow 제거.
  - spec: `docs/specs/spec-architecture.md` § Panic escape hatch 차단.
  - 검증 결과 (2026-05-08): clippy `-D warnings` 통과 + 233 tests pass (167 unit + 21 integration + 45 xtask) + tarpaulin 87.74% (≥80% gate). `docs/research/phase6-baseline.md` 박음 — production expect 2건 (`commands/scan/mod.rs:70` ScanReport serialize + `mod.rs:419` rayon pool build, 둘 다 `#[allow(clippy::expect_used)]` 임시 박혀 silent suppress 상태) enumeration. test 면제는 lib.rs:6 + main.rs:1 (cfg_attr) + integration.rs:1 (file-level) 3곳. 누락된 production violation 0건 검증.

- [x] **S. unwrap/expect/panic 위반 fix**
  - acceptance: production 코드의 `.unwrap()` / `.expect()` / `panic!()` 모두 `?` + `anyhow::Context` 또는 `Result` 변환으로 대체. **임시 박힌 `#[allow(clippy::expect_used)]` 2건 (`commands/scan/mod.rs:70`, `mod.rs:415`) 제거 + 진짜 fix.** baseline 위반 0건 도달.
  - 검증: `cargo clippy --workspace --all-targets -- -D warnings` 시 production 코드에서 0 warning + `#[allow(clippy::*_used)]` 0건.
  - spec: `docs/specs/spec-architecture.md` § Panic escape hatch 차단.
  - 검증 결과 (2026-05-08): production expect 2건 모두 `.map_err(|e| GitlessError::Config(format!(...)))?` 패턴으로 fix. (1) `commands/scan/mod.rs:70` ScanReport serialize 실패 → `Config("ScanReport JSON serialization failed: ...")`. (2) `commands/scan/mod.rs:415` rayon thread pool build 실패 → `Config("rayon thread pool build failed: ...")` (advisor 권고 + spec § Config "환경 문제" 정합). `#[allow(clippy::expect_used)]` 2건 모두 제거. `run_with_client` doc comment의 `# Panics` 블록 제거 + `# Errors`로 흡수. clippy `-D warnings` 통과 (production 0 warning) + 233 tests pass (167 unit + 21 integration + 45 xtask) + tarpaulin 87.77% (≥80% gate, +0.03%).

- [x] **T. unwrap/expect/panic deny 전환**
  - acceptance: workspace lint `unwrap_used`/`expect_used`/`panic` warn → deny. `cargo clippy -D warnings` 통과. 188+ tests pass.
  - spec: `docs/specs/spec-architecture.md` § Enforcement 단계.
  - 검증 결과 (2026-05-08): workspace `Cargo.toml` `[workspace.lints.clippy]`의 `unwrap_used`/`expect_used`/`panic` 모두 warn → deny 박음. `cargo fmt --check` 통과 + `cargo clippy --workspace --all-targets -- -D warnings` 통과 + 233 tests pass (167 unit + 21 integration + 45 xtask) + tarpaulin 87.77% (≥80% gate). baseline 위반 0건 (task S에서 production expect 2건 fix 완료) → deny 전환에 추가 fix 0건. xtask는 `[lints] workspace = true` 미박힘(task L 영역) — workspace deny는 `gitless-sync` crate에만 영향. Phase 6.3 panic 검출 trilogy (R warn 박음 → S 위반 fix → T deny 박음) 완결, 향후 신규 production 위반은 빌드 fail로 즉시 차단.

### Phase 6.4 — File 분할 (LOC + Layer 결합)

- [ ] **F. scan/mod.rs 1093줄 분할**
  - acceptance: orchestrator(`mod.rs`)에 진입점만 남기고 logic을 domain/IO sub-module로 분리 (예: `scan/orchestrator.rs` 또는 mod.rs 그대로 + helper file 추가). 분할 직후 LOC 게이트 + cycle 게이트 + cross-slice 게이트 동시 통과 (task N — 분할이 새 cycle 만들 가능성 차단). 188 tests pass.
  - spec: `docs/specs/spec-architecture.md` § Slice-internal directional discipline + § LOC 임계.

- [ ] **G. shared/github*.rs 748줄 분할**
  - acceptance: A에서 이전한 후, `shared/github.rs` (또는 `shared/github/mod.rs` 폴더)을 sub-module 분리 (rest/graphql/common 또는 trees/blobs/commits 도메인별). LOC + cycle 게이트 통과. 188 tests pass.
  - spec: 동일.

- [ ] **H. scan/graphql.rs 565줄 분할**
  - acceptance: GraphQL alias batching logic + response parsing 분리 (예: `graphql/batch.rs` + `graphql/parse.rs` + `graphql/mod.rs`). LOC + cycle 게이트 통과. 188 tests pass.
  - spec: 동일.

- [ ] **I. diff/mod.rs 472줄 분할**
  - acceptance: orchestrator + domain + IO 분리 (예: `diff/compare.rs` domain + `diff/output.rs` 등). LOC + cycle 게이트 통과. 188 tests pass.
  - spec: 동일.

### Phase 6.5 — 구조적 분리 task

- [ ] **Q. error 모듈 도메인 분리**
  - acceptance: `shared/error.rs` 137줄 단일 enum → `shared/error/mod.rs` (최상위 `GitlessError` + exit code 매핑) + `shared/error/network.rs` / `error/config.rs` / `error/filesystem.rs` 등 도메인별 sub-module. 호출자 API 호환 유지. **`docs/specs/spec-error-contracts.md` § GitlessError variants + exit code mapping 표 갱신** (도메인 sub-module 박힌 후, advisor §3 갭 fix). 188 tests pass.
  - spec: `docs/specs/spec-architecture.md` § LOC 임계 § 구조적 분리 + `docs/specs/spec-error-contracts.md`.

- [ ] **P. integration tests 도메인별 분리**
  - acceptance: `crates/gitless-sync/tests/integration.rs` 1 file → 도메인별 분리 (`tests/scan_dogfooding.rs`, `tests/diff_workflow.rs`, `tests/init_redirect.rs`, `tests/scan_backend_parity.rs` 등 자연 묶음). 공통 setup → `tests/common/mod.rs` (Cargo가 separate test로 취급 안 함). 21 integration tests 모두 pass.
  - spec: Rust 공식 ch11-03 + `docs/specs/spec-architecture.md` § 구조적 분리.

### Phase 6.6 — Dogfooding + 회귀 가드

- [ ] **L. xtask self-dogfooding**
  - acceptance: `xtask/` crate에도 workspace lints 적용 (300줄 LOC + clippy 60/15/5 + panic deny). xtask 자체 코드가 게이트 통과. xtask Cargo.toml에 `[lints] workspace = true` 박음.
  - spec: `docs/specs/spec-architecture.md` § Enforcement.

- [ ] **M. 분할 전/후 baseline metric 박제**
  - acceptance: `docs/research/phase6-baseline.md` 박음. 분할 전 (현재) + 분할 후 (F-I 완료) 측정값: file 수, total LOC, max LOC, fan-out (file별 import count), cycle count, panic 위반 카운트.
  - spec: 없음 (research artifact, clean-context §4 누락 추가).

- [ ] **N. F-I 분할 후 layer 게이트 pass 검증 (각 task 안에 step 명시)**
  - acceptance: F~I 각 task 완료 시 `cargo xtask check-line-limits` + `cargo xtask check-cycles` 통과 검증을 acceptance에 명시. 각 task 완료 시 cycle 0건 + cross-slice ref 0건 + LOC 위반 0건 (면제 카테고리 외).
  - spec: `docs/specs/spec-architecture.md` § Slice 안 acyclic + § Cross-slice 직접 ref 금지.

- [ ] **O. pre-commit hook 또는 CI gate 박음**
  - acceptance: `.github/workflows/ci.yml` (또는 `.git/hooks/pre-commit`)에 `cargo xtask check-line-limits` + `cargo xtask check-cycles` + `cargo machete` + `cargo public-api diff` 추가. 게이트가 실제 PR 차단하는지 회귀 가드. Windows runner 검증 필수.
  - spec: 없음 (CI 설정).

### Phase 6.7 — Step 2/3 deny 전환 + 부속 리서치

- [ ] **J. baseline 위반 0건 도달 후 LOC + cycle 게이트 deny 전환**
  - acceptance: F-I 분할 + Q error 분리 + P tests 분리 후 LOC 위반 0건 (300 면제 카테고리 외) + cycle 위반 0건. xtask check-* 명령이 위반 시 exit 1로 박음 (deny). CI 게이트 deny 전환.
  - 검증: `cargo xtask check-line-limits` exit 0 + `cargo xtask check-cycles` exit 0.
  - spec: `docs/specs/spec-architecture.md` § Enforcement.

- [ ] **K. 외부 Rust 프로젝트 LOC 통계 측정 (부속 리서치)**
  - acceptance: ripgrep / cargo / tokio 등 mid-size Rust 프로젝트 LOC 분포 측정 (`tokei` 또는 `scc`). `docs/research/rust-loc-stats.md` 박음. 우리 300 임계 사후 검증 (외부 통계와 비교).
  - spec: 없음 (research artifact, 흥미 위주).

## 의존 순서

```
A → B → {C, D, E}
A → {F, G, H, I}  (G는 A 직후 자연 cascade)
B → L
{D, E} → {F, G, H, I}  (게이트 박힌 후 분할 task 검증)
F-I → N (각 task 안에 step)
F-I → J (deny 전환)
Q + P → J (구조 분리 후 deny)
{D, E} → O (CI gate)
R → S → T (단계적 panic 도입, 독립 진행 가능)
M (baseline metric) — A 직전 + J 직후 (전/후)
K (외부 통계) — 독립, 어디서든 가능
```

ralph build mode 진행 권장 순서:
1. A (cross-slice fix)
2. B → C → D → E (xtask + 게이트 박음)
3. M baseline 박음 (전 측정)
4. R → S → T (panic 검출 단계적 도입, F-I와 병렬 가능)
5. F → G → H → I (파일 분할, N step 포함)
6. Q + P (구조 분리)
7. L (xtask self-dogfooding)
8. O (CI gate 박음)
9. J (deny 전환)
10. M baseline 박음 (후 측정)
11. K (외부 통계, 부속 리서치)
