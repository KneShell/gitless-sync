# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

TBD — Phase 5 audit sweep (task Z) + Phase 7+ 진입 시점 추가.

## [0.2.0] - 2026-05-09

> Phase 5 — 도메인 함정 정리. 38 task ralph 자율 진행 본진 종료 (V1 CHANGELOG + Z audit sweep 잔여). 사람 개입 0회 (advisor BLOCKING fix 다수는 self-correct).

### Added

#### Pitfall handling — 8 핵심 + 신규 함정 4건

- **NFC path normalization** — walker (`commands/scan/walker.rs`) + remote tree (`shared/github/trees.rs` 3 mode) 양쪽 적용. NFD/NFC 입력은 NFC key로 collapse.
- **case_collision detection** — 3 시나리오 구현 (canonical / diagonal / local-both). `commands/scan/case_collision.rs` (vertical slice 정합) + `compare.rs` 분류 진입.
- **encoding_rs 다중 인코딩 변환 시도 후 detect-only** — UTF-8 1차 시도 → encoding_rs sniff (Shift_JIS / EUC_KR / GBK / Windows-1252) → binary fallback. **hash 입력은 항상 원본 raw bytes** (b-policy). `shared/decode.rs` 추가.
- **Submodule (`160000`) detect-only** — `Status::Failed` + `failed_reason: "submodule"` + `mode: "160000"` JSON 출력.
- **Symlink (`120000`) detect-only** — local symlink (lstat-only, dangling/circular는 graceful skip) + remote tree symlink entry 양쪽 detect.
- **Empty file 실파일 검증** — 0-byte file ↔ remote empty blob → `Status::Identical` (G-010).
- **Executable (`100755`) mode bit detect** — content 동일 시 `Status::Identical` 유지 + JSON `mode: "100755"` 포함.
- **`.gitattributes` 파서 추가** — `shared/gitattributes.rs` (296 LOC, ※ Z task에서 module 폴더 분할 예정). project root + 하위 디렉토리 1회 로드 + glob pattern matching (gitignore-style) + 가장 깊은 `.gitattributes` 우선 + line-level 마지막 매칭 winner. `.git/info/attributes` / global 미지원.
- **`.gitattributes` attribute 화이트리스트** — `AttributeMatch` enum 5 entry: `TextAuto / Binary / EolLf / EolCrlf / LfsPointer` + `Unspecified / Unsupported { attribute_name }`. 화이트리스트 외 (`working-tree-encoding`, `ident`, `filter=*` (lfs 외), macro attributes, legacy `crlf`) → `Unsupported`.
- **`prepare_for_hash` 7 분기 helper** — `apply_text_auto` / `apply_binary` / `apply_eol_lf` / `apply_eol_crlf` / `apply_unspecified` 5 helper + `LfsPointer` / `Unsupported` caller-side `Status::Failed`. cognitive_complexity 15 deny 회피.
- **BOM 처리** — UTF-8 BOM strip (text=auto + 미명시 정책) + UTF-16 BOM (`FF FE` LE / `FE FF` BE) detect → `Status::Failed` + `failed_reason: "encoding"`.
- **git LFS pointer detection** — `.gitattributes` `filter=lfs` 매칭 path는 자동 `Status::Failed` + `failed_reason: "lfs_pointer"` + `lfs_pointer: {oid: "?", size: 0}`. **scan은 blob fetch 안 함** (Phase 4 batching 이득 보존). diff는 first-line signature `version https://git-lfs.github.com/spec/v1` 검증 + oid/size 정확 파싱 (defence-in-depth).
- **Windows long path / 예약 파일명 detect-only** — 260자+ path 또는 예약 파일명 (`CON` / `PRN` / `NUL` / `AUX` / `COM1-9` / `LPT1-9`) detect → `Status::Failed` + `failed_reason: "long_path"`. `commands/scan/long_path.rs` 추가.

#### Schema v1.1 (minor — backward compat)

- 새 필드 — `mode` (4-digit octal: `100644` / `100755` / `120000` / `160000`) + `failed_reason` (skip_serializing on `None`) + `lfs_pointer` (skip_serializing on `None`).
- v1.0 backward-compat lock test 추가 (`output.rs::tests` 5 lock).
- envelope `schema_version: "1.0"` → `"1.1"` minor bump.

#### Dependencies

- `unicode-normalization = "0.1"` — NFC normalize 도입.
- `encoding_rs = "0.8"` (Mozilla, Apache-2.0/MIT) — 다중 인코딩 sniff 도입. cargo-bloat measurement: `.text` section attribution 0 KiB (LTO + strip + dead code elim, encoding shortlist 4종만 retain).

#### Specs (신규)

- `docs/specs/spec-domain-pitfalls.md` — Phase 5 모든 함정 spec hub.

#### Research artifacts (신규)

- `docs/research/phase5-vault-baseline.md` — Phase 5 진입 시점 vault scan baseline.
- `docs/research/phase5-vault-after.md` — Phase 5 후 vault dogfood (T 결과: 117 files / 81 identical / 36 local_only_changed / 0 drift / 0 failed).
- `docs/research/phase5-regression.md` — v0.1 vs v0.2 binary diff (W 결과: REGRESSION 0건, envelope W1/W2 정확화만, 121/121 path binary delta 0).
- `docs/research/phase5-gitattributes-bench.md` — `.gitattributes` parser perf baseline (X 결과: 100 rules × 10K paths 측정 P95 50.2 µs).
- `docs/research/phase5-scan-scale-bench.md` — large vault scale (R3 결과: 10K real-file scan, `.gitattributes` overhead 2.82x).
- `docs/research/encoding-library-eval.md` — encoding_rs vs chardet 평가 + cargo-bloat 사후 측정 (Y).

#### CI gate (Windows runner)

- `.github/workflows/ci.yml` 4 게이트 추가 — `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` + `cargo tarpaulin --engine llvm --workspace --out Stdout --fail-under 80`.

### Changed

- **`prepare_for_hash` 시그니처** — `gitattr: &Arc<GitAttributes>` + `path: &str` 인자 추가. caller 모두 갱신.
- **`status="failed"` 의미 확장** — v0.1 = "hash IO 실패"만, v0.2 = 9 reasons (`hash_io` / `encoding` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `nfd_collision` / `case_collision` / `gitattributes_unsupported`). 기존 호출자가 `status == "failed"` 단일 분기 사용 시 `failed_reason` 추가 분기 필요 — 특히 LFS pointer를 hash error로 오인 위험. ※ 5 reason은 코드 구현 (`hash_io` / `submodule` / `symlink` / `lfs_pointer` / `long_path` / `case_collision`), 3 reason (`encoding` / `nfd_collision` / `gitattributes_unsupported`)은 enum-spec'd-but-unimplemented (caller-side plumbing follow-up).
- **default LF normalize policy 변경** — v0.1 = 항상 LF normalize, v0.2 = `.gitattributes` conditional + 화이트리스트 5 entry 만 적용. `Unspecified` (default) branch는 v0.1 정책 그대로 (호환).

### Deprecated

- 없음.

### Removed

- 없음.

### Fixed

- **G-005 mtime 휴리스틱 적용 정합** — `local_mtime == remote_last_commit_at` 동률은 `Status::Drift`로 격하. v0.1부터 spec에 정의된 룰을 Phase 5 audit에서 각 함정 처리 후 정합 검증.

### Security

- 없음.

### Verified

- **vault dogfood (T, 2026-05-09)** — KneShell/gitless-sync@main 117 files / 0 drift / 0 failed (false drift 0건).
- **v0.1 baseline regression diff (W, 2026-05-09)** — REGRESSION 0건 (envelope W1 schema_version + W2 mode field 정확화만). 자동 fail trigger 발동 없음.
- **383 tests pass** — 293 lib + 41 integration + 49 xtask.
- **tarpaulin 90.73%** (949/1046 lines).

### Known limitations

- **`failed_reason` enum-spec'd-but-unimplemented 3건** — `encoding` / `nfd_collision` / `gitattributes_unsupported`. detect 코드는 구현됐으나 caller-side `pipeline.rs` plumbing follow-up. Phase 5 후속 task로 처리.
- **`.gitattributes` module 폴더 분할 미완료** — `shared/gitattributes.rs` 단일 file (296 LOC) + sibling `gitattributes_tests.rs` + `gitattributes_classify_tests.rs`. spec-architecture.md § 금지 패턴 (sibling test file) 위반 1건. **task Z** (audit + cleanup sweep)에서 `shared/gitattributes/{mod, parser, classify, matching}` 4 file 분할 + sibling 정리 진행.
- **vault scale 1000+ path mtime cache 재검토 미진행** — ADR 0008 § Future work 그대로 (50 path scale에선 noise floor, vault scale 측정은 v0.3+).

## [0.1.0] - Phase 6 완료 시점 (2026-05-09)

> Phase 1 ~ Phase 6 누적. v0.1 + v0.2 (gh subprocess 마이그레이션) + Phase 4 (GraphQL batching) + Phase 6 (Code Quality) 완료.

### Added

#### CLI 명령어
- `scan` — 로컬 디렉토리 ↔ GitHub repo 단방향 비교 → 4-state JSON 출력.
- `diff` — 단일 파일 차이 표시.
- `init` — `gitless-sync.toml` stdout TOML emit (사용자 redirect).

#### 분류
- 4-state classification: `identical` / `local_only_changed` / `remote_only_changed` / `drift` / `failed`.
- 시간 비교 휴리스틱 (G-005).
- Schema v1.0 정의 (호출자 backward-compat 보장).

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

- **도메인 함정**: NFD vs NFC, 대소문자 충돌, 비-UTF-8 인코딩, submodule, symlink, 빈 파일 실파일 검증, 실행 권한, `.gitattributes`. v0.2 (Phase 5)에서 해소.
- **추가 함정** (clean-context 보강): UTF-16 BOM, git LFS pointer, Windows long path / 예약 파일명, `.gitignore` 무시 정책 명시.

### Excluded (영구 비목표)

- 3-way merge / 양방향 동기화.
- 인터랙티브 UI.
- GitHub 외 호스팅 (GitLab, Bitbucket).
- LFS 추적 파일 직접 fetch (pointer detect-only — fetch는 호출자 책임).
- Event 기반 layer 통신 (channel/observer/actor) — yagni 영구 제외 (Phase 6 vague).
