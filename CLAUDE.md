# gitless-sync

## Project Overview
git이 없는 로컬 디렉토리를 GitHub repo와 단방향으로 비교해, 드리프트를 정량적으로 보고하는 read-only AI 친화 CLI. iCloud 동기화 디렉토리처럼 git 사용 자체가 불가능한 환경에서 "평행우주 드리프트"를 막기 위한 도구. 도구는 사실(4분류 JSON)만 제공하고 결정은 호출자(사람 또는 AI)에게 맡긴다.

## Key Constraints
- **OS**: Windows 1차 타겟. macOS/Linux는 부수효과로 지원하되 검증은 Windows 기준.
- **Rust**: stable 채널, MSRV 1.95.0. `rust-toolchain.toml`로 고정.
- **HTTP**: blocking `ureq`. async 도입은 명시적 요구 발생 시까지 보류.
- **Safety**: `#![forbid(unsafe_code)]` 워크스페이스 lint. release profile `panic = "abort"`.
- **Cargo.lock**: binary CLI이므로 commit 대상.
- **Test coverage**: Unit test 라인 커버리지 ≥ 80% (cargo-tarpaulin LLVM 백엔드). 합의된 강제 조건.
- **Read-only**: 도구는 파일·원격을 절대 수정하지 않는다. write 책임은 별도 도구 (Phase 3 `gitless-push`)로 분리.

## Architecture
**Vertical slice — 명령어 단위 자체 모듈.** `shared/`는 여러 명령어가 동일 로직 사용하는 진짜 공통만 들어간다.

```
crates/gitless-sync/src/
├── main.rs                # CLI 인자 파싱, 명령어 디스패치 (clap)
├── commands/
│   ├── scan/              # scan 명령어 자체 모듈
│   │   ├── mod.rs         # entry point + ScanArgs
│   │   ├── github.rs      # Trees / Blobs / Commits API (ureq)
│   │   ├── walker.rs      # 로컬 디렉토리 walk (walkdir + ignore)
│   │   ├── compare.rs     # 4분류 판정 + Status enum + FileEntry
│   │   └── output.rs      # ScanReport JSON 직렬화
│   └── diff/              # diff 명령어 자체 모듈
│       └── mod.rs
└── shared/                # 진짜 공통
    ├── hash.rs            # LF-normalized blob hash (자체 정의 SHA)
    ├── normalize.rs       # LF normalize, BOM 처리, binary 휴리스틱
    ├── ignore.rs          # .gitignore + builtin + --ignore 합집합
    ├── error.rs           # GitlessError enum (thiserror) + exit code 매핑
    └── config.rs          # gitless-sync.toml + env 로드
```

모듈 가시성은 `pub(crate)`로 슬라이스 경계 강제. 명령어 추가 시 다른 명령어 코드를 건드리지 않는다.

## Ralph Workflow
이 프로젝트는 Ralph Wiggum Technique으로 자율 개발한다.
- `docs/ralph/prompt-plan.md` — Planning 모드 (build 들어가기 전 implementation-plan 갱신)
- `docs/ralph/prompt-build.md` — Building 모드 (iteration당 task 하나)
- `docs/ralph/project-ops.md` — 빌드/테스트/검증 명령어
- `docs/ralph/guardrails.md` — 실패 패턴 누적
- `docs/ralph/implementation-plan.md` — 작업 목록 (별도 세션에서 사람이 작성, plan 모드 스킵)
- `docs/specs/*.md` — 주제별 요구사항 명세

## File Locations
- 워크스페이스 루트: `Cargo.toml` (members = `crates/gitless-sync`)
- 바이너리 크레이트: `crates/gitless-sync/`
- src: `crates/gitless-sync/src/`
- toolchain 고정: `rust-toolchain.toml`
- 빌드 산출물: `target/` (gitignore)

## Critical Rules

### 도구 본성
- **Read-only.** 어떤 task든 파일 쓰기·원격 변경을 도입해서는 안 된다.
- **사실만 제공.** 도구는 결정을 내리지 않는다. AI/사람이 결과를 보고 다음 액션을 결정.
- **임의 디렉토리 + 임의 GitHub repo 간 비교.** vault 같은 특정 도메인 종속 금지.

### 비목표 (v0.1)
3-way merge / 양방향 동기화 / 인터랙티브 UI / GitHub 외 호스팅 / LFS / 도메인 함정(NFD vs NFC, 대소문자 충돌, 비-UTF-8 인코딩, submodule, 심볼릭 링크, 실행 권한, `.gitattributes` 파싱) — Phase 5에서 다룰 것. 자세한 백로그는 `docs/roadmap.md`.

### 검증된 함정
- **`tarpaulin`은 Windows 지원됨** (LLVM 백엔드 `--engine llvm`). 페르소나 패널이 "미지원"이라 단언해도 곧이듣지 말 것 (2026-04-27 fact check 결과).
- **`local_sha`/`remote_sha`는 git 표준 blob SHA가 아닌 자체 정의 해시.** 정의는 `docs/specs/spec-hash-and-normalize.md` 참조. GitHub UI나 `git hash-object`로 얻는 SHA와 다를 수 있다.
- **`mockito` (blocking HTTP mock)** 사용. `wiremock`은 async 전제이므로 ureq와 부정합.

### 사용자 취향 결정 (검증·토론 대상 X)
- Vertical slice 아키텍처 (명령어 단위 자체 모듈, `shared/`는 진짜 공통만)
- Unit test coverage ≥ 80% (tarpaulin 라인) — 작은 CLI라도 의식적 채택.

### 메모리 환경
이 프로젝트는 obsidian vault(`C:\Users\admin\iCloudDrive\iCloud~md~obsidian`)와 별개의 auto memory 폴더를 사용한다. vault에 쌓인 사용자 컨텍스트(프로필·재무·자기성찰 등)는 여기서 자동 로드되지 않는다. 정상 동작이며, 글로벌 `~/.claude/CLAUDE.md`(Monday 페르소나 + Universal Rules)만 양쪽에서 공통이다.
