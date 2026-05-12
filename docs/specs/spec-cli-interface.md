# Spec: CLI Interface

## 목적
사용자(특히 AI 호출자)가 안정적으로 호출할 수 있는 CLI 인자/플래그 표면. 명령어는 `scan`(JSON 결과)과 `diff`(특정 파일의 raw text diff) 두 개.

## 현재 상태
- `crates/gitless-sync/src/main.rs`에 clap derive로 `Cli`, `Commands` 정의 완료. `scan` / `diff` 디스패치 + exit code 매핑까지 구현됨.
- 글로벌 플래그 (`--repo`, `--branch`, `--local`, `--ignore`, `--keep-bom`, `--pretty`)와 `scan` 전용 플래그 (`--summary-only`, `--status`)가 시그니처에 있음.
- `verbose` 플래그 (`-v`, `-vv`)는 미반영. 추가 필요.
- 인증 토큰 입력은 본 도구에서 받지 않는다 (ADR 0002). `gh auth login` 으로 사전 처리.

## 작업 범위

### 명령어
- `gitless-sync scan` — 비교 실행, 4분류 결과를 stdout JSON으로 출력.
- `gitless-sync diff <path>` — 특정 파일의 raw text unified diff를 stdout으로 출력. 양쪽 normalize 후 비교.
- `gitless-sync init` — repo/branch/ignore 인자에서 `gitless-sync.toml` 본문을 stdout TOML로 출력. 도구는 파일을 작성하지 않는다 — 사용자가 shell redirect로 영구 파일 생성 (ADR 0004).

### 글로벌 플래그
| 플래그 | 의미 | 기본값 |
|--------|------|--------|
| `--repo <owner/name>` | GitHub repo | (필수, 또는 config에서) |
| `--branch <name>` | branch | `main` |
| `--local <path>` | 로컬 디렉토리 | `.` (cwd) |
| `--ignore <pattern>` | ignore 패턴 (반복 가능, gitignore 문법) | (없음) |
| `--keep-bom` | UTF-8 BOM 보존 모드 | false |
| `--json` / `--pretty` | 출력 포맷 | `--json` (compact) |
| `-v` / `-vv` | stderr 로그 레벨 (info / debug) | warning만 |
| `--backend <rest\|graphql>` | GitHub API 호출 backend 선택. default는 `graphql` (ADR 0006), REST는 explicit fallback으로 유지. | `graphql` |

### Backend 분기

> **갱신 (ADR 0006, 2026-05-07)**: default backend `rest` → `graphql` 전환. v0.1 stub 표현은 obsolete (P3a에서 본체 구현됨, P3b에서 stub error 제거).

- `--backend graphql` (default): `gh api graphql` subprocess + alias batching. 호출자(LLM)가 명시 안 해도 default로 활성. 자세한 정책: `spec-github-api.md` § GraphQL backend.
- `--backend rest` (explicit fallback): v0.1/v0.2에서 검증된 REST + rayon 8c 흐름. GraphQL 운영 이슈(rate limit, alias batching 응답 정합성, partial errors 등) 발생 시 즉시 fallback. 자세한 정책: `spec-github-api.md` § Backend 선택 + § fetch_tree / fetch_blob / fetch_last_commit_at + § 병렬 호출 정책 § REST 분기.
- 호출자(LLM) 인터페이스 변경 0 — 결과 ScanReport JSON identical. backend 인지 부담 0. v0.1부터 `--backend` flag 시그니처는 그대로 호환.

### `scan` 전용 플래그
- `--summary-only` — `files[]` 배열 빼고 `summary` 객체만 출력. 큰 vault에서 LLM 컨텍스트 절약.
- `--status <list>` — 콤마 구분 enum 필터. 예: `--status drift,local_only_changed`. 지정한 status 파일만 `files[]`에 포함.

### `diff` 전용 인자
- `<path>` — 비교할 파일의 상대 경로 (forward slash).
- `--json` — diff 출력을 JSON 형식으로 전환 (opt-in). 미명시 시 기존 unified text stdout 유지 (default). 상세 스키마는 § diff --json 출력 형식.

### diff --json 출력 형식

`--json` 명시 시 stdout 한 줄 JSON. stderr side marker 미출력.

```json
{"side": "...", "unified": "..." | null, "raw": "..." | null, "binary": bool}
```

| field | type | 의미 |
|-------|------|------|
| `side` | `"both"` \| `"local_only"` \| `"remote_only"` | 파일 존재 위치 (presence). |
| `unified` | `string \| null` | normalize 후 unified diff 텍스트 (side=both 한정). normalize-equal이면 `""`, normalize-diff이면 diff 텍스트. side≠both이면 `null`. |
| `raw` | `string \| null` | 단일 사이드 원본 파일 내용 (side=local_only 또는 remote_only 한정). side=both이면 `null`. |
| `binary` | `bool` | 바이너리 파일이면 `true`. `unified` / `raw` 모두 `null`. |

케이스별 stdout:

| 케이스 | stdout |
|--------|--------|
| side=both + normalize-equal | `{"side":"both","unified":"","raw":null,"binary":false}` |
| side=both + normalize-diff | `{"side":"both","unified":"--- a/…\n+++ b/…\n…","raw":null,"binary":false}` |
| side=local_only | `{"side":"local_only","unified":null,"raw":"<file content>","binary":false}` |
| side=remote_only | `{"side":"remote_only","unified":null,"raw":"<file content>","binary":false}` |
| binary | `{"side":"<side>","unified":null,"raw":null,"binary":true}` |

authoritative sub-schema 정의는 `spec-output-schema.md` § diff sub-schema (task D scope).

### init subcommand

#### 인자
- `--repo <owner/name>` — **필수**. 미명시 또는 빈 문자열이면 `GitlessError::Config("repo not specified")` 즉시 반환, exit code 1, stdout 출력 0, stderr `error_code: "CONFIG"`. 글로벌 `--repo`와 다르게 init은 fallback 소스(env / toml) 없이 인자만 본다 — toml 파일을 만드는 도구이므로 자기 자신을 입력 소스로 쓸 수 없음.
- `--branch <name>` — 옵셔널. 명시 시에만 TOML에 emit. 미명시 시 결과 TOML에 `branch` 줄 없음 → 다음 `scan` 실행 시 도구 내장 기본값 `main` fallback (`spec-config.md` § 우선순위).
- `--ignore <pattern>` — 옵셔널 반복. 명시된 패턴 모두 TOML 배열로 emit. 미명시 시 결과 TOML에 `ignore` 줄 없음.
- 외부 호출 0 — repo 존재 검증·인증 검사 0. 잘못된 repo가 명시되어도 다음 `scan` 실행 시 자연스럽게 surface (gh CLI 에러 → `GitlessError::Http`).

#### stdout 출력 형식
- 출력은 `spec-config.md` § 스키마와 동일한 TOML 본문. emit 순서: `repo` → `branch` → `ignore` (직렬화 안정성).
- 옵셔널 필드는 `Some` / non-empty 시에만 emit.
- `repo` emit: `repo = "<owner/name>"\n`.
- `branch` emit: `branch = "<name>"\n`.
- `ignore` emit: `ignore = [<quoted, comma+space joined>]\n`. 패턴은 각각 `"..."`로 quote.
- 결과 TOML은 round-trip 안정 — `toml::from_str::<Config>` 파싱 통과 + 모든 필드가 입력과 일치.

#### stderr hint
- 정상 init 실행 시 stderr에 항상 hint 한 줄: `Tip: redirect stdout to ./gitless-sync.toml to persist this config.`
- tty 감지 분기 0 — redirect 여부와 무관하게 항상 emit.

#### redirect 패턴
사용자(또는 Claude Code)가 shell redirect로 영구 파일 생성:
```bash
gitless-sync init --repo owner/name --branch main > gitless-sync.toml
```
도구는 파일을 작성하지 않는다 (ADR 0001 read-only 영구). 같은 디렉토리에 이미 `gitless-sync.toml`이 있으면 shell redirect가 덮어쓴다 — 도구 책임 밖. `--force` / 충돌 처리 / 파일 권한 코드 0 (ADR 0004).

#### exit code
| Code | 의미 | Variant |
|------|------|---------|
| 0 | 정상 emit | `Ok(())` |
| 1 | `--repo` 미명시 (또는 빈 문자열) | `GitlessError::Config("repo not specified")` |

init은 외부 호출이 없으므로 exit code 2 / 3 / 4 / 5는 발생하지 않는다.

### 인자 우선순위
CLI > env > `gitless-sync.toml` > 도구 내장 기본값. 자세한 건 `spec-config.md`.

## Acceptance Criteria
- `[AUTO]` `cargo run -- --help`가 위 모든 플래그를 보여준다.
- `[AUTO]` `cargo run -- --help` stdout에 `scan` / `diff` 서브커맨드 description 한 줄 이상 노출 (cli-ux-feedback.md § F1).
- `[AUTO]` `cargo run -- scan --help`가 `--summary-only`, `--status`를 추가로 보여준다.
- `[AUTO]` `cargo run -- diff --help`가 `<path>` positional 인자를 보여준다.
- `[AUTO]` `cargo run -- init --help`가 `--repo`, `--branch`, `--ignore`를 보여준다 + `after_help` / `long_about`에 redirect 예시 한 줄 노출.
- `[AUTO]` `--ignore` 플래그를 두 번 이상 지정하면 `Vec<String>`에 누적된다 (clap 기본 동작).
- `[AUTO]` `--status drift,local_only_changed` 같은 콤마 구분 입력이 `Vec<Status>`로 파싱된다.
- `[AUTO]` clap이 알 수 없는 플래그를 받으면 비-zero 종료 + stderr에 사용법 출력 (clap 기본).
- `[AUTO]` `cargo run -- init --repo a/b`가 stdout에 `repo = "a/b"\n` 한 줄 emit + exit code 0 + stderr hint 한 줄.
- `[AUTO]` `cargo run -- init --repo a/b --branch dev`가 stdout에 `repo = "a/b"\n` + `branch = "dev"\n` 두 줄 emit (이 순서).
- `[AUTO]` `cargo run -- init --repo a/b --ignore "dist/" --ignore "*.tmp"`가 stdout에 `repo = "a/b"\n` + `ignore = ["dist/", "*.tmp"]\n` emit.
- `[AUTO]` `cargo run -- init` (--repo 미명시) → exit code 1, stdout 출력 0, stderr `error_code: "CONFIG"` JSON 한 줄.
- `[AUTO]` init stdout 출력이 `toml::from_str::<Config>` 파싱 통과 + repo/branch/ignore 모든 필드가 입력 인자와 일치 (round-trip).
- `[AUTO]` `cargo run -- diff --help`에 `--json` 플래그가 표시됨 (task N 구현 후).
- `[AUTO]` `diff <path>` (--json 미명시) → 기존 unified text stdout 또는 side marker stderr 동작 유지.
- `[AUTO]` `diff <path> --json` → stdout 한 줄 JSON, stderr side marker 0 bytes.
- `[AUTO]` `diff <path> --json` + local-only → `{"side":"local_only","unified":null,"raw":"<content>","binary":false}`.
- `[AUTO]` `diff <path> --json` + both normalize-equal → `{"side":"both","unified":"","raw":null,"binary":false}`.
- `[AUTO]` `diff <path> --json` + both normalize-diff → `{"side":"both","unified":"<diff text>","raw":null,"binary":false}` (unified field에 unified diff 텍스트 포함).
- `[AUTO]` `diff <path> --json` + binary → `{"side":"<side>","unified":null,"raw":null,"binary":true}`.
