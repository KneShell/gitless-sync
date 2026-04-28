# Spec: Ignore Policy

## 목적
로컬 디렉토리 walk 시 비교 대상에서 제외할 파일/디렉토리 패턴 결정. 도구 내장 기본 + 로컬 `.gitignore` + 사용자 `--ignore` 인자 + 설정 파일 ignore 패턴의 **합집합**.

## 현재 상태
- `crates/gitless-sync/src/shared/ignore.rs::IgnoreMatcher` 구조체 + `BUILTIN_IGNORES` 상수 박힘.
- `IgnoreMatcher::new`, `is_ignored` 모두 `todo!()` — 구현 필요.
- 의존성: `ignore` crate (Cargo.toml에 박힘).

## 작업 범위

### 패턴 소스 (모두 합집합)
1. **도구 내장 기본** (`BUILTIN_IGNORES`): `.git/`, `.DS_Store`, `Thumbs.db`, `desktop.ini`, `node_modules/`, `target/`.
2. **로컬 `.gitignore`** (root에 있으면 자동 적용).
3. **`--ignore <pattern>`** CLI 인자 (반복 가능).
4. **설정 파일 ignore 패턴** (`spec-config.md`).

### 패턴 문법
gitignore 표준 문법 (`ignore` crate가 처리). 와일드카드 `*`, `**`, 디렉토리 suffix `/`, 부정 `!` 등.

### 매칭 동작
- `is_ignored(path)`는 위 4개 소스 중 어느 하나라도 매치되면 true.
- 부정 패턴(`!foo`)은 `ignore` crate의 표준 동작을 따른다 (마지막 매치 우선).
- 매칭 키는 root 기준 상대 경로, forward slash (G-004).

### 사용자가 builtin을 끄는 옵션
v0.1 비목표. 사용자가 builtin을 명시적으로 끄려면 별도 미래 기능 (Phase 2+).

## Acceptance Criteria
- `[AUTO]` `IgnoreMatcher::new(root, &[])`는 root에 `.gitignore`가 없어도 성공한다.
- `[AUTO]` builtin 패턴 매치: `is_ignored(Path::new(".git/HEAD"))` == `true`.
- `[AUTO]` builtin 패턴 매치: `is_ignored(Path::new("node_modules/foo"))` == `true`.
- `[AUTO]` `--ignore "*.log"` 인자 → `is_ignored(Path::new("debug.log"))` == `true`.
- `[AUTO]` `.gitignore`에 `dist/` 있을 때 → `is_ignored(Path::new("dist/bundle.js"))` == `true`.
- `[AUTO]` `.gitignore` + `--ignore` 합집합: `.gitignore`에 `dist/`, 사용자 `--ignore "*.bak"` → 양쪽 다 매치.
- `[AUTO]` PRD 검증 시나리오 9: `.gitignore`에 패턴 + `--ignore` 인자가 합집합으로 동작 (통합 테스트, tempfile + 실제 .gitignore 파일).
- `[AUTO]` 매칭 키는 forward slash (Windows 백슬래시 입력도 정규화 후 매칭).
