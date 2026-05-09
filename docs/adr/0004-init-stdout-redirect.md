# ADR 0004: `gitless-sync init`은 stdout TOML 출력 + redirect 패턴

- **Status**: Accepted
- **Date**: 2026-05-07
- **Resolves**: `docs/roadmap.md` § Phase 2 init 출력 방식 미정 (사용자 결정 2026-05-07)
- **Related**: `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` (read-only 영구), `docs/specs/spec-cli-interface.md`, `docs/specs/spec-config.md`

## Context

`docs/roadmap.md` § Phase 2 원안은 `gitless-sync init`이 "현재 디렉토리에 `gitless-sync.toml` 작성. 기존 파일 있으면 `--force` 없이 실패."로 명시돼 있었다. 그러나 이 원안은 ADR 0001 § D2(`gitless-push` 폐지 + read-only 영구) 결정과 정면 충돌한다 — `init` 역시 파일을 쓰는 순간 도구는 더 이상 read-only가 아니다. roadmap이 ADR 0001보다 먼저 작성됐기 때문에 발생한 stale 항목.

세 옵션을 평가했다.

| 옵션 | 도구 파일 작성 | `--force` / 충돌 처리 코드 | ADR 0001 정합 | 사용자 마찰 |
|---|---|---|---|---|
| (A) stdout TOML + redirect | 0 | 0 | 100% | shell redirect 1회 |
| (B) dry-run 기본 + `--write` flag | 옵셔널 | 부분 (write 분기) | 80% (write 분기 존재) | 동일 |
| (C) 원안 — 직접 파일 작성 + `--force` | 항상 | 전부 | 0% (위반) | 0 |

(A)는 unix tool 관행(`gh api ... > out.json`, `openssl ... > cert.pem`)과 자연스럽게 일치하고, 도구 코드에서 파일 IO·권한·기존 파일 충돌·`--force` 매트릭스가 전부 사라진다. (B)는 마찰은 같으면서 write 분기를 도구 안에 남기므로 ADR 0001 정합이 부분만 유지된다. (C)는 원안 자체가 ADR 0001 위반이라 후보에서 탈락.

## Decision

`gitless-sync init`은 입력 인자에서 만든 TOML 본문을 stdout으로 출력한다. 도구는 파일을 작성하지 않는다. 사용자가 shell redirect로 영구 파일을 생성한다.

```bash
gitless-sync init --repo owner/name --branch main > gitless-sync.toml
```

스키마는 v0.1 `spec-config.md` § 스키마 그대로 (`repo` / `branch` / `ignore` 3개 필드). 확장 0. emit 순서는 `repo` → `branch` → `ignore` (직렬화 안정성). 옵셔널 필드는 `Some` / non-empty 시에만 emit.

`--repo` 미명시 시 `GitlessError::Config("repo not specified")`, exit 1, stderr `error_code: "CONFIG"`. repo 존재 검증·외부 호출 0 — 잘못된 repo가 들어 있어도 다음 `scan` 실행 시 자연스럽게 surface.

정상 init 실행 시 stderr에 항상 hint 1줄: `Tip: redirect stdout to ./gitless-sync.toml to persist this config.` tty 감지 분기 0.

## Consequences

### 코드
- `--force` / `--write` / 기존 파일 충돌 / 파일 권한 처리 코드 0.
- `commands/init/run`은 `&mut impl std::io::Write`를 받는 단일 함수. stdout 인젝션으로 단위 테스트 + 통합 테스트 모두 같은 시그니처로 검증.
- `tty 감지` 분기 0. `atty` / `is-terminal` crate 도입 0.

### 문서 / spec
- `CLAUDE.md` § Critical Rules § Read-only 룰 갱신 0 (이미 영구). § Current State + § 사용자 취향 결정에 한 줄씩 추가.
- ADR 0001 갱신 0 (read-only 영구가 그대로 유효).
- README + `--help`(clap `after_help`) + stderr hint 보강 필요 → P6에서 처리.
- `docs/roadmap.md` § Phase 2 원안 ("현재 디렉토리에 작성 + `--force`")는 P2에서 stdout redirect로 갱신, 실패 모드 (`--force` / 파일 권한 / 기존 파일 충돌) 항목은 obsolete로 제거.
- `spec-cli-interface.md` / `spec-config.md` / `spec-error-contracts.md`는 P2에서 init 정의 추가.

### 운영
- 사용자 마찰: shell redirect 1회. unix 관행에 일치하여 학습 비용 0.
- Claude Code 호출 마찰: `gitless-sync init --repo ... > path` 한 줄. JSON/TOML 파싱 0 (그대로 파일).
- 잘못된 repo가 toml에 들어 있어도 다음 `scan`에서 자연스럽게 에러로 surface (gh CLI 에러 + `GitlessError::Http` 매핑).

## References

- ADR 0001 (`docs/adr/0001-gh-subprocess-and-drop-push-tool.md`) § D2 read-only 영구
- `docs/specs/spec-cli-interface.md` (P2에서 init subcommand 추가)
- `docs/specs/spec-config.md` § 스키마 (v0.1 그대로 재사용)
- 사용자 결정 회의 (2026-05-07, 본 세션)
- unix CLI 관행: `gh api ... > out.json`, `openssl req ... > cert.pem`, `kubectl get ... -o yaml > deploy.yaml`
