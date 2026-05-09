# Code Smell Audit — Task Z (Phase 5.12)

> 2026-05-09. 6 병렬 sub-agent (Test / Module / Error / Public-API / Rust-idiom / Panic-escape) audit 결과 정리.
> Z task acceptance: **확정 fix 1건** (`shared/gitattributes.rs` module 폴더 분할 + sibling test 제거)만 본 task에서 처리. 나머지 finding은 후행 task 입력 (`docs/ralph/implementation-plan.md` Phase 6+에 사람이 task 추가, ralph plan 모드 스킵 정책).

## 1. Test 구조 (Explore sub-agent)

### 1-1. Sibling test files (spec § 금지 패턴 위반)

5 신규 위반 + 2 known (Z task scope):

| File | 상태 |
|---|---|
| `crates/gitless-sync/src/commands/scan/pipeline_tests.rs` | 위반 (신규) |
| `crates/gitless-sync/src/commands/scan/pipeline_tests_lfs.rs` | 위반 (신규) |
| `crates/gitless-sync/src/commands/scan/pipeline_tests_long_path.rs` | 위반 (신규) |
| `crates/gitless-sync/src/commands/scan/pipeline_tests_modes.rs` | 위반 (신규) |
| `crates/gitless-sync/src/shared/github/trees_tests.rs` | 위반 (신규) |
| `crates/gitless-sync/src/shared/gitattributes_tests.rs` | **[Z task fix]** |
| `crates/gitless-sync/src/shared/gitattributes_classify_tests.rs` | **[Z task fix]** |

**fix 패턴**: 각 production source를 module 폴더로 분할 (예: `pipeline.rs` → `pipeline/{mod.rs, ...}` + 각 sub-module에 `#[cfg(test)] mod tests`). spec § 금지 패턴 — sibling test file은 LOC 회피 수단으로 사용 금지, production 자체 분할로 해결.

### 1-2. mod loader sites (`#[cfg(test)] #[path = "..._tests.rs"] mod tests;`)

7 site 모두 `#[cfg(test)]` guard 정확. 1-1 fix cascade로 자연 정리됨.

### 1-3. `#[cfg(test)]` 가드 일관성

inconsistency 0건. integration tests (`tests/*.rs`)에 redundant guard 0건 (Rust 관용 정합).

### 1-4. Test helper duplication

| Helper | 중복 site |
|---|---|
| `ok_resp(body: &[u8]) -> GhResponse` | `commands/scan/test_helpers.rs:45` + `commands/scan/graphql/test_helpers.rs:9` + `commands/diff/test_helpers.rs:25` + `shared/github/trees_tests.rs:8` |
| `err_resp(stderr: &str) -> GhResponse` | `commands/scan/test_helpers.rs:53` + `commands/scan/graphql/test_helpers.rs:17` + `commands/diff/test_helpers.rs:33` + `shared/github/trees_tests.rs:16` |
| `tree_args(repo, branch) -> Vec<String>` | `commands/scan/test_helpers.rs:61` + `commands/diff/test_helpers.rs:41` + `shared/github/trees_tests.rs:24` |

**fix 후보**: `tests/common/mod.rs` 패턴 mirror — `crates/gitless-sync/src/shared/test_helpers.rs`에 `#[cfg(test)] pub(crate) mod` 추가하여 dedup. 후행 task scope.

## 2. Module 구조 (Explore sub-agent)

### 2-1. mod.rs thin orchestrator (spec § Module 폴더 단위 정책)

| `mod.rs` | LOC | 평가 |
|---|---|---|
| `commands/scan/mod.rs` | 256 | orchestrator (entry point + thin wrapper). 정합. |
| `commands/init/mod.rs` | 190 | borderline — `run`이 23 LOC business logic + tests 130 LOC. spec § thin orchestrator vs single-purpose slice 사이에서 false positive 가능 (init은 단일 책임). 사람 결정 필요. |
| `commands/diff/mod.rs` | 54 | clean. |

### 2-2. Near-cap files (≥ 250 LOC, 300 cap 근접)

| File | LOC | 평가 |
|---|---|---|
| `shared/gitattributes.rs` | 296 (실제 measured 276 + sibling 추가 미포함) | **[Z task fix]** module 폴더 분할 |
| `commands/scan/pipeline.rs` | 278 | domain — 후행 phase 진입 시 분할 후보 |
| `shared/normalize.rs` | 259 | domain — 자연 분할 어려움 |
| `commands/scan/walker.rs` | 248 | IO — case_collision/long_path 추가로 인한 증가 |
| `shared/github/commits.rs` | 252 | IO — Trees/Commits/Blobs 동급 분리됨 |
| `commands/scan/mod.rs` | 256 | orchestrator |
| `commands/scan/compare.rs` | 222 (test 83 포함) | domain |
| `shared/decode.rs` | 219 | domain |
| `shared/gh.rs` | 212 | IO — gh subprocess wrapper |

### 2-3. 폴더 split 패턴

분할 정합 사례:
- `shared/github/{mod, trees, commits, blobs, error_map}` — re-export hub 9 LOC
- `shared/error/{mod, core, network, ...}` — re-export hub 12 LOC
- `commands/scan/graphql/{mod, batch, parse, query, test_helpers}` — re-export hub 35 LOC

**collapse 후보**: 단일 sub-module만 가진 폴더는 발견 안 됨. 모두 정합.

### 2-4. Visibility leaks → § 4 (Public API audit)에 통합.

### 2-5. Cross-slice ref + slice-internal directional violation

발견 0건 — `cargo xtask check-cycles` deny gate가 baseline 0 유지.

## 3. Error handling (general-purpose sub-agent)

### 3-1. Hand-rolled error formatting (stderr drift)

| Site | 평가 |
|---|---|
| `shared/github/trees.rs:93-98` | non-blob entry warning이 plain-text `eprintln!`, JSON envelope 미사용 — spec § stderr 출력 형식 § warning channel에서 명시 허용. **drift 아님** (사람 결정 — 강제할지). |
| `commands/scan/pipeline.rs:123` | `hash_io` warning channel — 같은 패턴, spec 명시 허용. |

### 3-2. Result-swallowing patterns

발견 0건 (모든 `let _ = ...`은 `std::fmt::Write` infallible 케이스).

### 3-3. `?` chain breaks

| Site | 평가 |
|---|---|
| `commands/scan/walker.rs:95-97` | `walkdir_to_io`가 `walkdir::Error` → `io::Error::other(err.to_string())` 변환으로 underlying io::Error/path/depth context 손실. `shared/gitattributes.rs:213-218` `walk_err_to_gitless`는 `err.into_io_error().map_or_else(..., GitlessError::Io)` 패턴 — divergence. |

**fix 후보**: 두 mapping을 `walkdir::Error → GitlessError::Io` 단일 helper로 dedup. 후행 task.

### 3-4. Manual error constructions (`From` impl 미사용)

| Pattern | 발견 site |
|---|---|
| `serde_json::Error → GitlessError::Http(format!("decode ...: {e}"))` | `shared/github/trees.rs:46-47` + `shared/github/commits.rs:47-48` + `shared/github/blobs.rs:31-32` + `commands/scan/graphql/parse.rs:38-39` |
| `serde_json::Error → GitlessError::Config(...)` | `commands/scan/mod.rs:50-51` (semantically wrong — serialize defect를 Config로 매핑) |
| `repo split` | `commands/scan/mod.rs:78` + `commands/diff/compute.rs:38` + `commands/scan/graphql/query.rs:11-15` (3 site, 3 different message) |

**fix 후보**: `From<serde_json::Error> for GitlessError` impl로 dedup + repo validation을 shared helper로. 후행 task.

### 3-5. `failed_reason` enum gap surfaces (spec'd-but-unimplemented)

spec § Per-file Pitfall Reasons (line 162-191) 9 reason vs 구현 5 variant — 3 gap:

| Spec'd reason | surface site |
|---|---|
| `encoding` | `shared/decode.rs:53-76` `try_decode_text` 결과를 `pipeline.rs::try_short_circuit_failed`에서 plumbing 안 함 — raw bytes로 fall-through |
| `nfd_collision` | `walker.rs::relative_path`에서 NFC canonical 적용 — collision detect 없음 (case_collision::detect는 별도 NFD 처리 안 함) |
| `gitattributes_unsupported` | `shared/normalize.rs::prepare_for_hash`가 `AttributeMatch::Unsupported { .. } | AttributeMatch::Unspecified | AttributeMatch::LfsPointer => apply_unspecified` 일괄 — `Unsupported`가 v0.1 default로 silently demote |

**fix 후보**: 3 caller-side plumbing — 후행 task scope (Phase 5 후속). spec line 162 `현재 상태` § hedge marker 추가됨.

### 3-6. Exit code drift

`shared/error/core.rs:49`에서 `Self::Http(_) => 3` (RateLimitExceeded 동급). spec § Exit Code mapping은 `Http → 1` (line 84) + line 226 `5xx fallthrough → exit code 1`. **드리프트 1건** — 후행 task scope (spec-error-contracts.md § N-task audit hedge marker line 34 명시).

### 3-7. Error message inconsistency

4 message drift — `repo not specified` × 2 sites + `invalid repo format: ... (expected owner/name)` 2 variant. `gh CLI not found`는 `GH_NOT_FOUND_MESSAGE` constant 사용. `decode <kind> response: {e}`가 4 sites에서 같은 패턴 반복. **fix 후보**: shared constant + helper. 후행 task.

## 4. Public API exposure (general-purpose sub-agent)

### 4-1. Inappropriate `pub` (over-exposure)

14 site `pub` demote 후보 — 권장 visibility별 group:

- `pub(super)`: `scan/compare.rs:64` (classify), `scan/walker.rs:11` (LocalFile) + `:38` (walk), `scan/long_path.rs:35` (is_invalid), `scan/output.rs:6` (SCHEMA_VERSION).
- `pub(crate)`: `shared/ignore.rs:16` (IgnoreMatcher), `shared/normalize.rs:18` (is_binary) + `:24` (normalize_text), `shared/config.rs:23` (load), `shared/gitattributes.rs:31` (RawAttribute) + `:154` (AttributeMatch), `shared/error/network.rs:25` (GraphqlErrorExtensions).
- private (drop `pub`): `shared/ignore.rs:7` (BUILTIN_IGNORES), `commands/init/mod.rs:20` (STDERR_HINT).

### 4-2. Over-broad `pub mod`

| Site | 평가 |
|---|---|
| `commands/scan/mod.rs:5-16` | 12 `pub mod` 중 외부에서는 `output::serialize` 1건만 사용 (+ 결과 chain의 `compare::{Status, FailedReason, LfsPointer, FileEntry}` + `output::{ScanReport, Summary}`). 11개를 `pub(crate) mod`로 demote 가능. |
| `shared/mod.rs:1-10` | `pub mod {decode, ignore, normalize, path}`가 외부 노출 안 함 — `pub(crate) mod`로 demote 가능. |

### 4-3. Re-export leaks (`pub use` 외부 노출 부적절)

| Site | 평가 |
|---|---|
| `shared/github/mod.rs:9` `pub use trees::RemoteFile` | intra-crate 사용만 — `pub(crate) use` |
| `shared/error/mod.rs:13` `pub use core::{GitlessError, StderrPayload}` | `StderrPayload`는 internal — demote |
| `shared/error/mod.rs:14` `pub use network::{GraphqlError, GraphqlErrorExtensions, map_graphql_error}` | 3개 모두 intra-crate — `pub(crate) use` |

### 4-4. Unused `pub` (zero callers)

| Site | 평가 |
|---|---|
| `shared/decode.rs:24` `TextDecodeResult` enum | production caller 0건 (test only). § 3-5 `encoding` plumbing 시 production caller 추가 예정. |
| `shared/decode.rs:53` `try_decode_text` fn | 동일. |

### 4-5. Superfluous `Clone` derives

| Site | 평가 |
|---|---|
| `commands/scan/walker.rs:10` `LocalFile` | `Clone` derive 미사용 (`f.relative_path.clone()` 같은 field-level clone만 사용) |
| `shared/github/trees.rs:8` `RemoteFile` | 동일 |

### 4-6. `lib.rs` re-export drift

drift 0건 — `lib.rs:8-9`가 minimal (`pub mod commands; pub mod shared;`). drift는 inner `mod.rs` layer에 (§ 4-1/4-2/4-3 covered).

## 5. Rust idiom (general-purpose sub-agent)

| Category | High | Med | Low |
|---|---|---|---|
| `.clone()` overuse | 0 | 0 | 2 (`commands/scan/mod.rs:82` `cfg.ignore.clone()`, `shared/gitattributes.rs:205` `name.clone()` → `to_owned()`) |
| `&Vec`/`&String` params | 0 | 0 | 0 |
| `Vec::new()` + push | 0 | 0 | 0 |
| `if let Some` vs let-else | 0 | 0 | 0 |
| `match` on Option/bool | 0 | 0 | 1 (`commands/diff/compute.rs:50-53` `match opt { Some => Some(?), None => None }` → `.transpose()` 후보) |
| Manual `Default` impl | 0 | 0 | 0 |
| `format!` interpolation drift | 0 | 0 | 2 (`commands/init/mod.rs:42` + `shared/hash.rs:6` — captured-identifier style 미사용, cosmetic) |
| `unwrap_or` vs `_default` | 0 | 0 | 0 |
| `as` overflow casts | 0 | 0 | 0 |
| Dead code | 0 | 1 | 0 (`shared/gitattributes.rs:17` module-level `#![allow(dead_code)]` — broader 영향. `RawAttribute::Unset/Unspecified`가 production 미사용) |

**총**: 0 high / 1 med / 5 low. 큰 위반 없음 — Phase 6 lint deny gate 효과로 baseline 깨끗.

## 6. Panic escape hatch (general-purpose sub-agent)

baseline **clean**. 발견 결과:

| Category | Production count |
|---|---|
| `#[allow(clippy::*)]` lint bypass | 0 |
| `.expect("invariant")` 패턴 | 0 |
| `unreachable!()` | 1 (`commands/scan/compare.rs:79` contract-documented exhaustive arm) |
| `assert!`/`debug_assert!` | 0 |
| Implicit `[]` indexing panics | 0 (모두 invariant-bounded) |
| Integer arithmetic panic | 0 |
| `.unwrap()` reintroduced | 0 |
| `.expect()` reintroduced | 0 |

**판정**: Phase 6 deny gate (`unwrap_used`/`expect_used`/`panic`)가 효과 발휘. production panic source는 1 documented `unreachable!()`만 존재.

## 7. 종합 — Z task scope

### 7-1. 본 task fix (확정)

`shared/gitattributes.rs` (296 LOC) → `shared/gitattributes/{mod.rs, parser.rs, classify.rs, matching.rs}` 4 file 분할.
- `mod.rs` = re-export hub (`pub(crate) use parser::*; pub(crate) use classify::*; pub(crate) use matching::GitAttributes`)
- `parser.rs` = `RawAttribute` + `LineRule` + `parse_lines`/`parse_one_line`/`parse_attribute` + tests
- `classify.rs` = `AttributeMatch` + `classify_raw_attributes` + `whitelist_match` + `unsupported_name` + tests
- `matching.rs` = `GitAttributes` + `AttributesFile` + `load`/`match_path`/`classify_path`/`is_empty` + walker/path helpers + tests

본 task에 포함된 추가 fix:
- sibling test file 정리 (`gitattributes_tests.rs` + `gitattributes_classify_tests.rs` 제거).
- caller import path (`crate::shared::gitattributes::{GitAttributes, AttributeMatch}`) 호환 유지.

### 7-2. 후행 task 입력 (사람이 추가 — ralph plan 모드 스킵)

발견된 finding 중 follow-up scope. 다음 9건 우선순위 정렬:

1. **Sibling test file cleanup (5 신규)**: `pipeline_tests*.rs` × 4 + `trees_tests.rs` → production module 폴더 분할. § 1-1.
2. **Test helper dedup**: `shared/test_helpers.rs`로 cross-module 통합 (§ 1-4).
3. **`failed_reason` enum gap caller-side plumbing**: `encoding` + `nfd_collision` + `gitattributes_unsupported` (§ 3-5).
4. **Exit code drift fix**: `Http → 1` (§ 3-6).
5. **Visibility tightening**: 14 over-exposure + 2 over-broad `pub mod` + 3 re-export leak + 2 unused pub + 2 superfluous Clone (§ 4).
6. **Error construction dedup**: `From<serde_json::Error>` + repo split helper (§ 3-4).
7. **Module-level `#![allow(dead_code)]` narrow**: gitattributes.rs의 broader 영향 좁히기 (§ 5 dead code).
8. **`init/mod.rs` borderline**: `run`의 23 LOC business logic 분리 여부 — false positive 가능, 사람 결정 (§ 2-1).
9. **walkdir error mapping unify**: `walker.rs::walkdir_to_io` vs `gitattributes.rs::walk_err_to_gitless` (§ 3-3).

### 7-3. clean 영역 — 위반 없음 (Phase 6 deny gate 효과)

- Panic escape hatch (§ 6) — 0 production panic source.
- Cross-slice ref + slice-internal directional discipline (§ 2-5) — 위반 0.
- Default/Clone derives 정합 (manual impl 일관) — § 5 정합.
