# Project Operations

## Build / Compile
- `cargo build` — debug 빌드
- `cargo build --release` — release 빌드
- `cargo check` — 타입 체크만 (빠른 피드백)

## Test
- `cargo test --workspace` — 전체 테스트
- `cargo test --workspace -- --nocapture` — println! 노출
- `cargo test <name>` — 특정 테스트만

## Lint / Validate
- `cargo clippy --workspace --all-targets -- -D warnings` — clippy 워크스페이스 (warning을 error로)
- `cargo fmt --check` — 포맷 검사 (수정 없이 위반만 보고)
- `cargo fmt` — 자동 포맷 적용
- `cargo deny check` — 의존성 정책 (라이선스, 보안, 중복 등)
- `cargo audit` — 알려진 보안 취약점 스캔

## Coverage
- `cargo tarpaulin --engine llvm --out Stdout --workspace` — 라인 커버리지 측정 (Windows는 LLVM 백엔드 필수)
- 게이트: 라인 커버리지 ≥ 80%. 미달 시 빌드 실패로 간주.
- 통합 테스트(임시 디렉토리 + mock HTTP)는 별도 카운트, 80% 게이트엔 미반영.

## Full Validation Pipeline
1. `cargo fmt --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo xtask check-line-limits` (file ≤ 300 LOC)
4. `cargo xtask check-cycles` (cycle 0 + cross-slice 0)
5. `cargo machete` (unused dependency 0건)
6. `cargo test --workspace`
7. `cargo tarpaulin --engine llvm --workspace --out Stdout` (≥ 80%)
8. `cargo deny check` (의존성 변경 시)
9. `cargo audit` (의존성 변경 시)

위 1~7을 순서대로 통과하면 task 완료 가능. 8~9는 의존성 추가/변경한 task에 한해 추가. `cargo-public-api`는 CI gate(`.github/workflows/ci.yml`) 한정 — local에서는 사용 안 함 (ref + nightly 부담).

## Architecture Tools (Phase 6)

`docs/specs/spec-architecture.md` § 외부 도구 박제. Phase 6 Step 3/4에서 cycle/LOC/API/unused dependency 게이트 적용 시 사용.

### 설치
```
cargo install cargo-modules     # 0.26+ (의존 그래프 + cycle 검출)
cargo install cargo-public-api  # 0.51+ (API 변경 추적, nightly toolchain 필요)
cargo install cargo-machete     # 0.9+ (unused dependency)
rustup toolchain install nightly  # cargo-public-api 전용. 본 프로젝트 빌드는 stable 1.95.0 그대로.
```

### 명령어

- `cargo modules dependencies -p gitless-sync --lib --no-fns --no-types --no-traits --no-sysroot` — graphviz dot 출력. `cargo xtask check-cycles`가 이 출력을 파싱해 module-level uses 그래프에서 cycle 검출 + cross-slice ref 위반 검출 (`commands/scan` ↔ `commands/diff` ↔ `commands/init` 간 import 금지). 위반 1건 이상이면 exit 1.
  - **WHY 직접 파싱**: cargo-modules `--acyclic` flag는 type-method edge(예: `enum GitlessError` ↔ `fn exit_code`)를 cycle로 잡는 false positive가 있어 모듈 단위 분석에 부적합.
- `cargo public-api -p gitless-sync` — 워크스페이스 manifest는 미지원, `-p`로 패키지 지정 필수. 실행 시 nightly로 자동 fallback (rustup default는 stable 그대로). diff는 `cargo public-api diff <ref>`. 분할 회귀 가드 — 의도치 않은 public 노출 검출.
- `cargo machete` — unused dependency 검출. Exit 0 = clean, exit 1 = 위반 발견. **현 baseline**: 위반 0건 (task O 시점 `anyhow` 제거로 정리, 2026-05-09). 정 false positive 시 `[package.metadata.cargo-machete] ignored = [...]`로 명시.

### 게이트 적용 시점

| 도구 | xtask wrap | 적용 task | enforcement |
|---|---|---|---|
| cargo-modules | `cargo xtask check-cycles` | E | deny active (cycles + cross-slice refs, baseline 0 위반) |
| (없음 — pure xtask) | `cargo xtask check-line-limits` | D (warn) → J (deny) | deny active (300 LOC, baseline 0 위반, doc-heavy 면제) |
| cargo-public-api | (직접 실행, CI 비교) | task O CI gate | PR 이벤트에서 `diff origin/<base>..HEAD` 표시 (deny 아님) |
| cargo-machete | (직접 실행, CI gate) | task O CI gate | CI gate active (exit 1 deny, baseline 0 위반) |

상세 권장 cli/exit 정책은 task E/J/L/O 시점 spec/plan 갱신 동반.

## Git Workflow
- 브랜치: `main` 단일. 작업 브랜치 분기 없이 진행 (소규모 단일 개발자).
- 커밋 단위: 한 task = 한 commit. ralph build iteration 종료 직전 commit.
- 커밋 메시지: Conventional Commits 약식 사용 (`feat:` `fix:` `test:` `docs:` `refactor:` `chore:`).
- `Cargo.lock`은 commit 대상 (binary CLI).
