# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

TBD — Phase 7+ 진입 시점 추가.

## [0.2.0] - 2026-05-09

> Phase 5 — 도메인 함정 정리. 상세 task별 결과는 `docs/ralph/implementation-plan.md` § Phase 5.

### Added

- 8 핵심 함정 detect/handle — NFC normalize / case_collision (3 시나리오) / `encoding_rs` sniff (UTF-8 + Shift_JIS/EUC_KR/GBK/Windows-1252 + binary fallback, hash 입력은 항상 raw bytes — b-policy) / submodule (160000) detect-only / symlink (120000) lstat-only / 빈 파일 실파일 검증 (G-010) / executable (100755) mode bit / `.gitattributes` 파서 (project root + 하위 1회 로드 + glob + 가장 깊은 winner).
- 추가 함정 4건 — UTF-8 BOM strip + UTF-16 BOM (`FF FE`/`FE FF`) → `failed_reason: "encoding"` / git LFS pointer (`filter=lfs` path 자동 Failed + diff first-line signature 검증) / Windows long path 260+ + 예약 파일명 (`CON`/`PRN`/`NUL`/`AUX`/`COM1-9`/`LPT1-9`) / `.gitignore` 무시 정책 spec 명시.
- `.gitattributes` 화이트리스트 5 entry — `text=auto` / `binary` / `eol=lf` / `eol=crlf` / `filter=lfs`. 그 외 (`working-tree-encoding`, `ident`, `filter=*` (lfs 외), macro, legacy `crlf`) → `Unsupported` + `Status::Failed`.
- Schema v1.1 (minor bump) — 신규 field `mode` (4-digit octal: `100644`/`100755`/`120000`/`160000`) + `failed_reason` (9 enum, skip_serializing on `None`) + `lfs_pointer` (skip on `None`). v1.0 backward-compat lock test (`output.rs::tests` 5 lock).
- Dependencies — `unicode-normalization = "0.1"` (NFC) + `encoding_rs = "0.8"` (Apache-2.0/MIT). cargo-bloat `.text` attribution 0 KiB (LTO + strip + dead code elim).
- Specs — `docs/specs/spec-domain-pitfalls.md` (Phase 5 함정 spec hub).
- Research — `docs/research/phase5-{vault-baseline, vault-after, regression, gitattributes-bench, scan-scale-bench}.md` + `encoding-library-eval.md`.
- CI gate Windows runner — `.github/workflows/ci.yml` 4 게이트 (`fmt --check` / `clippy -D warnings` / `test --workspace` / `tarpaulin --fail-under 80`).

### Changed

- `prepare_for_hash` 시그니처 — `gitattr: &Arc<GitAttributes>` + `path: &str` 추가. caller 모두 갱신.
- `status="failed"` 의미 확장 — v0.1 = "hash IO 실패"만, v0.2 = 9 reasons (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported`). caller가 `failed_reason` 분기 필요. 6 reason 코드 구현 (`hash_io`/`submodule`/`symlink`/`lfs_pointer`/`long_path`/`case_collision`), 3 reason enum-spec'd-but-unimplemented (`encoding`/`nfd_collision`/`gitattributes_unsupported`).
- default LF normalize — v0.1 = 항상, v0.2 = `.gitattributes` conditional + 화이트리스트 5 entry. `Unspecified` (default) branch는 v0.1 정책 그대로 유지.

### Fixed

- G-005 mtime 휴리스틱 정합 — `local_mtime == remote_last_commit_at` 동률은 `Status::Drift`로 격하 (각 함정 처리 후 정합 재검증).

### Verified

- vault dogfood (T) — KneShell/gitless-sync@main 117 files / 0 drift / 0 failed.
- v0.1 baseline regression diff (W) — REGRESSION 0건 (envelope schema_version + mode field 정확화만, 121/121 path binary delta 0).
- 383 tests pass (293 lib + 41 integration + 49 xtask) + tarpaulin 90.73% (949/1046 lines).

### Known limitations

- `failed_reason` 3건 plumbing follow-up (`encoding`/`nfd_collision`/`gitattributes_unsupported`) — caller-side `pipeline.rs`.
- `.gitattributes` module 폴더 분할 (task Z) — `shared/gitattributes/{mod, parser, classify, matching}` 4 file + sibling test 정리.
- vault scale 1000+ path mtime cache 재검토는 v0.3+.

## [0.1.0] - Phase 6 완료 시점 (2026-05-09)

> Phase 1 ~ Phase 6 누적 (v0.1 + Phase 4 GraphQL batching + Phase 6 Code Quality).

### Added

- CLI 명령어 — `scan` (디렉토리↔repo 4-state JSON 출력) + `diff` (단일 파일 차이) + `init` (`gitless-sync.toml` stdout TOML emit).
- 분류 — 4-state (`identical`/`local_only_changed`/`remote_only_changed`/`drift`/`failed`) + 시간 비교 휴리스틱 (G-005) + Schema v1.0 (호출자 backward-compat 보장).
- Hash & Normalize — 자체 정의 SHA-1 (`SHA-1("blob <size>\0<normalized content>")`, git 표준 blob SHA 아님 — G-001) + LF normalize + UTF-8 BOM strip (`--keep-bom` 시 보존) + binary 휴리스틱 (NUL byte 8000 안).
- GitHub API — `gh` CLI subprocess 단일 통로 (ADR 0001/0002) + REST + GraphQL (alias batching, default since Phase 4 — ADR 0006) + `MockGhClient` trait inject.
- Quality Gates (Phase 6) — clippy 60/15/5 deny + panic 검출 (`unwrap_used`/`expect_used`/`panic`, tests 면제) + LOC 300 게이트 + cycle/cross-slice 0건 + 외부 도구 (cargo-public-api / machete / tarpaulin / deny / audit) + CI gate.

### Architecture decisions

- Read-only 영구 (ADR 0001) + Vertical slice + cross-slice 직접 ref 금지 + slice 안 acyclic + directional discipline (orchestrator → domain → IO) + Windows 1차 + MSRV 1.95.0 stable + `#![forbid(unsafe_code)]` + release `panic = "abort"` + `lto = "thin"` + `strip = true` + 박제 expiration (Phase 진입마다 재검토).

### Verified

- vault dogfooding 356 files / 0 drift (2026-04-29 ureq baseline).
- Phase 6 baseline: 244 tests pass (174 lib + 21 integration + 49 xtask) + tarpaulin 88.31%.

### Known limitations (v0.2에서 해소)

- 도메인 함정 8건 + 추가 함정 4건 (UTF-16 BOM/LFS pointer/Windows long path/`.gitignore` 정책).

### Excluded (영구 비목표)

- 3-way merge / 양방향 동기화 / 인터랙티브 UI / GitHub 외 호스팅 (GitLab/Bitbucket) / LFS 추적 파일 직접 fetch (pointer detect-only) / Event 기반 layer 통신 (channel/observer/actor).
