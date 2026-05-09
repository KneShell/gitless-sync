# Spec: Ignore Policy

## 목적
로컬 디렉토리 walk 시 비교 대상에서 제외할 파일/디렉토리 패턴 결정. 도구 내장 기본 + 로컬 `.gitignore` + 사용자 `--ignore` 인자 + 설정 파일 ignore 패턴의 **합집합**.

## 현재 상태
- `crates/gitless-sync/src/shared/ignore.rs::IgnoreMatcher` 구현 완료 (`Gitignore` 매처 + 4 소스 합집합).
- `crates/gitless-sync/src/commands/scan/walker.rs::walk` 가 매처를 적용 — 파일 entry는 `is_ignored` 매치 시 skip, 디렉토리는 probe path (`<dir>/.probe`) 기법으로 prune.
- 의존성: `ignore` crate (`Cargo.toml`에 박힘) + `walkdir`.

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

### Scan 범위 (Phase 5)

ignore 매치된 path는 **scan 결과 어디에도 박지 않는다**. 정확한 의미:

1. **walker 산출물 제외**: `walk(root, matcher)` 가 반환하는 `Vec<LocalFile>` 에 박지 않음 (`walker.rs`).
2. **비교 대상 제외**: `compare`/`classify` 입력에 박지 않음 — 4분류 판정 자체가 일어나지 않음.
3. **summary 카운트 제외**: `Summary { identical, local_only_changed, remote_only_changed, drift, failed }` 어느 카운터에도 +1 하지 않음 (`output.rs::Summary`). ignored 카운터는 별도 박지 않는다 — scan 범위 자체에서 사라짐.
4. **JSON `files[]` 제외**: `--include-files` 박혀도 ignored path는 entry로 직렬화되지 않음.

**디렉토리 prune 정책**: `walker.rs::walk` 는 디렉토리 진입 시 probe path (`<dir>/.probe`) 로 매치 검사한 뒤 prune. 즉 `node_modules/`, `target/`, `.git/` 같은 대형 디렉토리는 walk cost 자체가 0 (descent 안 함). 비교 대상에서 제외만 하는 게 아니라 IO 자체가 안 일어남.

**원격 측 정책**: GitHub Trees API 응답의 path는 ignore 적용 안 함 (원격은 git 관리 하 — `.git/` 같은 path가 원격에 박힐 일이 없음). 단 `--ignore` CLI 패턴은 원격 path도 동일 매처로 거름 (양쪽 합집합 동률 보장 — local-only 또는 remote-only 분류가 ignore 패턴으로 인해 비대칭 박히는 걸 방지).

### 사용자가 builtin을 끄는 옵션
v0.1 비목표. 사용자가 builtin을 명시적으로 끄려면 별도 미래 기능 (Phase 2+).

## Acceptance Criteria
- `[AUTO]` `IgnoreMatcher::new(root, &[])`는 root에 `.gitignore`가 없어도 성공한다.
- `[AUTO]` builtin 패턴 매치: `is_ignored(Path::new(".git/HEAD"))` == `true`.
- `[AUTO]` builtin 패턴 매치: `is_ignored(Path::new("node_modules/foo"))` == `true`.
- `[AUTO]` builtin 패턴 매치: `is_ignored(Path::new("target/debug/build/foo"))` == `true` (Phase 5 — `target/` 검증 박힘, `ignore::tests::builtin_matches_target_subpath`).
- `[AUTO]` `--ignore "*.log"` 인자 → `is_ignored(Path::new("debug.log"))` == `true`.
- `[AUTO]` `.gitignore`에 `dist/` 있을 때 → `is_ignored(Path::new("dist/bundle.js"))` == `true`.
- `[AUTO]` `.gitignore` + `--ignore` 합집합: `.gitignore`에 `dist/`, 사용자 `--ignore "*.bak"` → 양쪽 다 매치.
- `[AUTO]` PRD 검증 시나리오 9: `.gitignore`에 패턴 + `--ignore` 인자가 합집합으로 동작 (통합 테스트, tempfile + 실제 .gitignore 파일).
- `[AUTO]` 매칭 키는 forward slash (Windows 백슬래시 입력도 정규화 후 매칭).
- `[AUTO]` Phase 5 — walker prune: `node_modules/` 디렉토리는 walk 자체 안 함 (`walker::tests::skips_directory_excluded_by_builtin`).
- `[AUTO]` Phase 5 — walker prune: 중첩된 `target/` 디렉토리도 prune (`walker::tests::nested_ignored_directory_is_pruned`).
- `[AUTO]` Phase 5 — custom `.gitignore` 패턴 walk 적용: `.gitignore`에 `build/` 박음 → `build/` 하위 file이 walker 산출물에 박지 않음 (`walker::tests::applies_gitignore_from_root`).
