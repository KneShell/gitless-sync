# Spec: Release Distribution

## 목적

`gitless-sync`를 사용하기 위해 Rust toolchain 설치 + 소스 clone + `cargo build --release` 흐름이 필수인 현 상태를, **tag push 자동 트리거 → cross-platform portable binary artifact 산출 → GitHub Release attach** 흐름으로 대체한다. 사용 케이스(iCloud 동기화 디렉토리 drift 점검)와 toolchain 강제 설치 비용이 어울리지 않음 — 사용자가 archive 한 개 다운로드 + 압축 해제 + 실행으로 도구를 쓸 수 있어야 한다.

본 spec은 release 배포 인프라(workflow + asset 정책 + 검증 흐름)만 정의한다. **실제 release tag 작업·release notes 작성·CHANGELOG 갱신은 별도 release phase 책임** (본 phase 비목표). 첫 dry-run 트리거도 본 phase 종료 후 별도 phase로 분리.

**CHANGELOG / release notes 톤 가이드라인**: 사용자(배포 받는 측) 시점 변화만 — Added / Changed / Fixed / Removed bullet, 한두 줄. Phase 내 task별 detail · finding 사이클 · 설계 의도는 `docs/ralph/implementation-plan.md` 와 git log 영역. 즉 CHANGELOG는 "받는 사람이 알아야 할 결과", git log는 "어떻게 거기 도달했는지" — 둘이 같은 영역 침범 금지. 이전 v0.6.0 entry는 과도기 패턴이라 그대로 보존, v0.7.0부터 본 가이드라인 적용.

## 현재 상태

- Release artifact 0건. 사용자는 소스 clone + `cargo build --release` 직접 실행이 유일한 설치 경로.
- `.github/workflows/ci.yml` 단일 workflow 존재 (gates: fmt/clippy/test/coverage/public-api). release 흐름 부재.
- `crates/gitless-sync/Cargo.toml::package.version` = `0.6.0` (Phase 10 catch-up commit `db40612` 시점 정합).
- Cargo.toml version bump 누락 가드는 G-020 (release tag task 본문에 Cargo.toml bump step 명시 룰). 본 spec은 workflow 측 정합 검증 layer로 G-020을 보강한다.

## 작업 범위

### Target matrix

| Target triple | Archive | OS runner | 검증 수준 |
|---|---|---|---|
| `x86_64-pc-windows-msvc` | `.zip` | `windows-latest` | full (build + smoke run) |
| `x86_64-unknown-linux-musl` | `.tar.gz` | `ubuntu-latest` | full (build + smoke run, musl static link) |
| `aarch64-apple-darwin` | `.tar.gz` | `macos-latest` | build-only (CLAUDE.md 정책: macOS는 build 검증만, runtime test 없음) |

3 target 모두 한 release tag push에서 matrix job으로 병렬 산출. Linux는 musl 정적 링크로 glibc 버전 비의존 (사용자 환경 다양성 흡수). macOS는 Apple Silicon 단일 (Intel `x86_64-apple-darwin`은 비목표 — Apple 자체 deprecation 흐름).

### Trigger

- `push` event + `tags: ['v*']` filter — semver tag (`v0.7.0` 등) push 시 자동 실행.
- `workflow_dispatch` — manual trigger. input 1개:
  - `dry_run: boolean` (default `false`). `true`면 GitHub Release 미생성, Actions artifact로만 산출 (검증용).

본 spec 시점 기준 default branch trigger·schedule trigger·PR trigger 모두 비대상 (release artifact는 tag 시점에만 의미).

### Archive 포맷 + Asset 명명

명명 규칙: `gitless-sync-{TAG}-{TARGET}.{EXT}`

| Tag | Target | Filename |
|---|---|---|
| `v0.7.0` | `x86_64-pc-windows-msvc` | `gitless-sync-v0.7.0-x86_64-pc-windows-msvc.zip` |
| `v0.7.0` | `x86_64-unknown-linux-musl` | `gitless-sync-v0.7.0-x86_64-unknown-linux-musl.tar.gz` |
| `v0.7.0` | `aarch64-apple-darwin` | `gitless-sync-v0.7.0-aarch64-apple-darwin.tar.gz` |

Archive 내부 layout (모든 target 동일):

```
gitless-sync-v0.7.0-{TARGET}/
├── gitless-sync(.exe)     # 실행 파일 (Windows는 .exe)
├── README.md              # repo root README 사본
├── LICENSE                # repo root LICENSE 사본
└── CHANGELOG.md           # repo root CHANGELOG 사본
```

### Cross-compile 도구

`taiki-e/upload-rust-binary-action@v1` 채택. 채택 근거:

- target triple 별 cross-compile (musl/macOS) + archive + checksum + GitHub Release upload를 단일 step에 캡슐화. matrix YAML 압축.
- linux musl target에 대해 `cross` / `cargo-zigbuild` 등 toolchain 자동 선택 내장. hand-rolled matrix 대비 maintenance cost ↓.
- semver tag pin (`@v1`). SHA pin은 본 spec 비목표 (§ YAGNI 참조).

대안 비교 — `actions-rs/release` (deprecated, 미채택) / hand-rolled `cargo build --target` + `tar`/`zip` matrix (검증 부담 ↑, 미채택).

### Checksum

각 archive 산출 직후 SHA256 hash 계산. 두 형태 동시 emit:

1. **Per-asset checksum file**: `{archive}.sha256` (예: `gitless-sync-v0.7.0-x86_64-pc-windows-msvc.zip.sha256`). 한 줄 형식 `<hash>  <filename>` (GNU coreutils 호환).
2. **Aggregate manifest**: `sha256sums.txt` (모든 archive checksum 한 파일에 누적, 한 줄 = 한 archive). `sha256sum -c sha256sums.txt` 단일 명령으로 전체 검증 가능.

사용자 검증 명령 (README 본문에 동일 예시 emit 예정 — T3 scope):

```powershell
# Windows PowerShell
Get-FileHash -Algorithm SHA256 gitless-sync-v0.7.0-x86_64-pc-windows-msvc.zip
# 출력의 Hash 값을 .sha256 파일 첫 컬럼과 byte 비교.
```

```bash
# Linux/macOS
sha256sum -c sha256sums.txt
# 또는 단일 파일:
sha256sum gitless-sync-v0.7.0-x86_64-unknown-linux-musl.tar.gz
```

### Attestation

`actions/attest-build-provenance@v3` step으로 archive별 SLSA build provenance 발급. 발급 후 GitHub OIDC 기반 attestation은 repo 단위 verify 가능.

사용자 검증 명령:

```bash
gh attestation verify gitless-sync-v0.7.0-x86_64-pc-windows-msvc.zip \
    --repo <owner>/gitless-sync
```

verify 성공 = archive가 본 repo의 release workflow에서 산출된 binary임을 SLSA 기준으로 입증. 빌드 supply chain compromise 탐지의 baseline.

### Version 정합 (G-020 보강)

`G-020` (`docs/ralph/guardrails.md`)은 release tag task 본문에 Cargo.toml version bump step을 명시하라는 룰 — 사람·ralph 측 절차 가드. 본 spec은 workflow 측 sanity check layer로 G-020을 보강한다.

워크플로우는 빌드 전 다음 검증을 수행:

1. `$GITHUB_REF_NAME` (예: `v0.7.0`)에서 leading `v` strip → `0.7.0`.
2. `crates/gitless-sync/Cargo.toml`의 `[package]` 섹션 `version` 필드 추출 (e.g., `cargo metadata --format-version 1 --no-deps`를 jq filter `.packages[0].version`으로 파싱하거나, 동등한 grep/sed 방식).
3. 두 값 byte 비교. mismatch 시 즉시 `exit 1`로 workflow fail-fast — build/upload step 진입 안 함.

이 검증은 G-020 사례 (v0.5.0/v0.6.0 tag target Cargo.toml = `0.4.2` 그대로) 재발 방지의 마지막 안전망. 사람이 G-020 룰을 까먹고 tag push 해도 workflow가 거부.

### dry_run behavior

`workflow_dispatch` input `dry_run`의 두 경로 명시:

| Step | `dry_run=false` (default, tag push 포함) | `dry_run=true` (manual only) |
|---|---|---|
| Version 정합 (Cargo.toml ↔ tag) | 수행 | tag ref면 수행, non-tag ref면 **warning + skip** (임의 ref 허용) |
| GitHub Release object 생성 (`gh release create`) | **수행** (build matrix 전 단일 job — taiki-e는 attach만 함, create는 caller 책임) | **skip** (Release 미생성 경로) |
| Cross-compile build (3 target) | 수행 | 수행 |
| Archive + checksum 생성 | 수행 | 수행 |
| Attestation 발급 | 수행 | **수행** (attestation 자체 검증도 dry-run 대상) |
| Asset attach (taiki-e action `gh release upload`) | 수행 | **skip** |
| Actions artifact upload (`actions/upload-artifact@v4`) | skip (Release attach이 1차 산출처) | **수행** (Release 미생성 시 archive 회수 경로) |

핵심 원칙: dry-run은 "Release를 만들지 않는다"는 의미일 뿐, asset 생성·attestation·checksum 등 산출 파이프라인 자체는 동일 경로로 검증 — 그게 dry-run의 목적. ref 정책 또한 의도적 분기: `dry_run=true`는 임의 ref (브랜치 포함)에서 manual 트리거 가능 — 인프라 회귀 점검 목적이라 version 정합 강제는 부적합. `dry_run=false` (tag push 포함)만 tag ref + Cargo.toml version 정합을 fail-fast 강제 (G-020).

### Read-only 보장

`ADR 0001` (`docs/adr/0001-gh-subprocess-and-drop-push-tool.md`) — gitless-sync 도구의 read-only는 영구 결정. release workflow도 이 정합에 종속:

- workflow는 repo 코드를 수정하지 않는다. checkout + build + archive + upload만.
- 도구 binary 자체도 read-only 도구. release artifact = 검증된 read-only CLI.
- CHANGELOG/version bump 등 코드 변경은 release tag 작성 **전** 사람·ralph commit으로 처리 (별도 release phase). workflow 안에서 자동 commit/push 안 함.

### YAGNI

본 phase에서 의도적 제외 — 사유 한 줄 each.

1. **macOS notarization (Apple 공증)**: 별도 Apple Developer 계정 + signing 인프라 필요, 현재 비목표 (개인 도구 사용 목적). 사용자가 macOS Gatekeeper 차단 시 수동 우회 가이드만 README에 추가 검토 (별도 phase).
2. **Windows Authenticode 서명**: 별도 코드 서명 인증서 비용 + 인프라 필요, 현재 비목표. Windows SmartScreen 경고는 attestation verify로 대체 (불완전하지만 baseline).
3. **패키지 매니저 배포 (Scoop / Homebrew / winget)**: 각 매니저 manifest 유지·release pipeline 분기 필요. 현재 수요 0 (단일 사용자), 별도 phase.
4. **`cargo install` (crates.io publish)**: crates.io publish는 Rust toolchain 사용자에 한정 — 본 phase 목적(toolchain 없는 사용자 접근)과 직교. 필요하면 별도 phase.
5. **Install one-liner 스크립트** (`curl ... | sh` 패턴): supply chain 위험 + URL 안정성·hash pin 등 별도 보안 layer 필요. attestation verify 흐름이 안정화된 후 별도 phase 검토.
6. **GitHub Actions external action SHA pin**: 본 phase는 semver tag pin (`@v1`, `@v3`, `@v6`)으로 시작. SHA pin은 보안 강화 layer지만 dependabot 자동 갱신 정책으로 대체 가능 — 별도 phase에서 SHA pin + dependabot 동시 도입 검토.

각 항목은 비목표일 뿐 영구 제외 아님 — 본 spec 정합과 충돌하지 않는 별도 phase에서 추가 가능.

## Acceptance Criteria

본 spec 본문이 다음 항목을 모두 다룬다 (T2/T3 측 코드/문서 산출물 검증은 `implementation-plan.md` Phase 11 T2/T3 entry 참조 — duplicate 회피).

- [AUTO] § Target matrix 표가 3 target triple (`x86_64-pc-windows-msvc`, `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`) 모두 명시 + 검증 수준 column에 macOS = "build-only" 표기.
- [AUTO] § Archive 포맷 + Asset 명명에 `gitless-sync-{TAG}-{TARGET}.{EXT}` 패턴 + 3 target 예시 행 + archive 내부 layout fence (실행 파일 + README + LICENSE + CHANGELOG) 포함.
- [AUTO] § Trigger 섹션이 `push: tags: ['v*']` + `workflow_dispatch` (input `dry_run: boolean`, default `false`) 두 경로 명시.
- [AUTO] § Cross-compile 도구 섹션이 `taiki-e/upload-rust-binary-action@v1` 채택 + 채택 근거 ≥ 1줄.
- [AUTO] § Checksum 섹션이 per-asset `.sha256` + aggregate `sha256sums.txt` 두 형태 동시 emit + 사용자 검증 명령 (Windows `Get-FileHash` + Linux `sha256sum`) 두 예시 포함.
- [AUTO] § Attestation 섹션이 `actions/attest-build-provenance@v3` step + `gh attestation verify` 사용자 검증 명령 예시 포함.
- [AUTO] § Version 정합 섹션이 `docs/ralph/guardrails.md` § G-020 cross-link + workflow 측 검증 절차 (Cargo.toml `version` 추출 + tag leading `v` strip + byte 비교 + mismatch `exit 1` before build) 명시.
- [AUTO] § dry_run behavior 표가 `dry_run=false` / `dry_run=true` 두 경로별 step 수행/skip 매핑 (Release 생성·Actions artifact upload 분기 포함) 명시.
- [AUTO] § Read-only 보장 섹션이 `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` cross-link + workflow가 repo 코드 수정 안 함을 명시.
- [AUTO] § YAGNI 섹션이 정확히 6 bullets (macOS notarization / Windows Authenticode / 패키지 매니저 / `cargo install` / install one-liner / GHA SHA pin) + 각 한 줄 사유.
- [AUTO] § 목적 섹션이 release notes 작성 책임을 본 phase 비목표로 명시 (별도 release phase 책임 footnote).
