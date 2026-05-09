# Spec: Configuration Loading

## 목적
설정 값(repo, branch, ignore 패턴)을 3단계 우선순위로 결정. CLI > `gitless-sync.toml` > 도구 내장 기본값. 인증 토큰은 본 도구의 책임이 아니다 (ADR 0002 — `gh auth login` 위임).

## 현재 상태
- `crates/gitless-sync/src/shared/config.rs::Config` 구조체 + serde 정의 + `load(path)` 구현 완료.
- 인증 토큰 입출력 경로(`--token`, `GITHUB_TOKEN` env, `resolve_token` 헬퍼)는 모두 제거됨 (M3, ADR 0002).

### L1-task audit (2026-05-09)

spec § `.gitattributes` 위치 정책 (Phase 5) ↔ K1 구현 (`shared/gitattributes.rs`) ↔ caller (`commands/scan/mod.rs::scan`) 정합 audit.

**구현 정합**:
- working tree 한정 — `gitattributes.rs:76 GitAttributes::load(root)`가 `WalkDir::new(root)`로 root 진입점을 강제. caller `commands/scan/mod.rs:93`에서 `Arc::new(GitAttributes::load(local_root)?)` 호출 — vault local root 1회 (spec-hash-and-normalize.md § Lifetime 계약 정합). `pub fn load(root: &Path)` 시그니처는 root 외부 진입점을 컴파일러 차원에서 차단.
- `.git/info/attributes` 미지원 — `gitattributes.rs:81 is_dot_git_dir`가 `.git` 디렉토리 자체를 `WalkDir::filter_entry`로 skip. unit test `gitattributes_tests.rs::dot_git_directory_is_skipped` (line 110-117)에서 `.git/.gitattributes` 케이스 무시 검증.
- global `~/.gitconfig` / `~/.config/git/attributes` 미지원 — 구현에 `home_dir` / `HOME` / `XDG_CONFIG_HOME` / `gitconfig` ref **0건** (Grep 검증). `dirs` crate / `std::env::home_dir` 사용 안 함 — 자동 미지원.
- macro attributes pattern 매칭만 지원 — line-level 매칭은 통과. attribute level은 K1.5 화이트리스트 외 attribute를 `AttributeMatch::Unsupported { attribute_name }` variant로 분류 (`gitattributes.rs:164 classify_raw_attributes` + `gitattributes.rs:186 whitelist_match`).

**Drift surface (0건, advisor BLOCKING fix)**:
- 초기 audit에서 macro attribute *정의 line* (`[attr]binary -text -diff -merge`) graceful skip 누락을 drift surface로 기록했으나 — advisor BLOCKING fix로 phantom drift 정정 (오류). 정확 trace: `[attr]binary`는 gitignore-style glob character class (`[abc]` 형식) — `{a,t,r}` 문자 집합 + literal `binary`로 분류되어 ignore crate `GitignoreBuilder::add_line`가 valid glob pattern으로 통과. attributes 토큰(`-text -diff -merge`)은 K1.5 화이트리스트 외라 `AttributeMatch::Unsupported { attribute_name: "text" }` (첫 unsupported, `classify_raw_attributes:172`)로 분류. **0 drift** — spec § 미지원 § macro attributes line ("pattern 매칭만 지원") ↔ K1 구현 정확 정합.

**Spec self-consistency (정합)**:
- spec-domain-pitfalls.md § `.gitattributes` 화이트리스트 § 파서 line 정합 ("working tree 한정 (.gitattributes 파일). `.git/info/attributes` / global 미지원 (spec-config.md § `.gitattributes` 위치)").
- spec-hash-and-normalize.md § `.gitattributes` 정확 재현 § 파서 line 정합 ("**working tree 한정** (`.gitattributes` 파일). `.git/info/attributes` / global 미지원 (spec-config.md § `.gitattributes` 위치)").

**validation**: cargo fmt clean (spec-only, G-012 적용). 코드 변경 0 — baseline 유지.

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

### `.gitattributes` 위치 정책 (Phase 5)

도구가 자동 로드하는 `.gitattributes` 위치 — **working tree만**:
- project root `.gitattributes`
- 하위 디렉토리 `.gitattributes` (가장 깊은 게 우선)

**미지원** (Phase 5 영구 비목표):
- `.git/info/attributes` — local git config, working tree 외부.
- global `~/.gitconfig` 또는 `~/.config/git/attributes` — user-level, working tree 외부.
- macro attributes 정의 (예: `[attr]binary -text -diff -merge`) — pattern 매칭만 지원. `[attr]binary`는 glob character class `{a,t,r}` + literal `binary`로 분류되어 ignore crate가 valid pattern으로 통과. attributes 토큰(`-text -diff -merge`)은 화이트리스트 외 → K1.5 `AttributeMatch::Unsupported` variant로 분류 (매칭 path가 있을 때).

근거: read-only 도구 본성 — `.git/` 폴더 자체를 안 읽는다 (gitless 환경). working tree만 보는 게 정합. 미지원 위치에 정의된 attribute는 Phase 5 화이트리스트(text/binary/eol=lf|crlf) 외라 자동 무시 + `failed_reason: "gitattributes_unsupported"` 마크 가능 (spec-domain-pitfalls.md § `.gitattributes` 화이트리스트).

### Cache (Phase 4) — 제거됨 (ADR 0008, 2026-05-07)

Phase 4 P4에서 도입했던 mtime 기반 SHA cache는 P6c 측정에서 speedup ≈ 1.0x (noise floor 안쪽)로 § Phase 4 사전 결정 §15 임계값 < 1.5x 제거 영역에 떨어졌다. ADR 0008로 본 도구는 cache를 보유하지 않는다. cache 위치/형식/lifecycle 정의도 함께 obsolete.

## Acceptance Criteria
- `[AUTO]` `config::load(Some(path_to_toml))`가 정상 TOML 파일을 파싱한다.
- `[AUTO]` `config::load(None)` 또는 파일 없는 경로 → `Config::default()` 반환 (필드 모두 None / 빈 Vec).
- `[AUTO]` `gitless-sync.toml` 파싱 에러 시 → `GitlessError::Config(...)`, exit code 1.
- `[AUTO]` 우선순위 검증: CLI `--repo "a/b"` + toml `repo="e/f"` → 결과는 `a/b` (CLI 승리).
- `[AUTO]` `repo` 필드가 모든 소스에서 누락 시 → `GitlessError::Config("repo not specified")`, exit code 1.
