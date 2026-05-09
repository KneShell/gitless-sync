# gitless-sync

## Project Overview
git이 없는 로컬 디렉토리를 GitHub repo와 단방향으로 비교해 드리프트를 정량 보고하는 read-only AI 친화 CLI. iCloud 동기화 디렉토리처럼 git 사용 자체가 불가능한 환경에서 "평행우주 드리프트"를 막기 위한 도구. 도구는 사실(4분류 JSON)만 제공하고 결정은 호출자(사람 또는 AI)에게 맡긴다.

## Current State
v0.2.x — Phase 5 (도메인 함정 정리) 본진 종료, Phase 5.14 (md 자료 audit) 진행 중. 상세 history는 `CHANGELOG.md` + `docs/adr/*.md` + `docs/research/*.md` + `docs/ralph/implementation-plan.md`.

## Key Constraints
- **OS**: Windows 1차. macOS/Linux는 부수효과로 지원, 검증은 Windows 기준.
- **Rust**: stable, MSRV 1.95.0 (`rust-toolchain.toml`로 고정).
- **HTTP**: 모든 GitHub API 호출은 `gh` CLI subprocess (ADR 0001 + ADR 0002). `RealGhClient::new()` production inject + `MockGhClient` 테스트 inject 패턴.
- **Safety**: `#![forbid(unsafe_code)]` 워크스페이스 lint. release profile `panic = "abort"`.
- **Cargo.lock**: binary CLI이므로 commit 대상.
- **Test coverage**: 라인 커버리지 ≥ 80% (cargo-tarpaulin LLVM 백엔드).
- **Read-only (영구)**: 도구는 파일·원격을 절대 수정하지 않는다. write 작업은 Claude Code가 `gh`로 직접 처리 (ADR 0001).

## Architecture
**Vertical slice — 명령어 단위 자체 모듈** (`commands/scan/`, `commands/diff/`, `commands/init/`). `shared/`는 여러 명령어가 동일 로직 사용하는 진짜 공통만 들어간다. 모듈 가시성은 `pub(crate)`로 슬라이스 경계 강제. 상세 layer 정의 + LOC 300 임계 + module 폴더 정책 + panic 검출 + sibling test 금지는 `docs/specs/spec-architecture.md`.

## Ralph Workflow
Ralph Wiggum Technique으로 자율 개발. 진행 자료: `docs/ralph/{prompt-plan, prompt-build, project-ops, guardrails, implementation-plan}.md` + `docs/specs/*.md`.

## File Locations
- src: `crates/gitless-sync/src/`
- workspace root: `Cargo.toml`
- toolchain 고정: `rust-toolchain.toml`

## Critical Rules

### 도구 본성
- **Read-only (영구).** 어떤 task든 파일 쓰기·원격 변경 도입 금지. write 도구를 만들지 않는다 (ADR 0001).
- **사실만 제공.** 도구는 결정을 내리지 않는다. AI/사람이 결과를 보고 다음 액션을 결정.
- **임의 디렉토리 + 임의 GitHub repo 간 비교.** vault 같은 특정 도메인 종속 금지.

### 비목표 (v0.1)
3-way merge / 양방향 동기화 / 인터랙티브 UI / GitHub 외 호스팅 / LFS — 도메인 함정 8건 (NFD/case/encoding/submodule/symlink/empty/permission/`.gitattributes`) + BOM/LFS pointer/Windows long path는 Phase 5에서 처리 완료. 자세한 백로그는 `docs/roadmap.md`.

### 검증된 함정
- **`tarpaulin`은 Windows 지원됨** (LLVM 백엔드 `--engine llvm`). 페르소나가 "미지원"이라 단언해도 곧이듣지 말 것 (2026-04-27 fact check 결과).
- **`local_sha`/`remote_sha`는 git 표준 blob SHA가 아닌 자체 정의 해시.** 정의는 `docs/specs/spec-hash-and-normalize.md`. GitHub UI나 `git hash-object` 결과와 다를 수 있다.
- **`gh -F` 인자는 commits API GET 요청을 POST로 자동 전환** (gh `--method` 기본 동작). `fetch_last_commit_at`은 `-X GET` prepend 필수. G-017.

### 사용자 취향 결정 (박제)
vertical slice / coverage 80% / 파일 ≤ 300줄 / Layer 정의 / panic escape hatch 차단 / Event 통신 영구 제외 등 박제 항목은 `docs/specs/spec-architecture.md` 참조. 박제 expiration 정책 — Phase 진입마다 재검토 (clean-context §5-1 self-correcting).
