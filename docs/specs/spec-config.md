# Spec: Configuration Loading

## 목적
설정 값(repo, branch, ignore 패턴)을 3단계 우선순위로 결정. CLI > `gitless-sync.toml` > 도구 내장 기본값. 인증 토큰은 본 도구의 책임이 아니다 (ADR 0002 — `gh auth login` 위임).

## 현재 상태
- `crates/gitless-sync/src/shared/config.rs::Config` 구조체 + serde 정의 + `load(path)` 구현 완료.
- 인증 토큰 입출력 경로(`--token`, `GITHUB_TOKEN` env, `resolve_token` 헬퍼)는 모두 제거됨 (M3, ADR 0002).

## 작업 범위

### 우선순위 (높음 → 낮음)
1. **CLI 인자** (`--repo`, `--branch`, `--ignore`, `--keep-bom`, `--pretty`)
2. **`gitless-sync.toml`** (프로젝트 루트에 있으면 자동 로드)
3. **도구 내장 기본값** (예: branch=`main`, local=`.`)

### `gitless-sync.toml` 스키마
```toml
repo = "owner/name"
branch = "main"
ignore = ["dist/", "*.tmp"]
```
모든 필드는 옵셔널. 누락 시 다음 우선순위로 fallback.

`gitless-sync init`은 본 스키마를 stdout TOML로 emit하는 도구다 — 사용자가 shell redirect로 영구 파일 생성 (ADR 0004). 자세한 정의는 `spec-cli-interface.md` § init subcommand.

### 비밀 정보 정책
토큰 같은 비밀 정보는 도구 코드·repo에 절대 포함하지 않음. `gitless-sync.toml`에 토큰 필드 정의하지 않음 (commit 위험). 인증은 외부 `gh` CLI에 위임 (`gh auth login`) — 본 도구는 토큰 문자열을 받지도 출력하지도 않는다.

### Cache (Phase 4)

> **신설 (Phase 4 P2, 2026-05-07, ADR 0009)**: Phase 4 mtime 기반 SHA cache는 도구 internal metadata. read-only 본성은 "user 데이터·원격 보존"이 본질이므로 cache는 예외. 자세한 근거: ADR 0009.

#### 위치

- 디렉토리: `dirs::cache_dir() + "gitless-sync/"`.
  - Linux/macOS: `$XDG_CACHE_HOME/gitless-sync/` 또는 `~/.cache/gitless-sync/`.
  - Windows: `%LOCALAPPDATA%\gitless-sync\`.
- 사용자 working directory / repo 내부 / vault 내부에는 **절대 작성 안 함** (ADR 0009 § Decision §1).
- 사용자가 `.gitignore`에 박을 필요 0 — repo 외부 OS user-cache 디렉토리에 위치하므로 git에 포착되지 않음.

#### 파일명

- repo+branch별 파일 분리: `<owner>__<repo>__<branch>.json`.
- filesystem-safe sanitize:
  - `/` → `__`
  - Windows reserved 문자(`<`, `>`, `:`, `"`, `\`, `|`, `?`, `*`)는 `_`로 치환 또는 제거.
- 예: `KneShell/gitless-sync` + `main` → `KneShell__gitless-sync__main.json`.

#### JSON 형식

```json
{
  "version": 1,
  "entries": {
    "path/to/file.md": {
      "mtime": "2026-05-07T12:34:56.789Z",
      "sha": "abc123..."
    }
  }
}
```

- `version`: 현재 `1`. 미래 schema 변경 시 증가. version 미스매치 시 통째 reset (default Cache 반환).
- `entries[path]`:
  - `mtime`: 로컬 파일 mtime (ISO-8601 UTC).
  - `sha`: 도구 자체 정의 SHA (LF-normalized + BOM-stripped, `spec-hash-and-normalize.md` 참조 — git 표준 blob SHA가 아님, G-001).
- path 키는 G-004 일관 forward slash 정규화.

#### Lifecycle

- **load**: scan 시작 시 `Cache::load(cache_path(repo, branch))`. parse 성공이면 반환, 실패(미존재/parse 에러/version 미스매치)면 `Cache::default()` + stderr warning 1줄 (`cache reset: <reason>`). graceful fallback (return type: `Cache`, not `Result`).
- **lookup**: 파일 walk 후 `cache.lookup(path, mtime)` → entry 존재 + mtime 일치 시 cached SHA 반환, 그 외 None.
- **insert**: cache miss 시 `hash::compute_blob_sha(path)` + `cache.insert(path, mtime, sha)`.
- **save**: scan 종료 직전 `cache.save(cache_path)`. atomic write (`<path>.tmp` → rename). 디렉토리 미존재 시 `create_dir_all`. write 권한 부족 시 `GitlessError::Io` 반환 → main.rs에서 stderr warning 처리, scan 결과 영향 0.

#### Graceful fallback

- **손상** (parse 실패): 통째 reset (default Cache + warning). scan 결과 정합성 영향 0 (1차 scan과 동일 timing).
- **권한 부족**: lookup은 가능 (default Cache 반환), save 실패 시 warning + scan 결과 정상.
- **소실** (cache 파일 미존재): cache miss → 1차 scan과 동일 timing. ScanReport 영향 0.

#### 호출자 contract 영향

- 결과 ScanReport JSON identical — cache 유/무로 결과 차이 0.
- LLM 호출자는 cache 유/무 인지 부담 0.
- 사용자가 cache 통째 reset 원할 시 user-cache 디렉토리의 해당 파일 수동 삭제. 도구 측 reset CLI flag 미제공 (yagni). P9 사용자 피드백 후 검토 가능.

#### 의존성

- `dirs` crate (P4에서 추가). 라이선스: MIT. `deny.toml` 화이트리스트 갱신 가능성 — P8 cargo deny check에서 surface.

#### 임계값 (P7b ADR 0008 결정용)

- P6c에서 cache hit speedup 측정 (1차 scan / 2차 scan timing ratio).
- 임계값 (§ Phase 4 사전 결정 §15):
  - **유지** (≥ 2x): cache 본문 그대로 + ADR 0008 confirm.
  - **제거** (< 1.5x): cache.rs 통째 삭제 + `dirs` 의존성 제거 + ADR 0009 obsolete 마크.
  - **경계** (1.5~2.0x): yagni 일관 시 제거 default. raw data로 ADR 0008에 근거 박음.

## Acceptance Criteria
- `[AUTO]` `config::load(Some(path_to_toml))`가 정상 TOML 파일을 파싱한다.
- `[AUTO]` `config::load(None)` 또는 파일 없는 경로 → `Config::default()` 반환 (필드 모두 None / 빈 Vec).
- `[AUTO]` `gitless-sync.toml` 파싱 에러 시 → `GitlessError::Config(...)`, exit code 1.
- `[AUTO]` 우선순위 검증: CLI `--repo "a/b"` + toml `repo="e/f"` → 결과는 `a/b` (CLI 승리).
- `[AUTO]` `repo` 필드가 모든 소스에서 누락 시 → `GitlessError::Config("repo not specified")`, exit code 1.

### Cache (Phase 4)

- `[AUTO]` `Cache::load(path_to_existing_cache)` parse 통과 → entries 정상 매핑.
- `[AUTO]` `Cache::load(path_to_corrupted_cache)` → `Cache::default()` + stderr warning 1줄 (graceful, return type `Cache` not `Result`).
- `[AUTO]` `Cache::load(path_to_missing_file)` → `Cache::default()` (warning 없거나 약식, scan 정상 진행).
- `[AUTO]` `Cache::lookup(path, mtime)` mtime 일치 시 cached SHA 반환, 불일치 시 None (mtime 변경 invalidate).
- `[AUTO]` `Cache::lookup` cache miss → None.
- `[AUTO]` `Cache::save` atomic write — `<path>.tmp` → rename, 이전 파일 손상 0.
- `[AUTO]` `Cache::save` 디렉토리 미존재 시 자동 `create_dir_all`.
- `[AUTO]` `Cache::save` 권한 부족 시 `GitlessError::Io` 반환 → main.rs warning + scan 결과는 정상 출력.
- `[AUTO]` `cache_path("KneShell/gitless-sync", "main")`이 `dirs::cache_dir() + "gitless-sync/KneShell__gitless-sync__main.json"`에 매핑 (sanitize 룰 적용).
- `[AUTO]` cache `version` 미스매치 (예: cached version 2 만남) → `Cache::default()` 반환 + warning.
- `[AUTO]` cache 위치는 `dirs::cache_dir()` 하위 — 사용자 working directory / repo 내부 / vault 내부에 절대 작성 안 함 (ADR 0009 § Decision §1).
