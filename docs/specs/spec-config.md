# Spec: Configuration Loading

## 목적
설정 값(repo, branch, ignore 패턴, 토큰)을 4단계 우선순위로 결정. CLI > env > `gitless-sync.toml` > 도구 내장 기본값.

## 현재 상태
- `crates/gitless-sync/src/shared/config.rs::Config` 구조체 + serde 정의 완료.
- `load(path: Option<&Path>)` 함수는 시그니처만 (`todo!()`).
- env 변수 `GITHUB_TOKEN`은 `clap(env = "GITHUB_TOKEN")`으로 자동 처리됨 (`main.rs`).

## 작업 범위

### 우선순위 (높음 → 낮음)
1. **CLI 인자** (`--repo`, `--branch`, `--ignore`, `--token`, `--keep-bom`, `--pretty`)
2. **env 변수**: `GITHUB_TOKEN` (필수, repo 권한)
3. **`gitless-sync.toml`** (프로젝트 루트에 있으면 자동 로드)
4. **도구 내장 기본값** (예: branch=`main`, local=`.`)

### `--token` 형식
- `env:<name>` — 명명된 env 변수에서 토큰 읽음 (예: `--token env:MY_TOKEN`).
- `literal:<value>` — 인자에 직접 토큰 (CI 등 비대화식 환경. 보안 주의).
- 미지정 시 기본 동작: `$GITHUB_TOKEN` env 변수 사용 (clap의 `env = "GITHUB_TOKEN"` 동작).

### `gitless-sync.toml` 스키마
```toml
repo = "owner/name"
branch = "main"
ignore = ["dist/", "*.tmp"]
```
모든 필드는 옵셔널. 누락 시 다음 우선순위로 fallback.

### 비밀 정보 정책
토큰 같은 비밀 정보는 도구 코드·repo에 절대 포함하지 않음. `gitless-sync.toml`에 토큰 필드 정의하지 않음 (commit 위험).

## Acceptance Criteria
- `[AUTO]` `config::load(Some(path_to_toml))`가 정상 TOML 파일을 파싱한다.
- `[AUTO]` `config::load(None)` 또는 파일 없는 경로 → `Config::default()` 반환 (필드 모두 None / 빈 Vec).
- `[AUTO]` `gitless-sync.toml` 파싱 에러 시 → `GitlessError::Config(...)`, exit code 1.
- `[AUTO]` Token 파싱: `--token env:MY_TOKEN` → `MY_TOKEN` env 변수 읽음. 변수 미설정 시 `GitlessError::AuthFailed`, exit code 2.
- `[AUTO]` Token 파싱: `--token literal:ghp_xxx` → `ghp_xxx` 그대로 사용.
- `[AUTO]` Token 파싱: `--token` 미지정 + `GITHUB_TOKEN` env 미설정 → `GitlessError::AuthFailed`, exit code 2 (PRD 검증 시나리오 10).
- `[AUTO]` 우선순위 검증: CLI `--repo "a/b"` + toml `repo="e/f"` → 결과는 `a/b` (CLI 승리).
- `[AUTO]` `repo` 필드가 모든 소스에서 누락 시 → `GitlessError::Config("repo not specified")`, exit code 1.
