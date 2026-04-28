# Spec: CLI Interface

## 목적
사용자(특히 AI 호출자)가 안정적으로 호출할 수 있는 CLI 인자/플래그 표면. 명령어는 `scan`(JSON 결과)과 `diff`(특정 파일의 raw text diff) 두 개.

## 현재 상태
- `crates/gitless-sync/src/main.rs`에 clap derive로 `Cli`, `Commands` 정의 완료. `scan` / `diff` 디스패치 + exit code 매핑까지 박힘.
- 글로벌 플래그 (`--repo`, `--branch`, `--local`, `--ignore`, `--token`, `--keep-bom`, `--pretty`)와 `scan` 전용 플래그 (`--summary-only`, `--status`)가 시그니처에 있음.
- `verbose` 플래그 (`-v`, `-vv`)는 미반영. 추가 필요.

## 작업 범위

### 명령어
- `gitless-sync scan` — 비교 실행, 4분류 결과를 stdout JSON으로 출력.
- `gitless-sync diff <path>` — 특정 파일의 raw text unified diff를 stdout으로 출력. 양쪽 normalize 후 비교.

### 글로벌 플래그
| 플래그 | 의미 | 기본값 |
|--------|------|--------|
| `--repo <owner/name>` | GitHub repo | (필수, 또는 config에서) |
| `--branch <name>` | branch | `main` |
| `--local <path>` | 로컬 디렉토리 | `.` (cwd) |
| `--ignore <pattern>` | ignore 패턴 (반복 가능, gitignore 문법) | (없음) |
| `--token <env\|literal>` | GitHub 토큰. 형식: `env:<name>` 또는 `literal:<value>` | `$GITHUB_TOKEN` |
| `--keep-bom` | UTF-8 BOM 보존 모드 | false |
| `--json` / `--pretty` | 출력 포맷 | `--json` (compact) |
| `-v` / `-vv` | stderr 로그 레벨 (info / debug) | warning만 |
| `--backend <rest\|graphql>` | GitHub API 호출 backend 선택. v0.1는 `rest`만 활성, `graphql`은 인터페이스만 박힌 stub (Phase 4에서 활성화). | `rest` |

### Backend 분기 (v0.1 인터페이스)
- `--backend rest` (기본): 정상 동작. 기존 v0.1 흐름 그대로.
- `--backend graphql`: `GitlessError::Config("GraphQL backend not implemented in v0.1; use --backend rest. Phase 4 ETA.")` 즉시 반환, exit code 1.
- 호출자(LLM)는 v0.1부터 `--backend graphql`을 명시 가능 → Phase 4 활성화 시 호출 코드 변경 없음 (forward-compat 보장).
- 자세한 정책: `spec-github-api.md` § Backend 선택.

### `scan` 전용 플래그
- `--summary-only` — `files[]` 배열 빼고 `summary` 객체만 출력. 큰 vault에서 LLM 컨텍스트 절약.
- `--status <list>` — 콤마 구분 enum 필터. 예: `--status drift,local_only_changed`. 지정한 status 파일만 `files[]`에 포함.

### `diff` 전용 인자
- `<path>` — 비교할 파일의 상대 경로 (forward slash).

### 인자 우선순위
CLI > env > `gitless-sync.toml` > 도구 내장 기본값. 자세한 건 `spec-config.md`.

## Acceptance Criteria
- `[AUTO]` `cargo run -- --help`가 위 모든 플래그를 보여준다.
- `[AUTO]` `cargo run -- scan --help`가 `--summary-only`, `--status`를 추가로 보여준다.
- `[AUTO]` `cargo run -- diff --help`가 `<path>` positional 인자를 보여준다.
- `[AUTO]` `--ignore` 플래그를 두 번 이상 지정하면 `Vec<String>`에 누적된다 (clap 기본 동작).
- `[AUTO]` `--token env:GITHUB_TOKEN`과 `--token literal:ghp_...` 두 형식이 모두 파싱된다 (구현은 `spec-config.md`).
- `[AUTO]` `--status drift,local_only_changed` 같은 콤마 구분 입력이 `Vec<Status>`로 파싱된다.
- `[AUTO]` clap이 알 수 없는 플래그를 받으면 비-zero 종료 + stderr에 사용법 출력 (clap 기본).
