# Implementation Plan

## Status

- Phase 11 진행 중 (2026-05-13~)
- Tasks: 4 (Phase 11)
- Completed: 1 / 4

## Notes for Build Mode

- ralph build mode는 첫 미완료 task (`[ ]`)부터 처리. 의존 순서가 본 plan에 명시 안 됐으면 acceptance + spec 본문에 잠재 의존 명시 (e.g., "X task 결과 위에서 진행").
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Hard gate (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출 lint) 모두 deny active 유지. 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (`project-ops.md`). 신규 task의 acceptance에 unit test 포함.

## Active Phase

### Phase 11 (2026-05-13~) — GitHub Release Distribution 인프라

목표: tag push 시 cross-platform portable binary artifact 자동 생성/업로드. 실제 dry-run 트리거 및 첫 release는 별도 phase에서 진행 (사용자 정책 "release 직전 phase = 검증 finding 자동 신규 phase chain" 적용 대상).

배경: 현재 사용자는 소스 clone + `cargo build --release`로 binary를 직접 만들어야 한다. Rust toolchain 강제 설치 비용이 사용성 (iCloud 디렉토리 drift 점검) 과 맞지 않음. G-020 (release Cargo.toml version bump 가드) 도입 흐름의 자연스러운 연장.

Spec audit (Phase 11 진입 전 사전 조사): `docs/specs/` 10개 spec 모두 active. v0.1 ureq baseline 표현은 ADR 0002로 이미 제거, v0.1 stub 표현은 ADR 0006으로 이미 제거. 별도 cleanup task 없음.

Task 단위 분할 (1 task = 1 commit, prompt-build.md § 4 정합).

#### T1 — `docs/specs/spec-release-distribution.md` 신규 작성

- [x] **T1** spec 신규 작성
  - **Files**: `docs/specs/spec-release-distribution.md` (신규)
  - **Type**: Spec-only (prompt-build.md § 2 코드 룰 적용 제외, G-012 spec-only 케이스로 tarpaulin baseline 유지 자동 통과)
  - **내용**:
    - Target matrix 3개:
      - `x86_64-pc-windows-msvc` — 1차 타겟 (CLAUDE.md 정합)
      - `x86_64-unknown-linux-musl` — 정적 링크, glibc 버전 비의존
      - `aarch64-apple-darwin` — Apple Silicon. macOS는 CLAUDE.md 정책상 build only, test 안 함
    - Archive 포맷: Windows `.zip`, Linux/macOS `.tar.gz`
    - Asset 명명: `gitless-sync-v{VERSION}-{TARGET}.{EXT}` (예: `gitless-sync-v0.7.0-x86_64-pc-windows-msvc.zip`)
    - Checksum: `sha256sums.txt` 단일 manifest + asset별 `*.sha256` 동봉. 사용자 검증 명령 예시 (`Get-FileHash` / `sha256sum`).
    - Attestation: `actions/attest-build-provenance` (SLSA build provenance). 사용자 검증: `gh attestation verify <binary> --repo <owner>/gitless-sync`.
    - Trigger: `push: tags: ['v*']` + `workflow_dispatch` (input `dry_run: bool` — true이면 GitHub Release 미생성, Actions artifact로만 산출).
    - Cross-compile 도구: `taiki-e/upload-rust-binary-action` (musl/macOS cross-compile + archive + checksum + upload 내장. hand-rolled matrix 대비 YAML 압축).
    - Version 정합 (G-020 cross-link): Cargo.toml `version` 필드와 tag SemVer 일치 검증. mismatch 시 workflow fail-fast.
    - Read-only 보장 (ADR 0001 정합): release workflow는 repo 코드 수정 금지. asset 산출만.
  - **Acceptance**:
    - spec 본문이 위 항목을 모두 다룬다.
    - ADR 0001 cross-link 1건 (read-only 보장 근거).
    - G-020 cross-link 1건 (version 정합 근거).
    - 기존 `docs/specs/spec-*.md` template 일관성 따른다 (헤더 구조, 섹션 순서, 코드 fence + 표 사용 패턴). 참조 우선순위: `spec-cli-interface.md` 또는 `spec-output-schema.md` 구조.
    - YAGNI 명시 섹션 포함 (의도적 제외 항목 + 사유): macOS notarization / Windows Authenticode 서명 / 패키지 매니저 배포 (Scoop·Homebrew·winget) / `cargo install` (crates.io publish) / install one-liner 스크립트 / GitHub Actions SHA pin. 각 항목 한 줄 사유 (현재 미수요 / 별도 phase / dependabot 자동 갱신 등).
  - **의존**: 없음

#### T2 — `.github/workflows/release.yml` 신규 작성

- [ ] **T2** release workflow 신규 작성
  - **Files**: `.github/workflows/release.yml` (신규)
  - **Type**: Non-Rust, non-spec. prompt-build.md § 2 Rust 코드 룰 제외 (yml 파일). § 3 validation 7단계는 Rust 영역 무변경으로 baseline 유지 자동 통과.
  - **추가 검증**: yml syntax valid (actionlint 로컬 1회 또는 GitHub Actions parser 통과).
  - **내용**: T1 spec 그대로 구현. 핵심 step:
    - `actions/checkout@v6`
    - Rust toolchain install (1.95.0 stable)
    - `Swatinem/rust-cache@v2` (CI와 정합)
    - `taiki-e/upload-rust-binary-action@v1` (target matrix + archive + checksum)
    - `actions/attest-build-provenance@v3` (artifact attestation)
    - `dry_run` input 조건부 Release create (false → Release attach, true → Actions artifact만)
  - **Acceptance**:
    - yml syntax 검증: 작성 직후 `python -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"` 통과 (exit 0). YAML 자체 syntax만 검증, Actions schema 의미 검증은 push 후 GitHub Actions parser가 자동으로 fail-fast 처리.
    - 3-target matrix 명시
    - `dry_run` input default = false
    - Phase 11 안에서 실제 workflow 트리거 없음 (인프라 준비만, 첫 dry-run은 phase 종료 후)
    - 외부 action 버전 pin: `taiki-e/upload-rust-binary-action@v1` / `actions/attest-build-provenance@v3` / `actions/checkout@v6` 등 메이저 버전 tag pin (SHA pin은 본 phase 비목표 — dependabot 자동 갱신 정책 가정).
  - **의존**: T1 [x]

#### T3 — `README.md` install 섹션 추가

- [ ] **T3** README install 섹션 추가
  - **Files**: `README.md`
  - **Type**: Docs (Rust 소스 아님). § 2 코드 룰 제외. § 3 validation 7단계 baseline 유지 자동 통과.
  - **추가 검증**: `cargo xtask check-readme-examples` 통과 (CI gate에 이미 존재).
  - **내용**:
    - 기존 "Build" 섹션 위에 **"Install (prebuilt binary)"** 신규 섹션
    - 각 OS별 release archive download/extract 안내 (curl/Invoke-WebRequest 예시)
    - SHA256 검증 명령 예시 (`Get-FileHash -Algorithm SHA256` Windows / `sha256sum -c sha256sums.txt` Linux)
    - attestation 검증 명령 예시 (`gh attestation verify <binary> --repo <owner>/gitless-sync`)
    - "Build from source" 섹션은 그대로 보존 (개발자/CI용)
  - **Acceptance**:
    - `cargo xtask check-readme-examples` 통과 (xtask 검증기는 `xtask/src/check_readme_examples/` 실존 확인됨, T3 범위는 README 본문 수정 한정 — xtask 구현 변경 금지)
    - install 섹션이 Build 섹션 위에 위치 (사용자 정상 경로 = prebuilt 우선)
    - 검증 명령 예시 3개 (download / sha256 / attestation)
  - **의존**: T1 [x] (T2와 독립 — README는 spec의 asset 명명·검증 명령 패턴만 참조, yml 결과물에 직접 의존하지 않음. ralph 순차 실행기는 T2가 먼저 잡혀도 무방, dependency 명세는 정확성 우선)

#### T4 — Phase 11 종료 — Status 갱신 + Active → Completed + CLAUDE.md 한 줄

- [ ] **T4** Phase 11 close
  - **Files**: `docs/ralph/implementation-plan.md`, `CLAUDE.md`
  - **Type**: Spec-only (`docs/ralph/*.md` + `CLAUDE.md`도 본 task 영역으로 명시)
  - **내용**:
    - `implementation-plan.md` Status: "Phase 11 종료 (2026-05-XX), Tasks: 4 (Phase 11), Completed: 4 / 4"
    - Active Phase 섹션 → "진행 중 phase 없음" 복귀
    - Completed Phases에 한 줄 추가: `- Phase 11 (2026-05-XX) — GitHub Release distribution 인프라 (spec + workflow + README).`
    - 본 Phase 11 task 본문은 한 줄 요약으로 압축 (Phase 10 패턴과 동일)
    - `CLAUDE.md` "Project Overview" 또는 "File Locations" 인접에 binary distribution 한 줄 추가 (source build 외 release download 옵션 명시)
    - **Release notes 작성 책임 명시**: Phase 11은 release 인프라 준비만. 실제 release tag 시점의 release notes(CHANGELOG 갱신 + GitHub Release body)는 별도 phase의 tag 작업 책임. 본 phase task에 release notes 작성 없음 — implementation-plan.md Phase 11 종료 줄에 "release notes 작성 = 다음 release phase 책임" 한 줄 footnote.
  - **Acceptance**:
    - Phase 11 task 본문이 Active Phase 섹션에서 사라지고 Completed Phases에 한 줄 마일스톤으로 압축. 압축 패턴: Phase 10 close commit `f5090bf` 참조 (56 라인 → 5 라인, Status + Active 갱신 + Completed 한 줄 추가).
    - CLAUDE.md에 한 줄 추가
    - release notes 책임 footnote 포함
  - **의존**: T1 [x], T2 [x], T3 [x]

## Completed Phases

Phase 1~10 + v0.4.1 / v0.4.2 누적 history — 자세한 내용은 git log + CHANGELOG.md 참조. 핵심 마일스톤:

- Phase 5 (2026-05-09~10) — 도메인 함정 8 핵심 + 4 추가 detect/handle + schema v1.0→1.1.
- Phase 6 (2026-05-09~10) — Hard gate 활성화 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출).
- Phase 7 (2026-05-10) — vault scale + Trees sub-tree fallback + 큰 파일 임계 + schema v1.1→1.2 + v0.3.0.
- Phase 8 (2026-05-10) — LLM-as-caller eval 7 friction 해소 (F1/F2 schema v1.2→1.3 + F3 diff --json + F4~F6 clap surface + F7 CI README sanity) + v0.4.0.
- v0.4.1 (2026-05-10) — clap argument-parse contract 회복 (try_parse + CONFIG_ERROR JSON wrap).
- v0.4.2 (2026-05-11) — cosmetic identical classification fix (normalize-equal sha-differ → Identical) + schema v1.3→1.4 (ADR 0015).
- Phase 9 (2026-05-12) — vault dogfood F1/F2/F3 (scan/diff about derive + init wording 정밀화 + summary-only failed visibility) + schema v1.4→1.5 + v0.5.0.
- Phase 10 (2026-05-12) — post-v0.5.0 clean-context audit Finding 1/2/3 해소 (SemVer 면제 근거 명문화 + hash_io explicit emit + minimal entry shape 발산 강조) + schema v1.5→1.6 + v0.6.0.

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner.
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **자율 진행 회피 영역** — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 외부 시각 검토 권장.
