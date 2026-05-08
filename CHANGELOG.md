# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Phase 5 — 도메인 함정 정리 (in progress, 2026-05-09 ~)

준비 중 — Phase 5 ralph 자율 진행 시 박음. 8 핵심 함정 + 4 추가 함정 (BOM / LFS pointer / Windows long path / `.gitignore` 정책) 처리. 자세한 진행은 git log + `docs/ralph/implementation-plan.md` 34 task.

예상 변경:
- `schema_version`: 1.0 → 1.1 (minor — 새 필드 `mode` + `failed_reason` + `lfs_pointer` 추가, 기존 호환)
- `prepare_for_hash` 시그니처 변경 — `gitattr: &Arc<GitAttributes>` 인자 추가 (`shared::gitattributes` 모듈 박음)
- 새 의존성: `unicode-normalization`, `encoding_rs` (Mozilla)

## [0.1.0] - Phase 6 완료 시점 (2026-05-09)

> Phase 1 ~ Phase 6 누적. v0.1 + v0.2 (gh subprocess 마이그레이션) + Phase 4 (GraphQL batching) + Phase 6 (Code Quality) 박힘.

### Added

#### CLI 명령어
- `scan` — 로컬 디렉토리 ↔ GitHub repo 단방향 비교 → 4-state JSON 출력.
- `diff` — 단일 파일 차이 표시.
- `init` — `gitless-sync.toml` stdout TOML emit (사용자 redirect).

#### 분류
- 4-state classification: `identical` / `local_only_changed` / `remote_only_changed` / `drift` / `failed`.
- 시간 비교 휴리스틱 (G-005).
- Schema v1.0 박음 (호출자 backward-compat 보장).

#### Hash & Normalize
- 자체 정의 SHA-1 (`SHA-1("blob <size>\0<normalized content>")`) — git 표준 blob SHA 아님 (G-001).
- LF normalize (`\r\n` → `\n`).
- BOM strip (UTF-8, `--keep-bom` 시 보존).
- Binary 휴리스틱 (NUL byte 8000 안).

#### GitHub API
- `gh` CLI subprocess 단일 통로 (ADR 0001 + ADR 0002).
- REST backend (Trees / Blobs / Commits API).
- GraphQL backend (alias batching, default since Phase 4 — ADR 0006).
- `MockGhClient` trait inject 패턴 (테스트).

#### Quality Gates (Phase 6)
- clippy: `too_many_lines` 60 / `cognitive_complexity` 15 / `too_many_arguments` 5 deny.
- panic 검출: `unwrap_used` / `expect_used` / `panic` deny (production 코드 한정, tests 면제).
- LOC 게이트: 모든 production file ≤ 300 LOC (xtask check-line-limits).
- Cycle 게이트: slice 안 의존 그래프 acyclic + cross-slice ref 0건 (xtask check-cycles + cargo-modules).
- 외부 도구: cargo-public-api / cargo-machete / cargo-tarpaulin / cargo-deny / cargo-audit.
- CI gate: `.github/workflows/ci.yml` Windows runner.

### Architecture decisions

- **Read-only 영구** (ADR 0001) — 도구는 파일·원격 절대 수정 안 함. write 작업은 Claude Code가 `gh`로 직접.
- **Vertical slice** + cross-slice 직접 ref 금지 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차 타겟** — macOS/Linux는 부수효과로 지원, 검증은 Windows 기준.
- **MSRV 1.95.0 stable** (rust-toolchain.toml로 고정).
- **`#![forbid(unsafe_code)]`** 워크스페이스 lint.
- **release profile `panic = "abort"`** + `lto = "thin"` + `strip = true`.
- **박제 expiration**: 모든 사용자 취향 박제 항목 Phase 진입마다 재검토.

### Verified

- vault dogfooding (356 files, 0 drift, 2026-04-29 — ureq baseline).
- Phase 6 baseline: 244 tests pass (174 lib + 21 integration + 49 xtask) + tarpaulin 88.31%.

### Known limitations (Phase 5 영구 후 해소 예정)

- **도메인 함정**: NFD vs NFC, 대소문자 충돌, 비-UTF-8 인코딩, submodule, symlink, 빈 파일 실파일 검증, 실행 권한, `.gitattributes`. v0.2 (Phase 5)에서 박음.
- **추가 함정** (clean-context 보강): UTF-16 BOM, git LFS pointer, Windows long path / 예약 파일명, `.gitignore` 무시 정책 명시.

### Excluded (영구 비목표)

- 3-way merge / 양방향 동기화.
- 인터랙티브 UI.
- GitHub 외 호스팅 (GitLab, Bitbucket).
- LFS 추적 파일 직접 fetch (pointer detect-only로 박음 — fetch는 호출자 책임).
- Event 기반 layer 통신 (channel/observer/actor) — yagni 영구 제외 (Phase 6 vague).
