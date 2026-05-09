# Phase 5 Vault Dogfooding (After) + Acceptance Verification

> Snapshot at task T commit time (2026-05-09). Phase 5 진행 종료 직후 박은 dogfood scan. Phase 5 task A baseline (commit `68fb0f0`, 92 files / 0 drift / 0 failed) 위에 task B~S까지 23 task 박힌 v0.2 코드 박힘 후 재실행.
>
> **Vault unavailable note** (baseline mirror): 이 머신(`C:\Users\dasgut`)에서 vault path(`C:\Users\admin\iCloudDrive\iCloud~md~obsidian`) 접근 불가. dogfood target은 `KneShell/gitless-sync` 자체 repo (92 files baseline → 117 files 현재) 한정. 함정 surface는 ~0건 예상되며 실제로 그렇게 측정됨 (아래 § Drift Source Analysis). pitfall handling 검증은 본 dogfood가 아닌 cross-reference integration tests (R/R2/S/P/P1/Q) chain이 담당.

## Measurement Setup

- 빌드: `cargo build --release --quiet` 후 `target/release/gitless-sync.exe scan --repo KneShell/gitless-sync --branch main --pretty > tmp/phase5-scan-after.json 2>tmp/phase5-scan-after.err`
- backend: `graphql` (default per ADR 0006)
- local root: `D:\00.Projects\02.Personal\05.gitless-sync` (이 repo 자체 — self-dogfooding, baseline 그대로)
- exit code: 0
- scanned_at: 2026-05-09T06:32:02.590821600Z
- schema_version: **1.1** (`mode` / `failed_reason` / `lfs_pointer` 필드 박힘 — task O 시점 박음, baseline은 1.0)

**Baseline 차이 — 박은 명령**: baseline은 `cargo run --release --quiet --` wrapper 박음, 본 task는 release exe 직접 박음. cargo run wrapper는 `cargo build` 캐시 박혀있으면 동일 binary 박혀 functional 차이 0 — Setup 정합 (advisor flag).

## Scan Summary

| Status | After (T) | Baseline (A) | Delta |
|---|---:|---:|---:|
| identical | 81 (69.2%) | 90 (97.8%) | -9 |
| local_only_changed | **36** (30.8%) | 2 (2.2%) | +34 |
| remote_only_changed | 0 (0.0%) | 0 (0.0%) | 0 |
| **drift** | **0** (0.0%) | 0 (0.0%) | 0 |
| **failed** | **0** (0.0%) | 0 (0.0%) | 0 |
| **Total** | **117** | 92 | **+25** |

**핵심 acceptance metric** (T scope):
- false drift 0건 ✓ — `0 drift / 0 failed`.
- false failed 0건 ✓ — same.
- 의도된 detect-only drift (submodule/symlink/encoding/lfs_pointer/long_path) 부재 — KneShell/gitless-sync에 함정 surface 0건 (baseline § Drift Source 그대로). pitfall handling 정확성 검증은 본 dogfood scope 외, integration tests chain이 담당 (§ Pitfall Verification Chain).

## Drift Source Analysis

### 0 drift / 0 failed — T acceptance 통과

`drift` / `failed` 0건은 baseline 그대로. submodule (`160000`) / symlink (`120000`) / LFS pointer (`filter=lfs`) / Windows long path / 비-UTF-8 인코딩 / `.gitattributes` 박힌 화이트리스트 외 attribute 모두 KneShell/gitless-sync에 surface 안 됨 — Rust 프로젝트 + ASCII source + `.gitattributes` 부재 + LFS 미사용 + submodule/symlink 미사용 + Windows path 한도 안 (이 repo 자체).

### 36 local_only_changed — provenance 분류

baseline은 2건 (`tmp/phase5-scan-baseline.{json,err}` race noise) 박힘. T 시점 36건 = 2 race noise + 17 new (Phase 5 phase 박힌 file) + 17 modified (Phase 5 phase 박은 변경). KneShell/gitless-sync remote는 Phase 5 진행 중 mid-phase snapshot이라 본 repo 변경 cascade 박지 않음 — 자연 누적.

#### Race noise (2건, baseline 동일 패턴)

| path | local_sha | 분류 |
|---|---|---|
| `tmp/phase5-scan-after.err` | `e69de29b...` (empty) | scan stderr redirect target — shell이 0-byte truncate 후 walker hash |
| `tmp/phase5-scan-after.json` | `e69de29b...` (empty) | scan stdout redirect target — 동일 race |

baseline `tmp/phase5-scan-baseline.{json,err}` 박힌 패턴 정합. shell redirect race는 `tmp/` 외 디렉토리로 redirect 박거나 pipe 박으면 자동 제거. 도메인 함정 아님.

#### NEW (17건, Phase 5 task 박힘 후 KneShell remote에 부재)

baseline commit `68fb0f0`에 부재 + 현재 박힘:

- **Source code** (4): `crates/gitless-sync/src/shared/decode.rs` (E·F task), `crates/gitless-sync/src/shared/gitattributes.rs` (K1 task), `crates/gitless-sync/src/shared/gitattributes_tests.rs` + `crates/gitless-sync/src/shared/gitattributes_classify_tests.rs` (K1.5 task — Z task에서 module 폴더로 정리 예정).
- **Pipeline tests** (4): `pipeline_tests.rs`, `pipeline_tests_lfs.rs`, `pipeline_tests_long_path.rs`, `pipeline_tests_modes.rs` (G/H/J/G1/R1 task).
- **Integration tests** (4): `tests/scan_gitattributes.rs` (S task), `tests/scan_modes.rs` (R task), `tests/scan_nfd_real_file.rs` (P1 task), `tests/scan_robustness.rs` (R2 task).
- **Benchmarks** (2): `benches/gitattributes_match.rs` (X task), `benches/scan_scale.rs` (R3 task).
- **Research artifacts** (3): `docs/research/encoding-library-eval.md` (E task), `docs/research/phase5-gitattributes-bench.md` (X task), `docs/research/phase5-scan-scale-bench.md` (R3 task).

#### MODIFIED (17건, baseline에 박힘 + Phase 5 phase 박음 변경)

- **Manifests** (2): `Cargo.lock`, `crates/gitless-sync/Cargo.toml` (encoding_rs / criterion / unicode-normalization 박힘 + bench entry).
- **Source code** (8): `commands/scan/{compare.rs, hash_local.rs, mod.rs, output.rs, pipeline.rs, walker.rs}`, `shared/{mod.rs, normalize.rs}` — Phase 5 task A~S 박힌 cascade.
- **Test scaffolding** (1): `tests/common/mod.rs`.
- **Plan** (1): `docs/ralph/implementation-plan.md` (Phase 5 task status 갱신).
- **Specs** (5): `spec-classification.md`, `spec-config.md`, `spec-error-contracts.md`, `spec-hash-and-normalize.md`, `spec-output-schema.md` (M·L1·N·L·O task audit).

→ **결론**: 36건 모두 self-dogfood mid-phase snapshot이라 자연 박힘. KneShell/gitless-sync remote는 Phase 5 cascade 박지 못한 시점이라 본 repo가 항상 앞서 박혀있음. **새 false drift 0건 + 새 false failed 0건이 T acceptance 핵심**, local_only_changed는 self-dogfood 본질적 noise.

## Pitfall Verification Chain

KneShell/gitless-sync에 함정 surface 0건이라 본 dogfood가 pitfall handling 정확성을 박지 못함 (baseline § Limitations 그대로). Phase 5 함정별 정확 처리는 cross-reference integration tests chain이 담당:

| 함정 | 검증 fixture | 박힘 task |
|---|---|---|
| NFD vs NFC (path) | `tests/scan_nfd_real_file.rs` (3 tests, NTFS 실파일 양방향) + `walker.rs::tests` synthetic | P1 + P |
| 비-UTF-8 인코딩 (raw bytes hash) | `decode.rs::tests` × 3 인코딩 (EUC-KR / Shift_JIS / Latin-1) | F + Q |
| submodule / symlink / 실행 권한 mode bit | `tests/scan_modes.rs` (4 tests, all-modes Trees body) | R |
| `.gitattributes` 화이트리스트 routing | `tests/scan_gitattributes.rs` (7 tests, 5 branches + multi-level + envelope) | S |
| LFS pointer detect-only | `pipeline_tests_lfs.rs` + `gitattributes::is_lfs` | G1 |
| Windows long path / 예약 파일명 | `pipeline_tests_long_path.rs` | R1 |
| `.gitgitattributes` parser robustness (malformed / utf8 mid-byte / dangling/circular symlink) | `tests/scan_robustness.rs` (4 tests) | R2 |
| 빈 파일 (실파일 + git constant) | `hash::tests::empty_blob_matches_git` + integration | I |

374~383 tests pass + tarpaulin 90.7% baseline (S task 시점)이 본 chain 박힌 결과. Production vault dogfood 검증은 vault 접근 박힌 환경에서 별도 task 박음 (Phase 5+).

## Schema 1.0 → 1.1 — 정확화 분류

baseline schema_version `1.0`, T 시점 schema_version `1.1` — `mode` / `failed_reason` / `lfs_pointer` 필드 추가 (task O 박음). spec-domain-pitfalls.md § "v0.1 vs v0.2 회귀 정의"의 정확화 화이트리스트에 박힘:

> **정확화 (의도된 변화 — 회귀 아님)**:
> - v0.1에서 LFS pointer를 raw text로 박아 mismatch한 entry가 v0.2에서 `failed_reason: "lfs_pointer"` 명시 박힘.

v1.0 backward-compat은 `output.rs::tests` 박힌 5 lock test (envelope / identical entry / failed-with-lfs entry / mode 박음 + failed_reason/lfs_pointer omit / failed_reason + lfs_pointer placeholder 박음, task O)에서 박음. 호출자가 v1.0 parser 박혀있으면 새 필드 ignore 박음 + status enum 5값 그대로 → 회귀 0건.

## v0.1 vs v0.2 회귀 정의 (W task hand-off)

본 task T는 high-level metric (drift / failed 0건) 검증이 scope. **per-file regression diff** (정확화 vs 회귀 자동 분류)는 의존성 graph상 W task scope (`docs/ralph/implementation-plan.md` § 의존 순서: `T → W`).

W task에서 박을 입력:
- v0.1 baseline JSON (Phase 5 진입 직전 박힌, 본 task에서는 미보존 — § Limitations).
- T 시점 JSON: `tmp/phase5-scan-after.json` (39 KB, 117 files).
- 정확화 화이트리스트: spec-domain-pitfalls.md § v0.1 vs v0.2 회귀 정의.
- 화이트리스트 외 status 변화는 회귀로 박음 (자동 fail).

## Limitations

1. **Baseline JSON 미보존**: A task 시점에 박힌 `tmp/phase5-scan-baseline.json`은 `tmp/` race noise 정리 시점에 박힘 X. baseline doc은 summary count 박힘 + path 박음 결과만 박혔고, **per-file regression diff는 박지 못함** (W task scope, baseline 박을 commit `68fb0f0` 시점 박은 v0.1 코드 재실행으로 별도 박음).
2. **Vault 접근 불가**: 머신 한정 (`C:\Users\dasgut`). vault path(`C:\Users\admin\...`) 박힌 환경에서 별도 vault dogfood task 박음 가능 (Phase 5+).
3. **함정 surface 0건**: KneShell/gitless-sync에는 NFD path / `.gitattributes` / LFS / submodule / symlink / 비-UTF-8 / Windows long path 모두 결여 — dogfood가 pitfall handling 정확성을 박지 못함. § Pitfall Verification Chain의 cross-reference integration tests로 박음.
4. **Schema 1.1 lock test는 unit-level**: O task 박힌 v1.0 backward-compat lock test 5건은 `output.rs::tests` 박힘 (synthetic envelope). production v1.0 parser 박힌 호출자 정합은 W task 자동 비교에서 박음.

## Acceptance

- [x] Phase 5 후 vault scan 재실행 — KneShell/gitless-sync 117 files baseline (vault 부재, self-dogfooding)
- [x] **false drift 0건** — `0 drift / 0 failed` ✓
- [x] **false failed 0건** — same ✓
- [x] 의도된 detect-only drift = submodule/symlink/encoding/lfs_pointer/long_path 박힘 surface 부재 — KneShell/gitless-sync에 함정 surface 0건 (검증 chain은 § Pitfall Verification Chain integration tests)
- [x] before/after 비교 박음 — § Scan Summary delta table + § Drift Source Analysis 36 path provenance 박힘
- [x] schema_version 1.0 → 1.1 정확화 분류 박음 (§ Schema 1.0 → 1.1)
- [x] per-file 정확화 vs 회귀 자동 분류는 W task hand-off (의존성 graph `T → W`)
