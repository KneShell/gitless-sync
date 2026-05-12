# CLI UX Feedback (도그푸딩 노트)

이 문서는 vault 측에서 gitless-sync를 실제로 호출하며 관측한 CLI 표면 위주의 거친 모서리를 기록한다. 향후 surface 개선 작업 시 참조.

## 세션 메타

- 일자: 2026-05-12
- 버전: v0.4.2 (commit `5f2f439` 기준 origin/main HEAD, sha1 0.11.0 bump 포함 빌드)
- 호출자: Claude Code (LLM 호출자)
- 시나리오: `init` → `gitless-sync.toml` 작성 → `scan --summary-only --pretty` → `scan --status local_only_changed,remote_only_changed --pretty`
- 대상 repo: vault ↔ `KneShell/obsidian-vault@main`

## Improvement 후보

### F1. `scan` / `diff` 서브커맨드 `--help` description 비어 있음

**현재**

```
Commands:
  scan
  diff
  init  Print a gitless-sync.toml template to stdout (you redirect to a file)
  help  Print this message or the help of the given subcommand(s)
```

`init` 한 줄 설명은 있는데 정작 가장 자주 호출되는 `scan` / `diff`는 공란. 최상위 `--help`에서 도구의 본질 동사("로컬↔원격 4분류 비교", "단일 파일 unified diff")가 표면화 안 됨.

**기대**

clap derive의 `#[command(about = "...")]`을 `scan` / `diff` 서브커맨드에도 추가. 한 줄 요약이면 충분:

- `scan` — "Compare local directory against remote repo, emit 4-state classification JSON"
- `diff` — "Show unified diff (or JSON) of a single file vs remote"

**위치**: `crates/gitless-sync/src/main.rs` clap derive 정의부 (서브커맨드 enum variant).

**Acceptance**: `spec-cli-interface.md § Acceptance Criteria`에 `[AUTO] cargo run -- --help` 검증 줄에 "서브커맨드별 description 한 줄 이상 노출" 추가.

### F2. `init` 서브커맨드 `--help` 문구가 "template printer" 인상

**현재**

```
init  Print a gitless-sync.toml template to stdout (you redirect to a file)
```

"template"이라는 단어가 인자 없이도 placeholder 토대로 출력 가능하다는 인상을 준다. 실제로는 `--repo` 미명시 시 `CONFIG_ERROR`로 거절 (ADR 0004 + spec-cli-interface.md § init subcommand 의도된 설계 — "toml 파일을 만드는 도구이므로 자기 자신을 입력 소스로 쓸 수 없음").

처음 사용자(또는 처음 LLM 호출 세션)는 `init`만 쳤다가 에러를 받고 "왜 template인데 입력이 필요하지?" 잠깐 헷갈린다.

**기대**

문구를 의도에 맞게 정밀화:

- "Emit gitless-sync.toml body from input args (stdout)" 또는
- "Compose gitless-sync.toml body from --repo/--branch/--ignore, emit to stdout"

"template printer"가 아니라 "input args로 본문 만드는 도구"임을 한 줄에 드러낸다.

**위치**: `crates/gitless-sync/src/main.rs` init 서브커맨드 clap derive.

**Acceptance**: `spec-cli-interface.md § init subcommand`에 `--help` 문구 형식 acceptance 한 줄 추가 (현재는 인자/exit code만 박혀 있음).

### F3. `--summary-only` 시 `failed` 목록만이라도 같이 보고 싶음

**현재**

`scan --summary-only`는 `summary` 객체만 출력. `failed` count가 0보다 크면 어떤 파일이 실패했는지 보려고 `scan --status failed`로 재호출 필요. 큰 vault에서 두 번 scan은 GitHub API 호출 부담 (Trees API + 필요 시 blob fetch).

**기대 (안 두 가지)**

- (a) `--summary-only`에 `failed` 한정 `files[]` 항상 포함 (다른 status는 여전히 omit). failed는 보통 handful이라 summary 크기 거의 그대로.
- (b) `--include-failed-files` opt-in flag 추가. summary-only 호환.

(a)가 호출자(LLM) 입장에서 한 호출로 충분한 정보 — 권장.

**근거**: failed는 cascade priority(spec-classification § Cascade priority)로 분류되고, 호출자는 곧바로 다음 액션(skip / re-attempt / spec.gitattributes 갱신)을 결정해야 함. 한 번 더 scan 도는 cost가 information value 대비 큼.

**위치**: `crates/gitless-sync/src/commands/scan/render.rs` (또는 출력 직렬화 모듈) + `spec-output-schema.md` § scan output § summary-only 분기.

**Acceptance**: `spec-output-schema.md`에 "summary-only 모드라도 failed status entries는 files[]에 emit" 명시. 단위 테스트는 `commands/scan/tests::summary_only_includes_failed`.

## Good (유지)

### G1. JSON error 포맷 일관성

`{"error_code":"CONFIG_ERROR","message":"Configuration error: repo not specified"}` 같은 형식. exit code + error_code + 사람용 message 3축 분리. AI 호출자가 파싱 안정. `spec-error-contracts.md` 정합 확인됨.

### G2. `presence` vs `status` 두 축 직교 분리

`presence: local_only / both / remote_only`와 `status: identical / local_only_changed / remote_only_changed / drift / failed`가 분리 emit. 단일 `status` 라벨에 "원격에 없음 + 변경"과 "양쪽 있고 변경"을 묶지 않고 presence로 분간하게 둔 게 호출자 입장에서 깔끔.

예: `local_only_changed` + `presence: local_only` = 신규 추가 파일 / `local_only_changed` + `presence: both` = 양쪽 있는데 로컬이 더 최신. 호출자는 두 케이스에서 다른 액션(create vs update)을 결정. spec-classification § 상태 정의의 통합 라벨 정책과 정합.

### G3. `diff_meaningful` 필드

normalize-equal cosmetic drift(BOM/encoding/LF-CRLF) 케이스에서 SHA는 다르지만 normalize 후 동일임을 별도 마킹. v1.4 schema fix(#6) 의도 그대로. 호출자가 false positive 동기화 트리거를 피하기 쉬움.

### G4. stdout / stderr 분리 정확

`init` 호출 시 `repo = "..."` TOML 본문은 stdout, `Tip: redirect stdout to ./gitless-sync.toml ...` hint는 stderr. `> gitless-sync.toml` redirect 시 파일에는 TOML만, 콘솔에는 Tip만 남아 unix tool 관행과 정합. ADR 0004 의도 그대로 구현됨.

## 정정 (사실 검증 후 폐기)

이번 도그푸딩 세션 초기 메모에서 다음 항목들은 사실관계 검증 후 폐기. 향후 같은 오해를 반복하지 않기 위한 기록.

- ~~"Tip 라인이 stdout으로 섞여 `> gitless-sync.toml` redirect 시 invalid TOML이 됨"~~
  - 실제: 명세대로 stderr 분리 (G4 참조). 초기 관측은 bash 호출 도구가 stdout/stderr를 합쳐서 보여줘서 발생한 오해. `2>/dev/null` / `1>/dev/null` 분리 호출로 검증 완료.
- ~~"`init`이 `--repo` 강제하는 건 template printer 광고와 인지부조화"~~
  - 실제: ADR 0004 + spec-cli-interface.md § init subcommand에 명시된 의도된 설계 ("toml 파일을 만드는 도구이므로 자기 자신을 입력 소스로 쓸 수 없음" — fallback 소스 env / toml 없이 인자만 본다). 인지부조화의 원인은 `--help` 문구(F2)이지 `--repo` 필수성 자체가 아님.
- ~~"`local_only_changed` 라벨이 새 파일(presence: local_only)에 어색"~~
  - 실제: spec-classification § 상태 정의에 명시된 통합 라벨. "원격에 없음" + "원격 있지만 로컬이 더 최신" 두 케이스를 같은 status로 통합하고 호출자가 presence로 분간하라는 의도 (G2 참조). 라벨 변경은 schema breaking change 가치 없음.

## 관련 명세

- `docs/adr/0004-init-stdout-redirect.md` — init stdout TOML + stderr Tip 패턴
- `docs/specs/spec-cli-interface.md` — CLI surface 정의, 서브커맨드 인자/플래그
- `docs/specs/spec-classification.md` — 4-state 분류 + presence/status 직교 축
- `docs/specs/spec-output-schema.md` — scan/diff JSON 출력 schema (F3 영향 범위)
- `docs/specs/spec-error-contracts.md` — error_code / exit code 매핑 (G1 정합)
