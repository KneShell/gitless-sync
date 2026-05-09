# Code Smell Audit — Task Z (Phase 5.12)

> 2026-05-09. 6 병렬 sub-agent (Test / Module / Error / Public-API / Rust-idiom / Panic-escape) audit 결과 정리.
> Z task acceptance: **확정 fix 1건** (`shared/gitattributes.rs` 박은 module 폴더 분할 + sibling test 제거)만 본 task에서 박는다. 박은 박은 finding 박은 박은 박은 후행 task 박을 입력 (`docs/ralph/implementation-plan.md` Phase 6+ 박은 박은 박을 사람이 박음, ralph plan 모드 스킵 정책).

## 1. Test 구조 (Explore sub-agent)

### 1-1. Sibling test files (spec § 금지 패턴 위반)

박은 박은 박은 5 신규 위반 + 2 known (Z task scope):

| File | 박은 박은 |
|---|---|
| `crates/gitless-sync/src/commands/scan/pipeline_tests.rs` | 위반 (신규) |
| `crates/gitless-sync/src/commands/scan/pipeline_tests_lfs.rs` | 위반 (신규) |
| `crates/gitless-sync/src/commands/scan/pipeline_tests_long_path.rs` | 위반 (신규) |
| `crates/gitless-sync/src/commands/scan/pipeline_tests_modes.rs` | 위반 (신규) |
| `crates/gitless-sync/src/shared/github/trees_tests.rs` | 위반 (신규) |
| `crates/gitless-sync/src/shared/gitattributes_tests.rs` | **[Z task fix]** |
| `crates/gitless-sync/src/shared/gitattributes_classify_tests.rs` | **[Z task fix]** |

**박을 fix 패턴**: 각 production source 박은 module 폴더 분할 (예: `pipeline.rs` → `pipeline/{mod.rs, ...}` + 각 sub-module에 `#[cfg(test)] mod tests`). spec § 금지 패턴 — sibling test file 박은 LOC 회피로 박은 박은 박지 말고 production 자체 분할로 박는다.

### 1-2. mod loader sites (`#[cfg(test)] #[path = "..._tests.rs"] mod tests;`)

박은 박은 박은 7 site 모두 `#[cfg(test)]` guard 정확. 1-1 fix cascade 박은 자연 정리.

### 1-3. `#[cfg(test)]` 가드 일관성

박은 박은 0 inconsistency. integration tests (`tests/*.rs`) 박은 redundant guard 0건 (Rust 관용 정합).

### 1-4. Test helper duplication

| Helper | 박은 박은 site |
|---|---|
| `ok_resp(body: &[u8]) -> GhResponse` | `commands/scan/test_helpers.rs:45` + `commands/scan/graphql/test_helpers.rs:9` + `commands/diff/test_helpers.rs:25` + `shared/github/trees_tests.rs:8` |
| `err_resp(stderr: &str) -> GhResponse` | `commands/scan/test_helpers.rs:53` + `commands/scan/graphql/test_helpers.rs:17` + `commands/diff/test_helpers.rs:33` + `shared/github/trees_tests.rs:16` |
| `tree_args(repo, branch) -> Vec<String>` | `commands/scan/test_helpers.rs:61` + `commands/diff/test_helpers.rs:41` + `shared/github/trees_tests.rs:24` |

**fix 후보**: `tests/common/mod.rs` 패턴 mirror — `crates/gitless-sync/src/shared/test_helpers.rs` 박은 `#[cfg(test)] pub(crate) mod` 박은 박은 박은 박은 dedup. 박은 박은 후행 task scope.

## 2. Module 구조 (Explore sub-agent)

### 2-1. mod.rs 박은 thin orchestrator (spec § Module 폴더 단위 정책)

| `mod.rs` | LOC | 박은 박은 |
|---|---|---|
| `commands/scan/mod.rs` | 256 | orchestrator (entry point + thin wrapper). 박은 박은 박은 박은 박은. |
| `commands/init/mod.rs` | 190 | borderline — `run` 박은 23 LOC business logic + tests 130 LOC. spec § thin orchestrator vs single-purpose slice 박은 박은 false positive 가능 (init 박은 단일 책임 박은 박은). 사람 결정. |
| `commands/diff/mod.rs` | 54 | clean. |

### 2-2. Near-cap files (≥ 250 LOC, 300 cap 박은 박은)

| File | LOC | 박은 박은 |
|---|---|---|
| `shared/gitattributes.rs` | 296 (실제 measured 276 + sibling 추가 박지 박은 박은) | **[Z task fix]** module 폴더 분할 |
| `commands/scan/pipeline.rs` | 278 | domain — 후행 phase 진입 시 분할 후보 |
| `shared/normalize.rs` | 259 | domain — 박은 박은 박은 박은 박은 박은 |
| `commands/scan/walker.rs` | 248 | IO — case_collision/long_path 박은 박은 박은 |
| `shared/github/commits.rs` | 252 | IO — Trees/Commits/Blobs 박은 박은 박은 |
| `commands/scan/mod.rs` | 256 | orchestrator |
| `commands/scan/compare.rs` | 222 (test 83 포함) | domain |
| `shared/decode.rs` | 219 | domain |
| `shared/gh.rs` | 212 | IO — gh subprocess wrapper |

### 2-3. 폴더 split 박은 박은 박은

박은 박은 박은 분할 정합:
- `shared/github/{mod, trees, commits, blobs, error_map}` — re-export hub 9 LOC
- `shared/error/{mod, core, network, ...}` — re-export hub 12 LOC
- `commands/scan/graphql/{mod, batch, parse, query, test_helpers}` — re-export hub 35 LOC

**collapse 후보**: 박은 박은 단일 sub-module 박은 박은 폴더 — 박은 박은 박은. 박은 박은 박은 박은 박은.

### 2-4. Visibility leaks → § 4 (Public API audit) 박은 박은 통합.

### 2-5. Cross-slice ref + slice-internal directional violation

박은 박은 0 — `cargo xtask check-cycles` deny gate 박은 박은 박은 baseline 0 박은 박은 정합.

## 3. Error handling (general-purpose sub-agent)

### 3-1. Hand-rolled error formatting (stderr drift)

| Site | 박은 박은 |
|---|---|
| `shared/github/trees.rs:93-98` | non-blob entry warning 박은 plain-text `eprintln!`, JSON envelope 박지 박은 — spec § stderr 출력 형식 § warning channel 박음 박혀있어 명시 허용. **drift 아님** (사람 결정 — 박을지 enforce). |
| `commands/scan/pipeline.rs:123` | `hash_io` warning channel — 같은 패턴, spec 박은 박은 박은 박은. |

### 3-2. Result-swallowing patterns

박은 박은 0건 (모든 `let _ = ...` 박은 `std::fmt::Write` 박은 infallible 박은 박은).

### 3-3. `?` chain breaks

| Site | 박은 박은 |
|---|---|
| `commands/scan/walker.rs:95-97` | `walkdir_to_io` 박은 `walkdir::Error` → `io::Error::other(err.to_string())` 박은 박은 박은 underlying io::Error/path/depth context 박은. `shared/gitattributes.rs:213-218` `walk_err_to_gitless` 박은 `err.into_io_error().map_or_else(..., GitlessError::Io)` 박은 박은 박은 박은 — divergence. |

**fix 후보**: 박은 mapping 박은 `walkdir::Error → GitlessError::Io` 박은 박은 단일 helper 박은 dedup. 박은 박은 후행 task.

### 3-4. Manual error constructions (`From` impl 박지 박은 박은)

| Pattern | 박은 박은 site |
|---|---|
| `serde_json::Error → GitlessError::Http(format!("decode ...: {e}"))` | `shared/github/trees.rs:46-47` + `shared/github/commits.rs:47-48` + `shared/github/blobs.rs:31-32` + `commands/scan/graphql/parse.rs:38-39` |
| `serde_json::Error → GitlessError::Config(...)` | `commands/scan/mod.rs:50-51` (semantically wrong — serialize defect 박은 Config 박은 박은) |
| `repo split 박은 박은 site` | `commands/scan/mod.rs:78` + `commands/diff/compute.rs:38` + `commands/scan/graphql/query.rs:11-15` (3 site, 3 message) |

**fix 후보**: `From<serde_json::Error> for GitlessError` impl 박은 박은 박은 박은 dedup + repo validation 박은 shared helper. 박은 박은 후행 task.

### 3-5. `failed_reason` enum gap surfaces (spec'd-but-unimplemented)

박은 박은 박은 spec § Per-file Pitfall Reasons (line 162-191) 박은 박은 9 reason vs 구현 5 variant — 3 gap:

| Spec'd reason | 박은 박은 surface site |
|---|---|
| `encoding` | `shared/decode.rs:53-76` `try_decode_text` 박은 결과 `pipeline.rs::try_short_circuit_failed` 박은 박은 plumbing 박지 박은 — 박은 raw bytes 박은 fall-through |
| `nfd_collision` | `walker.rs::relative_path` 박은 NFC canonical 박음 — collision 박은 박은 박은 (case_collision::detect 박은 박은 박은 NFD 박은 박은) |
| `gitattributes_unsupported` | `shared/normalize.rs::prepare_for_hash` 박은 `AttributeMatch::Unsupported { .. } | AttributeMatch::Unspecified | AttributeMatch::LfsPointer => apply_unspecified` 박은 박은 박은 — `Unsupported` 박은 v0.1 default 박은 silently demote |

**fix 후보**: 3 caller-side plumbing 박은 박은 — 박은 박은 후행 task scope (Phase 5 후속 박은 박은). spec line 162 `현재 상태` § 박은 박은 박은 박은 hedge 박힘.

### 3-6. Exit code drift

`shared/error/core.rs:49` 박은 `Self::Http(_) => 3` (RateLimitExceeded 박은 박음). spec § Exit Code mapping 박은 `Http → 1` (line 84) + line 226 `5xx fallthrough → exit code 1`. **드리프트 1건** — 박은 박은 후행 task scope (spec-error-contracts.md § N-task audit hedge marker line 34 박은 박은).

### 3-7. Error message inconsistency

박은 박은 4 message drift — `repo not specified` × 2 sites + `invalid repo format: ... (expected owner/name)` 박은 박은 박은 박은. `gh CLI not found` 박은 `GH_NOT_FOUND_MESSAGE` 박은 박은 박은. `decode <kind> response: {e}` 박은 4 sites 박은 박은. **fix 후보**: shared constant + 박은 박은 박은 후행 task.

## 4. Public API exposure (general-purpose sub-agent)

### 4-1. Inappropriate `pub` (over-exposure)

박은 박은 박은 14 site 박은 `pub` → `pub(crate)`/`pub(super)` 박은 후보:

| Site | 박은 박은 박은 |
|---|---|
| `commands/scan/compare.rs:64` `classify` fn | `pub(super)` |
| `commands/scan/walker.rs:38` `walk` fn | `pub(super)` |
| `commands/scan/walker.rs:11` `LocalFile` struct | `pub(super)` |
| `commands/scan/long_path.rs:35` `is_invalid` fn | `pub(super)` |
| `commands/scan/output.rs:6` `SCHEMA_VERSION` const | `pub(super)` |
| `shared/ignore.rs:7` `BUILTIN_IGNORES` const | private (drop `pub`) |
| `shared/ignore.rs:16` `IgnoreMatcher` struct | `pub(crate)` |
| `commands/init/mod.rs:20` `STDERR_HINT` const | private |
| `shared/normalize.rs:18` `is_binary` fn | `pub(crate)` |
| `shared/normalize.rs:24` `normalize_text` fn | `pub(crate)` |
| `shared/config.rs:23` `load` fn | `pub(crate)` |
| `shared/gitattributes.rs:31` `RawAttribute` enum | `pub(crate)` |
| `shared/gitattributes.rs:154` `AttributeMatch` enum | `pub(crate)` |
| `shared/error/network.rs:25` `GraphqlErrorExtensions` struct | `pub(crate)` |

### 4-2. Over-broad `pub mod`

| Site | 박은 박은 |
|---|---|
| `commands/scan/mod.rs:5-16` | 12 `pub mod` 박은 외부 박은 `output::serialize` 박은 박은 (+ 박은 박은 chain 박은 `compare::{Status, FailedReason, LfsPointer, FileEntry}` + `output::{ScanReport, Summary}` 박은 박은). 11 박은 `pub(crate) mod` 박은 박은. |
| `shared/mod.rs:1-10` | `pub mod {decode, ignore, normalize, path}` 박은 박은 외부 박은 박은 박은 — `pub(crate) mod` 박은 박은. |

### 4-3. Re-export leaks (`pub use` 박은 박은 박은 internal)

| Site | 박은 박은 |
|---|---|
| `shared/github/mod.rs:9` `pub use trees::RemoteFile` | intra-crate 박은 박은 — `pub(crate) use` |
| `shared/error/mod.rs:13` `pub use core::{GitlessError, StderrPayload}` | `StderrPayload` 박은 박은 박은 박은 internal — demote |
| `shared/error/mod.rs:14` `pub use network::{GraphqlError, GraphqlErrorExtensions, map_graphql_error}` | 3 박은 박은 intra-crate 박은 박은 — `pub(crate) use` |

### 4-4. Unused `pub` (zero callers)

| Site | 박은 박은 |
|---|---|
| `shared/decode.rs:24` `TextDecodeResult` enum | production 박은 박은 박은 (test only). § 3-5 `encoding` plumbing 박은 박은 production 박은 박은 박은 박은. |
| `shared/decode.rs:53` `try_decode_text` fn | 박은 박은 박은. |

### 4-5. Superfluous `Clone` derives

| Site | 박은 박은 |
|---|---|
| `commands/scan/walker.rs:10` `LocalFile` 박은 `Clone` derive | 박은 박은 박은 (`f.relative_path.clone()` 박은 field 박은 박음) |
| `shared/github/trees.rs:8` `RemoteFile` 박은 `Clone` derive | 박은 박은 박은 |

### 4-6. `lib.rs` re-export drift

박은 박은 0 — `lib.rs:8-9` 박은 minimal (`pub mod commands; pub mod shared;`). drift 박은 inner `mod.rs` layer 박은 박음 (§ 4-1/4-2/4-3 박음 covered).

## 5. Rust idiom (general-purpose sub-agent)

| Category | High | Med | Low |
|---|---|---|---|
| `.clone()` overuse | 0 | 0 | 2 (`commands/scan/mod.rs:82` `cfg.ignore.clone()`, `shared/gitattributes.rs:205` `name.clone()` → `to_owned()`) |
| `&Vec`/`&String` params | 0 | 0 | 0 |
| `Vec::new()` + push | 0 | 0 | 0 |
| `if let Some` vs let-else | 0 | 0 | 0 |
| `match` on Option/bool | 0 | 0 | 1 (`commands/diff/compute.rs:50-53` `match opt { Some => Some(?), None => None }` → `.transpose()` 후보) |
| Manual `Default` impl | 0 | 0 | 0 |
| `format!` interpolation drift | 0 | 0 | 2 (`commands/init/mod.rs:42` + `shared/hash.rs:6` — captured-identifier style 박지 박은 박은 cosmetic) |
| `unwrap_or` vs `_default` | 0 | 0 | 0 |
| `as` overflow casts | 0 | 0 | 0 |
| Dead code | 0 | 1 | 0 (`shared/gitattributes.rs:17` 박은 module-level `#![allow(dead_code)]` — 박은 박은 박은 박은 broader 박은 박은. `RawAttribute::Unset/Unspecified` 박은 production 박은 박은 박은) |

**총**: 0 high / 1 med / 5 low. 박은 박은 박은 박은 박은 — Phase 6 lint deny 박은 박은 박은 박은 박은 박은.

## 6. Panic escape hatch (general-purpose sub-agent)

박은 박은 **clean**. 박은 박은 박은:

| Category | Production count |
|---|---|
| `#[allow(clippy::*)]` lint bypass | 0 |
| `.expect("invariant")` 박은 박은 | 0 |
| `unreachable!()` | 1 (`commands/scan/compare.rs:79` 박은 contract-documented exhaustive arm) |
| `assert!`/`debug_assert!` 박은 박은 | 0 |
| Implicit `[]` indexing panics | 0 (모든 박은 invariant-bounded) |
| Integer arithmetic panic | 0 |
| `.unwrap()` reintroduced | 0 |
| `.expect()` reintroduced | 0 |

**판정**: 박은 박은 박은 deny gate (`unwrap_used`/`expect_used`/`panic`) 박은 박은 박은 박은 박은. 박은 박은 박은 production panic source 박은 1 documented `unreachable!()` 박은 박음.

## 7. 종합 — Z task scope

### 7-1. 본 task 박을 fix (확정)

`shared/gitattributes.rs` (296 LOC) → `shared/gitattributes/{mod.rs, parser.rs, classify.rs, matching.rs}` 4 file 분할.
- `mod.rs` = re-export hub (`pub(crate) use parser::*; pub(crate) use classify::*; pub(crate) use matching::GitAttributes`)
- `parser.rs` = `RawAttribute` + `LineRule` + `parse_lines`/`parse_one_line`/`parse_attribute` + tests
- `classify.rs` = `AttributeMatch` + `classify_raw_attributes` + `whitelist_match` + `unsupported_name` + tests
- `matching.rs` = `GitAttributes` + `AttributesFile` + `load`/`match_path`/`classify_path`/`is_empty` + walker/path helpers + tests

박은 박은 box 박은 박은 fix 박지 박은:
- sibling test file 정리 (`gitattributes_tests.rs` + `gitattributes_classify_tests.rs` 제거).
- 박은 박은 caller import path 박은 (`crate::shared::gitattributes::{GitAttributes, AttributeMatch}`) 박은 박은 박은.

### 7-2. 후행 task 입력 (사람이 박음 — ralph plan 모드 스킵)

박은 박은 박은 finding 박은 박은 박은 follow-up scope. 박은 박은 박은 박은 박은 박은 박은 박은:

1. **Sibling test file cleanup (5 신규)**: `pipeline_tests*.rs` × 4 + `trees_tests.rs` → production module 폴더 분할. § 1-1.
2. **Test helper dedup**: `shared/test_helpers.rs` 박은 박은 cross-module 박은 (§ 1-4).
3. **`failed_reason` enum gap caller-side plumbing**: `encoding` + `nfd_collision` + `gitattributes_unsupported` (§ 3-5).
4. **Exit code drift fix**: `Http → 1` (§ 3-6).
5. **Visibility tightening**: 14 over-exposure + 2 over-broad `pub mod` + 3 re-export leak + 2 unused pub + 2 superfluous Clone (§ 4).
6. **Error construction dedup**: `From<serde_json::Error>` + repo split helper (§ 3-4).
7. **Module-level `#![allow(dead_code)]` narrow**: gitattributes.rs 박은 박은 박은 박은 박은 (§ 5 dead code).
8. **`init/mod.rs` borderline**: `run` 박은 23 LOC business logic 박은 박은 분리 박을지 박은 박은 false positive — 사람 결정 (§ 2-1).
9. **walkdir error mapping unify**: `walker.rs::walkdir_to_io` vs `gitattributes.rs::walk_err_to_gitless` (§ 3-3).

### 7-3. clean 영역 — 박은 박은 박은 (Phase 6 deny gate 박은 박은 박은 박은)

- Panic escape hatch (§ 6) — 박은 박은 박은 0 production panic source.
- Cross-slice ref + slice-internal directional discipline (§ 2-5) — 박은 박은 0.
- Default/Clone derives 박은 박은 박은 (manual impl 박은 박은 박은 박은) — § 5 박은 박은 박음.
