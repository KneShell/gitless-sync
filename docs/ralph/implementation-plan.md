# Implementation Plan

## Status
- Last updated: 2026-05-07 (Phase 2 진입 — `gitless-sync init` 명령어)
- Total tasks: 8 (P1, P2, P3, P4, P5, P6, P7, P8)
- Completed: 0 / 8

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵된 상태.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 해당 spec 파일과 정확히 매핑되어야 함. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- **모든 task는 `[AUTO]`** — 사람 개입 0으로 자율 루프 진행. Phase 2 사전 결정은 § Phase 2 사전 결정에 박힘 (P1 ADR 0004 + P2 spec 갱신으로 코드 baseline에 반영).
- **transient 실패 자동 회복**: G-015로 박힌 [!] task는 `prompt-build.md` § 1 [!] auto-recovery 룰에 따라 다음 iteration 자동 reset.

## Phase 2 사전 결정 (ralph 자율 진행 시 변경 금지)

1. **Output 모드**: stdout TOML. 도구는 파일 작성 0. 사용자가 `gitless-sync init --repo owner/name --branch main > gitless-sync.toml`로 redirect. ADR 0001 read-only 영구와 100% 일관.
2. **TOML 스키마**: v0.1 `spec-config.md` § 스키마 그대로 — `repo` / `branch` / `ignore` 3개 필드. 확장 0.
3. **Repo 존재 검증**: 0. 외부 호출 0. 잘못된 repo 박혀도 다음 `scan` 실행 시 에러로 surface.
4. **자동 감지**: 0. 모두 명시 인자 (yagni 일관).
5. **CLI 인자**:
   - `--repo owner/name` **필수**. 미명시 시 `GitlessError::Config("repo not specified")`, exit 1.
   - `--branch <name>` 옵셔널. 미명시 시 emit 안 함 (load 시 기본값 `main` fallback).
   - `--ignore <pattern>` 옵셔널 반복. 미명시 시 emit 안 함.
6. **TOML emit 순서**: repo → branch → ignore (직렬화 안정성).
7. **stderr hint**: 정상 init 실행 시 stderr에 항상 박음 — `Tip: redirect stdout to ./gitless-sync.toml to persist this config.` tty 감지 분기 0.
8. **README 보강**: § Quick Start (또는 § Usage)에 init redirect 예시 1줄 + scan 1줄.
9. **--help 보강**: clap `init` subcommand `after_help` 또는 `long_about`에 redirect 예시 1줄.
10. **dogfooding**: P8에서 init stdout → tempfile → scan 통과 검증 (M8 선례). 박제 0, commit message에 카운트만.

## Dependency Graph

```
P1 → P2 → P3 → P4 → P5 → P6 → P7 → P8
```

Linear chain. 각 task가 다음 task의 compile-clean baseline.

## Tasks

### P1. ADR 0004 박음 + CLAUDE.md Current State 갱신 `[AUTO, 문서/spec]`
- **Spec reference**: `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` § Read-only 영구 (정합 명시), `CLAUDE.md` § Current State / § Critical Rules / § 검증된 함정
- **Files**: `docs/adr/0004-init-stdout-redirect.md` (신규), `CLAUDE.md`
- **Depends on**: none
- **Acceptance criteria**:
  - `[AUTO]` `docs/adr/0004-init-stdout-redirect.md` 신규 작성:
    - **Status**: Accepted
    - **Date**: 2026-05-07
    - **Resolves**: `docs/roadmap.md` § Phase 2 init 출력 방식 미정 (사용자 결정 2026-05-07)
    - **Related**: ADR 0001 (read-only 영구), `spec-cli-interface.md`, `spec-config.md`
    - § Context: roadmap 원안은 "현재 디렉토리에 toml 파일 작성 + `--force`"였으나 ADR 0001 read-only 영구 룰 위반. 3 옵션(stdout / dry-run+--write / 파일 작성) 평가 후 stdout 채택.
    - § Decision: init은 stdout TOML 출력. 도구는 파일 작성 0. 사용자가 redirect로 영구 파일 생성.
    - § Consequences: `--force` / `--write` / 충돌 처리 코드 0. CLAUDE.md/ADR 0001 룰 갱신 0. README + --help + stderr hint 보강 필요 (P6에서 처리).
    - § References: ADR 0001, `spec-cli-interface.md`, sample CLI 패턴(`gh api > out.json`).
  - `[AUTO]` `CLAUDE.md` § Current State 갱신: "Phase 2 진행 중 — `gitless-sync init` (ADR 0004 stdout redirect)" 한 줄 추가.
  - `[AUTO]` `CLAUDE.md` § 사용자 취향 결정 (검증·토론 대상 X) section에 "init은 도구가 파일 작성 안 함, stdout TOML + redirect 패턴 (ADR 0004)" 한 줄 박음.
- **Status**: `[ ]`

### P2. spec 갱신 — Phase 2 init 정의 `[AUTO, spec-only]`
- **Spec reference**: `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md`, `docs/specs/spec-error-contracts.md`, `docs/roadmap.md` § Phase 2
- **Files**: `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md`, `docs/specs/spec-error-contracts.md`, `docs/roadmap.md`
- **Depends on**: P1
- **Acceptance criteria**:
  - `[AUTO]` `spec-cli-interface.md`: § init subcommand 신규 추가 — clap 정의 + `--repo` 필수 + `--branch <name>` 옵셔널 + `--ignore <pattern>` 옵셔널 반복 + exit code 매트릭스 + stdout TOML 출력 + redirect 패턴 명시. § Acceptance Criteria에 init 케이스 추가.
  - `[AUTO]` `spec-config.md`: 본문에 "`gitless-sync init`이 본 스키마를 stdout TOML로 emit하는 도구"임을 한 줄 cross-ref 추가. 스키마 본체는 그대로.
  - `[AUTO]` `spec-error-contracts.md`: § init 에러 케이스 추가 — `--repo` 미명시 → `GitlessError::Config("repo not specified")`, exit 1 + stderr `error_code: "CONFIG"`. Acceptance Criteria에 PRD 시나리오 17 (init repo 미명시) 추가.
  - `[AUTO]` `roadmap.md` § Phase 2 갱신: 원안 "현재 디렉토리에 `gitless-sync.toml` 작성. 기존 파일 있으면 `--force` 없이 실패."를 "stdout TOML 출력 — 사용자가 `gitless-sync init ... > gitless-sync.toml`로 redirect (ADR 0004). 도구 파일 작성 0."으로 갱신. 실패 모드 `--force` / 파일 권한 / 기존 파일 충돌 항목 모두 제거 (도구 파일 작성 0이라 obsolete).
- **Status**: `[ ]`

### P3. init mod 신규 + CLI 디스패치 + TOML 직렬화 + 단위 테스트 매트릭스 `[AUTO, 코드]`
- **Spec reference**: `spec-cli-interface.md` § init subcommand (P2 갱신본), `spec-config.md` § 스키마
- **Files**: `crates/gitless-sync/src/commands/init/mod.rs` (신규), `crates/gitless-sync/src/commands/mod.rs` (init 모듈 노출 — `pub(crate) mod init;`), `crates/gitless-sync/src/main.rs` (clap subcommand + 디스패치), `crates/gitless-sync/src/lib.rs` (init 모듈 export — 통합 테스트가 진입점 호출), `crates/gitless-sync/src/shared/error.rs`, `crates/gitless-sync/src/shared/hash.rs`, `crates/gitless-sync/src/shared/normalize.rs`, `crates/gitless-sync/src/commands/scan/compare.rs`, `crates/gitless-sync/src/commands/scan/output.rs` (lib export로 surface된 pedantic clippy `must_use` / `# Errors` / `# Panics` 동반 정리 — v0.2 M4a Files 확장 선례, 발생 시만 수정)
- **Depends on**: P2
- **Acceptance criteria**:
  - `[AUTO]` `commands/init/mod.rs` 신규:
    - `pub(crate) struct InitArgs { repo: String, branch: Option<String>, ignore: Vec<String> }` (clap derive 또는 main.rs에서 build).
    - `pub(crate) fn run(args: &InitArgs, writer: &mut impl std::io::Write) -> Result<(), GitlessError>` 시그니처.
    - 본체: emit 순서 repo → branch → ignore. 옵셔널 필드는 `Some` / non-empty 시에만 emit.
    - `repo` emit: `format!("repo = \"{}\"\n", repo)`.
    - `branch` emit: `format!("branch = \"{}\"\n", branch)`.
    - `ignore` emit: `format!("ignore = [{}]\n", ...)` 형식. 패턴 각각 `"..."`로 quote, comma+space로 join.
  - `[AUTO]` `commands/mod.rs`에 `pub(crate) mod init;` 추가.
  - `[AUTO]` `main.rs`: clap subcommand `Init(InitArgs)` 추가 (scan/diff 옆). dispatch 시 `commands::init::run(&args, &mut std::io::stdout().lock())` 호출.
  - `[AUTO]` `lib.rs`: `pub mod commands;` 또는 `commands::init` 가시성 정렬 (통합 테스트 진입점 호출 가능하도록).
  - `[AUTO]` 단위 테스트 매트릭스 (`commands/init/mod.rs::tests`, `Vec<u8>` writer inject):
    - 정상 (repo only): `--repo a/b` → `"repo = \"a/b\"\n"`
    - repo + branch: `--repo a/b --branch dev` → 두 줄
    - repo + ignore (1개): `--repo a/b --ignore "*.tmp"` → repo 줄 + `ignore = ["*.tmp"]` 줄
    - repo + ignore (2개): `--ignore "dist/" --ignore "*.tmp"` → `ignore = ["dist/", "*.tmp"]`
    - 모든 필드: repo + branch + ignore (다중)
    - **round-trip 검증**: emit된 TOML이 `toml::from_str::<Config>` 파싱 통과 + 모든 필드 일치.
  - `[AUTO]` **emit 형식 baseline**: v0.1 `shared/config.rs::Config` struct round-trip 통과 기준 — 충돌 시 emit 측 조정 (Config struct는 baseline, 변경 금지).
  - `[AUTO]` **lib export cascade 정리**: init mod export로 surface된 pedantic clippy warning(`must_use` / `# Errors` / `# Panics` 등)은 본 task Files 영역 안에서 동반 정리. 영역 초과 시 [!] + 사람이 plan Files 확장 결정 후 reset (ralph 자율 회복 0 — § Constraints 영역 룰).
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` 통과.
- **Status**: `[ ]`

### P4. 에러 매핑 (--repo 미명시) + stderr hint 박음 `[AUTO, 코드]`
- **Spec reference**: `spec-error-contracts.md` § init 에러 케이스 (P2 갱신본)
- **Files**: `crates/gitless-sync/src/commands/init/mod.rs`, `crates/gitless-sync/src/main.rs`
- **Depends on**: P3
- **Acceptance criteria**:
  - `[AUTO]` `--repo` 미명시 검증: clap `required = true`로 박거나 `run` 진입부에서 `args.repo.is_empty()` 검사 후 `GitlessError::Config("repo not specified")` 반환. exit 1 + stderr `error_code: "CONFIG"` 매핑은 기존 `main.rs` 에러 핸들러가 처리.
  - `[AUTO]` 정상 종료 시 stderr hint 1줄 박음: `Tip: redirect stdout to ./gitless-sync.toml to persist this config.`
    - 박는 위치: `commands::init::run` 정상 종료 직전 또는 `main.rs` Init 분기 정상 경로. tty 감지 분기 0 — 항상 박음.
    - eprintln! 또는 stderr writer inject (테스트 용이성 위해 inject 권장).
  - `[AUTO]` 단위 테스트:
    - `--repo` 미명시 → `Err(GitlessError::Config(_))` 매칭.
    - stderr hint는 inject 패턴이라면 unit test에서 capture, 그렇지 않으면 P5 통합 테스트에서 검증.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` 통과.
- **Status**: `[ ]`

### P5. 통합 테스트 — init 시나리오 16~19 `[AUTO, 코드]`
- **Spec reference**: `spec-cli-interface.md` § init subcommand Acceptance Criteria, `spec-error-contracts.md` PRD 시나리오 17
- **Files**: `crates/gitless-sync/tests/integration.rs`
- **Depends on**: P4
- **Acceptance criteria**:
  - `[AUTO]` **테스트 패턴**: library entry(`commands::init::run` / `commands::scan::run_with_client`) 직접 호출 + writer / MockGhClient inject. `cargo run --` 자식 프로세스 호출은 P8 dogfooding 한정 — Windows + PowerShell EOL/encoding 잡음 회피. exit code 검증은 `err.exit_code()` 메서드로 대체.
  - `[AUTO]` 시나리오 16 (init 정상 emit): `commands::init::run(&InitArgs { repo, branch, ignore }, &mut Vec<u8>)` → `Ok(())` + Vec capture → `toml::from_str::<Config>` 파싱 통과 + repo/branch/ignore 모든 필드 일치.
  - `[AUTO]` 시나리오 17 (init repo 미명시): `InitArgs { repo: "".into(), .. }` → `Err(GitlessError::Config(_))` + `err.exit_code() == 1` + `err.error_code() == "CONFIG"` + 에러 메시지에 "repo not specified" substring.
  - `[AUTO]` 시나리오 18 (init stderr hint): 정상 init 실행 시 stderr writer inject로 capture → `redirect stdout` substring 포함.
  - `[AUTO]` 시나리오 19 (init → scan 라운드트립): init writer로 TOML capture → tempdir에 작성 → 같은 tempdir 기반 ScanArgs build → `commands::scan::run_with_client(&args, &MockGhClient stub)` 호출 → toml에서 repo/branch 자동 로드 + scan 정상 동작 확인 (MockGhClient stub 응답 정상 시).
  - `[AUTO]` **escalation**: 시나리오 19 단독 실패 + 16~18 통과 시 P5 통째 [!] + 사람이 P5 분할(P5a/P5b) 또는 19를 별도 escalation task로 분리 결정. ralph 자율 회복 0 (라운드트립 정합 충돌 가능성 — 영구 신호).
  - `[AUTO]` `cargo test --test integration` 전체 통과.
- **Status**: `[ ]`

### P6. README "Quick Start" + --help 갱신 `[AUTO, 문서]`
- **Spec reference**: ADR 0004 § Consequences (README + --help 보강 필요)
- **Files**: `README.md`, `crates/gitless-sync/src/main.rs` (clap doc string)
- **Depends on**: P5
- **Acceptance criteria**:
  - `[AUTO]` `README.md` § Quick Start 또는 § Usage에 init redirect 예시 추가:
    ```bash
    # Generate config file once per directory:
    gitless-sync init --repo owner/name --branch main > gitless-sync.toml

    # Then scan repeatedly without flags:
    gitless-sync scan
    ```
  - `[AUTO]` `main.rs` clap `Init` subcommand에 `after_help` 또는 `long_about` 박음. 마지막 줄에 redirect 예시 1줄 — `gitless-sync init --help` 출력 마지막에 노출.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` 통과.
- **Status**: `[ ]`

### P7. coverage 게이트 통과 검증 (phase-final) `[AUTO]`
- **Spec reference**: `docs/ralph/project-ops.md` § Coverage, `CLAUDE.md` § Test coverage, G-007, G-012, G-013
- **Files**: 미달 모듈에 unit test 추가 (필요 시), `deny.toml` (신규 의존성 도입 시)
- **Depends on**: P6
- **Acceptance criteria**:
  - `[AUTO]` `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo audit` 모두 통과.
  - `[AUTO]` `cargo tarpaulin --engine llvm --workspace --out Stdout` 라인 커버리지 ≥ 80%.
  - `[AUTO]` `cargo tree`로 신규 의존성 점검. Phase 2가 `toml` crate 신규 도입 시 deny.toml 라이선스 화이트리스트 갱신 + `cargo deny check` 재통과 확인. (`toml`이 이미 transitive로 박혀 있으면 추가 0.)
- **Status**: `[ ]`

### P8. dogfooding contract step `[AUTO]`
- **Spec reference**: ADR 0004 § Consequences, M8 dogfooding 선례
- **Files**: 박제 0. 실행 결과는 task `[x]` commit message에 카운트만 인라인.
- **Depends on**: P7
- **Acceptance criteria**:
  - `[AUTO]` **진입 사전 점검**: `gh auth status` exit 0 확인. 실패(인증 만료 / scope 부족) 시 즉시 [!] + 명시 메시지 ("P8 dogfooding requires `gh auth status` exit 0 — run `gh auth refresh -s repo` or `gh auth login`"). G-015 영구 신호 — auto-recovery 대상 아님, 사람 대기.
  - `[AUTO]` ralph 환경에서 release 빌드: `cargo build --release` exit 0.
  - `[AUTO]` `cargo run --release -- init --repo KneShell/gitless-sync --branch main` 실행 → stdout 캡처 → tempdir의 `gitless-sync.toml`로 작성. exit 0.
  - `[AUTO]` 작성된 toml이 `toml::from_str::<Config>` 파싱 통과 + repo/branch 필드 일치.
  - `[AUTO]` 같은 tempdir 또는 `--local D:\00.Projects\02.Personal\05.gitless-sync`로 `cargo run --release -- scan` 실행 → exit 0 + stdout JSON 파싱 통과 + summary 5 카운트(`identical`/`local_only_changed`/`remote_only_changed`/`drift`/`failed`) invariant 일치 (M8 게이트).
  - `[AUTO]` external command transient(network 5xx, gh exit≠0)는 G-015 retry policy 적용. 3회 실패 시 [!] + G-015 reference (auto-recovery 가능).
  - `[AUTO]` 박제 0. git log + commit message가 evidence trail. failed 비율은 commit message에 기록만 (BLOCKED 게이트 아님).
- **Status**: `[ ]`
