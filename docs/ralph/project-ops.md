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

## Git Workflow
- 브랜치: `main` 단일. 작업 브랜치 분기 없이 진행 (소규모 단일 개발자).
- 커밋 단위: 한 task = 한 commit. ralph build iteration 종료 직전 commit.
- 커밋 메시지: Conventional Commits 약식 사용 (`feat:` `fix:` `test:` `docs:` `refactor:` `chore:`).
- `Cargo.lock`은 commit 대상 (binary CLI).
