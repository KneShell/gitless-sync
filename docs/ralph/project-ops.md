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
3. `cargo test --workspace`
4. `cargo tarpaulin --engine llvm --workspace --out Stdout` (≥ 80%)
5. `cargo deny check` (의존성 변경 시)
6. `cargo audit` (의존성 변경 시)

위 1~4를 순서대로 통과하면 task 완료 가능. 5~6은 의존성 추가/변경한 task에 한해 추가.

## Architecture Tools (Phase 6)

`docs/specs/spec-architecture.md` § 외부 도구 박제. Phase 6 Step 3/4에서 cycle/LOC/API/unused dependency 게이트 박는 데 사용.

### 설치
```
cargo install cargo-modules     # 0.26+ (의존 그래프 + cycle 검출)
cargo install cargo-public-api  # 0.51+ (API 변경 추적, nightly toolchain 필요)
cargo install cargo-machete     # 0.9+ (unused dependency)
rustup toolchain install nightly  # cargo-public-api 전용. 본 프로젝트 빌드는 stable 1.95.0 그대로.
```

### 명령어

- `cargo modules dependencies -p gitless-sync --bin gitless-sync --no-fns --no-types --no-traits --no-sysroot --acyclic` — graphviz dot 출력 + `--acyclic` 시 cycle 1건 이상이면 exit ≠ 0. task E `cargo xtask check-cycles`가 이를 wrap.
- `cargo public-api -p gitless-sync` — 워크스페이스 manifest는 미지원, `-p`로 패키지 지정 필수. 실행 시 nightly로 자동 fallback (rustup default는 stable 그대로). diff는 `cargo public-api diff <ref>`. 분할 회귀 가드 — 의도치 않은 public 노출 검출.
- `cargo machete` — unused dependency 검출. Exit 0 = clean, exit 1 = 위반 발견. 현 baseline: gitless-sync에서 `anyhow` 1건 unused (panic fix task S 시점 자연 해결 예정 — task O CI gate 박을 시 task S 이후 활성화). 정 false positive 시 `[package.metadata.cargo-machete] ignored = [...]`로 박음.

### 게이트 박힘 시점

| 도구 | xtask wrap | 박힘 task | enforcement |
|---|---|---|---|
| cargo-modules | `cargo xtask check-cycles` | E | task J에서 deny |
| cargo-public-api | (직접 실행, CI 비교) | task O CI gate | API diff CI 표시 |
| cargo-machete | (직접 실행, CI gate) | task O CI gate | task S 이후 deny |

상세 권장 cli/exit 정책은 task E/L/O 시점 spec/plan 갱신 동반.

## Git Workflow
- 브랜치: `main` 단일. 작업 브랜치 분기 없이 진행 (소규모 단일 개발자).
- 커밋 단위: 한 task = 한 commit. ralph build iteration 종료 직전 commit.
- 커밋 메시지: Conventional Commits 약식 사용 (`feat:` `fix:` `test:` `docs:` `refactor:` `chore:`).
- `Cargo.lock`은 commit 대상 (binary CLI).
