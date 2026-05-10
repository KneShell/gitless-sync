# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.1] - 2026-05-10

> Phase 8 post-eval minor fix. v0.4.0 가 도입한 clap `value_enum` (F5) 부산물 회복 + F4 백틱 노이즈 정리. 외부 surface 변경: 잘못된 인자 시 사람용 multi-line text + exit 2 → JSON 한 줄 + exit 1 (CONFIG_ERROR) 로 복귀 — v0.3 이전 한 줄 JSON contract 와 의미 일관.

### Changed

- **F5 contract 회복** — `Cli::parse()` → `Cli::try_parse()` + `main::map_clap_parse_error` helper. clap argument-parse 실패 (예: `--status drif`) 가 `GitlessError::Config(_)` 로 wrap 되어 기존 `to_stderr_payload()` 경로로 한 줄 JSON (`{"error_code":"CONFIG_ERROR","message":"..."}`) + exit 1 contract 회복. `message` 안에 clap multi-line 출력 (escape 처리, valid 후보 5개 + did-you-mean hint) 그대로 보존 — 정보 손실 0. `--help` / `--version` / `DisplayHelpOnMissingArgumentOrSubcommand` (bare invocation) 은 `None` 분기로 clap 기본 출력 (stdout + exit 0) 통과.
- **F4 백틱 노이즈 제거** — `main.rs` `#[arg]` doc comment 7곳에서 markdown 백틱 제거. clap 이 백틱을 raw 출력해 `--help` 에 `` `GitHub` `` / `` `JSON` `` 등이 노출되던 cosmetic noise 해소. lib internal doc 은 유지.
- **main.rs 분해** — `dispatch` + `emit_error` helper 분리 (clippy `too_many_lines` 60 cap 회복). `handle_clap_parse_error` signature `&clap::Error` (`needless_pass_by_value`).

### Spec

- `spec-error-contracts.md` § Custom Error Types Config(String) 의미에 clap argument-parse 실패 매핑 박음. § Acceptance Criteria 새 `[AUTO]` 시나리오 (잘못된 `--status` / `--help` / `--version` / bare invocation 분기 검증) 추가.

### Verified

- Hard gate full pipeline PASS — `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo xtask check-line-limits` / `cargo xtask check-cycles` / `cargo machete` / `cargo test --workspace` / `cargo xtask check-readme-examples` / `cargo tarpaulin --engine llvm --workspace --fail-under 80`.
- 484 + 88 unit + integration test pass / 0 failed (gitless-sync workspace + xtask).
- 새 unit test 4건 (`map_clap_parse_error` Display kind None × 3 + invalid status `Some(Config)` + 5 후보 + did-you-mean + exit/error_code 검증).
- tarpaulin 88.47% (1305/1475 lines).
- vault dogfood (KneShell/obsidian-vault, 363 files) — 4-state 281/60/22/0/0 + 0 drift + 0 failed v0.4.0 baseline 유지 (regression 0).

## [0.4.0] - 2026-05-10

> Phase 8 (LLM-as-Caller Usability Fix, eval 7 friction 해소) 누적. Phase 8.1 (spec/ADR 사전 확정) + Phase 8.2 (F1+F2 schema) + Phase 8.3 (F3 diff --json) + Phase 8.4 (F4/F5/F6 clap surface) + Phase 8.5 (F7 CI README sanity) + Phase 8.6 (release tag). 상세 task별 결과는 git history (`git log --grep="<task ID>"`) + `docs/ralph/implementation-plan.md`.

### Added

- Phase 8.2 — `files[].diff_meaningful: Option<bool>` + `files[].presence: "local_only"|"both"|"remote_only"` field 추가 (F1+F2). F1: sha differ but normalize-equal (e.g., CRLF vs LF) 케이스를 AI 호출자가 1 scan 호출로 판별 가능 — `diff_meaningful=Some(false)`로 불필요한 `diff` 호출 생략. F2: 4-state status 유지 + `presence` field로 방향 즉시 확인. Failed / local-or-remote-only entry는 `diff_meaningful=None`.
- Phase 8.3 — `diff --json` opt-in flag (F3). 기존 unified text output 기본 유지 + `--json` 분기 시 `{"side": "...", "unified": "..." | null, "raw": "..." | null, "binary": bool}` 직렬화. AI 호출자가 파싱 없이 구조화 diff 수신 가능.
- Phase 8.4 — clap surface 개선 3종 (F4/F5/F6). F4: 각 flag `///` doc comment 1줄 (--help 가독성). F5: `--status` `value_enum` derive + `value_delimiter = ','` (valid 후보 자동 노출 + 에러 친화). F6: `--branch` `default_value = "main"` (--help에 기본값 노출).
- Phase 8.5 — `cargo xtask check-readme-examples` sub-command 신규 (F7). README Quick Start 코드블록 실제 실행 + exit 0 검증. CI hard gate 합류.
- Schema v1.2 → v1.3 minor bump — `files[].diff_meaningful: Option<bool>` + `files[].presence: enum` 신규 field. v1.0/v1.1/v1.2 backward-compat 유지 (lock test 갱신).
- ADR 0014 'scan-diff metadata contract' 신규 — F1+F2 묶음 결정 trail.

### Changed

- schema_version `"1.2"` → `"1.3"`.

### Verified

- eval 7 friction (P0~P3) 해소 + Phase 8.5 regression baseline (`cargo xtask synth-vault --count 1000 --seed 42` — 추가 field가 4-state breakdown 유지 검증). 상세 결과: `docs/research/phase8-regression.md`.

### Known limitations (v0.5+ 해소 예정)

- sub-tree fallback real public repo dogfood 부재 — git/git는 truncation 영역 외, linux/torvalds는 budget 1000 cap 위반. 현 baseline은 unit/integration test (Phase 7.1 task F/G) cover. budget 정책 진화 task 도입 시 재검토.
- hash phase instrumentation 부재 — ADR 0008 § Phase 7.3 재검토 mtime cache 재도입 트리거 (a)/(b) 정량 verify에 필요. yagni 일관 deferred — 별도 instrumentation work 도입 시점에 검토.

## [0.3.0] - 2026-05-10

> Phase 7 (vault scale + Trees sub-tree + 큰 파일 임계치) 누적. Phase 7.1 (Trees sub-tree fallback) + Phase 7.2 (큰 파일 임계치) + Phase 7.3 (vault scale dogfood, T main bench + U public repo cross-check + V mtime cache keep-drop confirm + W 종합) + Phase 7.4 (release tag). 상세 task별 결과는 git history (`git log --grep="<task ID>"`) + `docs/ralph/implementation-plan.md`.

### Added

- Phase 7.1 — Trees sub-tree 재귀 fallback (G-002 해소). `truncated:true` 응답 시 ref → commit → root tree 1회 resolve 후 sub-tree non-recursive fan-out으로 내려가며 entries 합산 (`shared/github/trees/fallback.rs`). Cap: call budget 1000 + entries 500_000. Cap 초과 또는 fallback 실패 시 `GitlessError::TreesTruncated` + exit code 5 (정책 일관, 부분 결과 사용 금지). 정상 path는 v0.2.x 동작 유지.
- Phase 7.2 — 큰 파일 임계치 2 reason (`file_too_large` ≥ 100 MB GitHub Blobs API hard limit / `memory_exceeded` ≥ 50 MB tool 메모리 안전 임계). `commands/scan/hash_local.rs::try_hash_local`가 `fs::metadata().len()` pre-flight + 분기, `shared/github/blobs.rs::fetch_blob_with_size_gate`가 Trees response size field로 fetch 전 분기 (skip 시 fetch_blob 호출 0회). cascade는 short_circuit.rs LFS 다음 우선순위 9 reason로 갱신.
- Schema v1.1 → v1.2 (minor bump) — `failed_reason` enum 9 → 11 (`file_too_large` + `memory_exceeded` 추가) + 신규 field `files[].size_bytes` (`Option<u64>`, `#[serde(skip_serializing_if = "Option::is_none")]`). `file_too_large` / `memory_exceeded` entry는 size_bytes 포함 + `is_binary: false` (size pre-flight short-circuit, local read 전 격하). 그 외 entry는 size_bytes omit. v1.0 / v1.1 backward-compat 유지 (lock test `output.rs::tests` 갱신 — 이전 envelope/entry 필수 field invariant 박힘).
- `shared/github/trees/parse.rs::TreeEntry::size` — `Option<u64>` field 추가 (Trees response size field 활용, sub-tree fallback + remote-side size pre-flight 양쪽 사용).
- Specs — spec-github-api.md § Trees truncation handling + spec-hash-and-normalize.md § Phase 7 — 큰 파일 처리 + spec-output-schema.md § v1.2 + 신규 Acceptance Criteria 7 시나리오.
- ADR — 0011 (Trees sub-tree fallback) + 0012 (큰 파일 임계치 100/50 MB) + 0013 (자율 chain hard cap depth 3 + token 200k + wall-clock 6h). ADR 0008 § Phase 7.3 재검토 추가 — 1000+ path scale에서 mtime cache 재도입 트리거 keep-drop 유지 (path 20× scale에도 walltime 1324.8 ms → 829/1109 ms로 hash 비중 폭증 신호 없음, 향후 trigger (a)/(b) 명시).
- guardrails — G-019 (자율 chain hard cap, Phase 7 vague 결과).
- Phase 7.3 — `xtask synth-vault` sub-command (1000+ markdown 합성 vault generator, seed/UTF-8 NFC/LF/mtime epoch 정책 정합, `xtask/src/synth_vault.rs`). dogfood + scale 측정용 — 도구 본체 contract 영향 0.
- Research — `docs/research/phase7-vault-scale-bench.md` (T main bench 1000 local × 129 remote 3 runs + U public repo cross-check 1000 local × 4964 remote git/git@94f0577 single run + W 종합 cross-comparison + ADR 0008 cross-link).

### Changed

- G-002 (Trees API truncation) — v0.2.x "즉시 fail" 정책 → v0.3 sub-tree fallback 도입. cap 초과 또는 fallback 실패 시에만 `GitlessError::TreesTruncated` + exit code 5.
- `compare.rs::FailedReason` enum 8 variant → 10 variant (`FileTooLarge` + `MemoryExceeded` 추가). `None` special case (`hash_io`)는 그대로 — 11 wire reason 모두 cover.

### Verified

- Phase 7.1 unit test 2 시나리오 (call budget 1001 / entries 500_001 cap trip) + integration test (multi-layer truncated descent → 합산 ScanReport).
- Phase 7.2 unit test 4 시나리오 (49MB local hash 정상 / 51MB memory_exceeded / 101MB file_too_large / 30MB LFS pointer 우선순위).
- Schema v1.2 acceptance 7 시나리오 unit test (`output.rs::tests`) — schema_version "1.2" + `FailedReason` 11 wire snake_case + size_bytes 정확 직렬화/omit + size_gate entry `is_binary: false` + v1.0/v1.1 envelope+entry 필수 field 박힘 invariant.
- Phase 7.3 vault dogfood — T main bench (1000 local markdown × 129 remote KneShell/gitless-sync, 3 runs cold+2warm, mean 829 ms, exit 0, failed 0, schema v1.2) + U public repo cross-check (1000 local × 4964 remote git/git@94f0577, 1 manual sanity run 1109 ms, exit 4 PARTIAL_FAILURE, failed 4 = 3 symlink `120000` + 1 submodule `160000` git/git remote-side, schema v1.2). Cross-comparison — remote 38× scale-up (129 → 4964)에 walltime ~+35% 증가 그침 (sub-linear, GraphQL batching + rayon 8c local hash 흡수). ADR 0008 § Phase 7.3 재검토 — mtime cache keep-drop 유지 (path 20× scale에도 walltime 폭증 신호 없음, hash phase instrumentation 부재로 정량 verify 불가 → yagni 일관). 종합 분석 + open items: `docs/research/phase7-vault-scale-bench.md` § 종합 (task W).

### Known limitations (v0.4+ 해소 예정)

- sub-tree fallback real public repo dogfood 부재 — git/git는 truncation 영역 외, linux/torvalds는 budget 1000 cap 위반. 현 baseline은 unit/integration test (Phase 7.1 task F/G) cover. budget 정책 진화 task 도입 시 재검토.
- hash phase instrumentation 부재 — ADR 0008 § Phase 7.3 재검토 mtime cache 재도입 트리거 (a)/(b) 정량 verify에 필요. yagni 일관 deferred — 별도 instrumentation work 도입 시점에 검토.

## [0.2.1] - 2026-05-10

> Phase 5 (도메인 함정 정리) + Phase 5.13/5.13.1/5.14 (follow-up) + Phase 6.1 (v0.2.x cleanup) 누적. v0.2.0은 ADR 0002 (gh subprocess migration) tag로 이미 사용. 본 release는 v0.2.x sub-phase patch increment. 상세 task별 결과는 git history (`git log --grep="<task ID>"`) + `docs/ralph/implementation-plan.md`.

### Added

- 8 핵심 함정 detect/handle — NFC normalize / case_collision (3 시나리오) / `encoding_rs` sniff (UTF-8 + Shift_JIS/EUC_KR/GBK/Windows-1252 + binary fallback, hash 입력은 항상 raw bytes — b-policy) / submodule (160000) detect-only / symlink (120000) lstat-only / 빈 파일 실파일 검증 (G-010) / executable (100755) mode bit / `.gitattributes` 파서 (project root + 하위 1회 로드 + glob + 가장 깊은 winner).
- 추가 함정 4건 — UTF-8 BOM strip + UTF-16 BOM (`FF FE`/`FE FF`) → `failed_reason: "encoding"` / git LFS pointer (`filter=lfs` path 자동 Failed + diff first-line signature 검증) / Windows long path 260+ + 예약 파일명 (`CON`/`PRN`/`NUL`/`AUX`/`COM1-9`/`LPT1-9`) / `.gitignore` 무시 정책 spec 명시.
- `.gitattributes` 화이트리스트 5 entry — `text=auto` / `binary` / `eol=lf` / `eol=crlf` / `filter=lfs`. 그 외 (`working-tree-encoding`, `ident`, `filter=*` (lfs 외), macro, legacy `crlf`) → `Unsupported` + `Status::Failed`.
- Schema v1.1 (minor bump) — 신규 field `mode` (4-digit octal: `100644`/`100755`/`120000`/`160000`) + `failed_reason` (9 enum, skip_serializing on `None`) + `lfs_pointer` (skip on `None`). v1.0 backward-compat lock test (`output.rs::tests` 5 lock).
- Dependencies — `unicode-normalization = "0.1"` (NFC) + `encoding_rs = "0.8"` (Apache-2.0/MIT). cargo-bloat `.text` attribution 0 KiB (LTO + strip + dead code elim).
- Specs — `docs/specs/spec-domain-pitfalls.md` (Phase 5 함정 spec hub).
- Research — `docs/research/phase5-{vault-baseline, vault-after, regression, gitattributes-bench, scan-scale-bench}.md` + `encoding-library-eval.md` + `phase4-measurements.md` (ADR 0003/0006/0007/0008 raw data hub).
- Phase 5.13/5.13.1 follow-up — `failed_reason` 3건 (`encoding`/`nfd_collision`/`gitattributes_unsupported`) caller plumbing 완성 + `shared/github/trees/` module 폴더 분할 + `commands/scan/pipeline/` module 폴더 분할 + sibling test 5 file 제거 + tmp/ + 외부 worktree 정리 + `.gitignore`에 `tmp/` 추가.
- Phase 5.14 (md 자료 audit) — CLAUDE.md "### 메모리 환경" section 제거 (privacy critical, vault path/admin username 노출 제거) + CLAUDE.md slim 142→45 LOC + CHANGELOG.md slim 159→76 LOC + ADR (0001~0009) audit + research/specs/ralph privacy 일반화 (vault path/`<project root>` placeholder).
- Phase 6.1 (v0.2.x cleanup) — ADR 0010 (cognitive_complexity vs LOC 300 orthogonal proxy 둘 다 유지 결정) + CI runner Linux 전환 (`windows-latest` → `ubuntu-latest`, 비용 + cold-start 둘 다 우위, G-018 cross-platform cfg gate 신규 발견 + fix).
- guardrails — G-016 (`cargo fmt --check` 무조건) + G-017 (gh `-F` GET→POST 자동 전환) + G-018 (cross-platform cfg gate — Windows-only `use`/test).
- CI gate Linux runner — `.github/workflows/ci.yml` 4 게이트 (`fmt --check` / `clippy -D warnings` / `test --workspace` / `tarpaulin --engine llvm --fail-under 80`). public-api diff PR-only.

### Changed

- `prepare_for_hash` 시그니처 — `gitattr: &Arc<GitAttributes>` + `path: &str` 추가. caller 모두 갱신.
- `status="failed"` 의미 확장 — v0.1 = "hash IO 실패"만, v0.2 = 9 reasons (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported`). caller가 `failed_reason` 분기 필요. 9 reason 모두 caller plumbing 완성 (Phase 5.13.1 EE/FF/GG).
- default LF normalize — v0.1 = 항상, v0.2 = `.gitattributes` conditional + 화이트리스트 5 entry. `Unspecified` (default) branch는 v0.1 정책 그대로 유지.
- implementation-plan.md skeleton inline slim — 440→246 LOC (completed task `결과` paragraph + sub-bullet 제거, header만 retain).

### Fixed

- G-005 mtime 휴리스틱 정합 — `local_mtime == remote_last_commit_at` 동률은 `Status::Drift`로 격하 (각 함정 처리 후 정합 재검증).
- hash_pass.rs PreState doc comment 3줄 중복 (sub-claude AUTO-FIX, EE diff cleanup miss).
- CLAUDE.md G-017 stale cross-ref + research 3 file 절대 경로 (`D:\00.Projects\02.Personal\05.gitless-sync` → `<project root>`).
- scan_errors.rs:13 `use std::fs;` cfg(windows) gate (Linux CI unused import) + compute.rs:267 backslash test cfg(windows) gate (Linux invalid path).

### Verified

- vault dogfood (T) — KneShell/gitless-sync@main 117 files / 0 drift / 0 failed.
- v0.1 baseline regression diff (W) — REGRESSION 0건 (envelope schema_version + mode field 정확화만, 121/121 path binary delta 0).
- 410+ tests pass + tarpaulin 90.57% (Phase 5 baseline 90.73% 대비 -0.16% 자연 변동).
- Linux CI runner 1차 검증 통과 (commit `87d2cf4`, 19m7s).

### Known limitations (v0.3+에서 해소 검토)

- vault scale 1000+ path dogfood 미측정 — Phase 5/6.1까지는 13~117 path scale 한정. Phase 7 진입 시 측정 + mtime cache 재도입 트리거 검토 (ADR 0008 § Future work).
- Trees API truncated repo 미지원 (G-002, 7MB or 100K entry 한도) — Phase 7+ sub-tree 재귀 fallback 도입 검토.
- 큰 파일 임계치 미정 (10MB+ 메모리 + Phase 4 cache 연결) — Phase 7+ 도입 검토.

## [0.1.0] - Phase 6 완료 시점 (2026-05-09)

> Phase 1 ~ Phase 6 누적 (v0.1 + Phase 4 GraphQL batching + Phase 6 Code Quality).

### Added

- CLI 명령어 — `scan` (디렉토리↔repo 4-state JSON 출력) + `diff` (단일 파일 차이) + `init` (`gitless-sync.toml` stdout TOML emit).
- 분류 — 4-state (`identical`/`local_only_changed`/`remote_only_changed`/`drift`/`failed`) + 시간 비교 휴리스틱 (G-005) + Schema v1.0 (호출자 backward-compat 보장).
- Hash & Normalize — 자체 정의 SHA-1 (`SHA-1("blob <size>\0<normalized content>")`, git 표준 blob SHA 아님 — G-001) + LF normalize + UTF-8 BOM strip (`--keep-bom` 시 보존) + binary 휴리스틱 (NUL byte 8000 안).
- GitHub API — `gh` CLI subprocess 단일 통로 (ADR 0001/0002) + REST + GraphQL (alias batching, default since Phase 4 — ADR 0006) + `MockGhClient` trait inject.
- Quality Gates (Phase 6) — clippy 60/15/5 deny + panic 검출 (`unwrap_used`/`expect_used`/`panic`, tests 면제) + LOC 300 게이트 + cycle/cross-slice 0건 + 외부 도구 (cargo-public-api / machete / tarpaulin / deny / audit) + CI gate.

### Architecture decisions

- Read-only 영구 (ADR 0001) + Vertical slice + cross-slice 직접 ref 금지 + slice 안 acyclic + directional discipline (orchestrator → domain → IO) + Windows 1차 (실행 환경) + MSRV 1.95.0 stable + `#![forbid(unsafe_code)]` + release `panic = "abort"` + `lto = "thin"` + `strip = true` + 박제 expiration (Phase 진입마다 재검토).

### Verified

- vault dogfooding 356 files / 0 drift (2026-04-29 ureq baseline).
- Phase 6 baseline: 244 tests pass (174 lib + 21 integration + 49 xtask) + tarpaulin 88.31%.

### Known limitations (v0.2에서 해소)

- 도메인 함정 8건 + 추가 함정 4건 (UTF-16 BOM/LFS pointer/Windows long path/`.gitignore` 정책).

### Excluded (영구 비목표)

- 3-way merge / 양방향 동기화 / 인터랙티브 UI / GitHub 외 호스팅 (GitLab/Bitbucket) / LFS 추적 파일 직접 fetch (pointer detect-only) / Event 기반 layer 통신 (channel/observer/actor).
