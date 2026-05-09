# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

TBD — Phase 7+ 진입 시점 추가.

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
