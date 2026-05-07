# ADR 0009: read-only 본성 명확화 — internal cache는 예외

- **Status**: Accepted
- **Date**: 2026-05-07
- **Related**: ADR 0001 (read-only 영구), ADR 0004 (init stdout TOML — 도구 파일 작성 0), `docs/specs/spec-config.md`, `CLAUDE.md` § Critical Rules § 도구 본성, `docs/ralph/implementation-plan.md` § Phase 4 사전 결정 §8~10

## Context

ADR 0001 § D2와 ADR 0004는 read-only 본성을 다음과 같이 박았다:
- ADR 0001: "도구는 파일·원격을 절대 수정하지 않는다. write 작업은 Claude Code가 `gh`로 직접 처리하므로 별도 push 도구를 만들지 않는다."
- ADR 0004: "도구는 파일을 작성하지 않는다. 사용자가 shell redirect로 영구 파일을 생성한다."

이 표현은 두 시점 모두 "user 데이터·원격 변경 0"이라는 본질을 강조하기 위함이었다. push 도구 / init 파일 작성은 둘 다 사용자가 직관적으로 "도구가 만든 결과물"로 인식하는 객체이고, 그 결과물이 실수로 user state(commit 히스토리, 디렉토리의 toml)를 덮어쓰는 risk가 본 도구의 신뢰성과 상충한다.

Phase 4에서 도입하려는 mtime cache(P4 task)는 본질이 다르다.
- **저장 위치**: `dirs::cache_dir() + "gitless-sync/"` (Linux/macOS `$XDG_CACHE_HOME` 또는 `~/.cache/`, Windows `%LOCALAPPDATA%`). 사용자 디렉토리 / repo 내부 / vault 내부 0.
- **저장 내용**: `<path>` → `(mtime, self-hash)` 매핑. 도구 자체 metadata.
- **사용자 데이터 영향**: 0. 사용자가 cache 파일을 모르고 살아도 도구 사용에 마찰 0 (graceful fallback).
- **소실 영향**: cache miss → 1차 scan과 동일 timing. 결과 정합성 영향 0.
- **LLM 호출자 영향**: 0. ScanReport JSON identical.

이 본성은 write 도구 / init 파일과 결정적으로 다르다. 그러나 ADR 0001/0004의 표현 "파일을 절대 수정하지 않는다"가 문자 그대로 적용되면 cache도 금지 대상이 된다. 표현이 본질을 과대 표현해서 발생하는 stale 케이스다.

## Decision

read-only 본성의 정의를 "**user 데이터·원격 보존**"으로 명확화한다. 도구가 자체 internal metadata(cache 등)를 OS user-cache 디렉토리에 저장·관리하는 것은 본 본성의 예외다.

명확화의 구체 기준:
1. **위치**: OS user-cache (`dirs::cache_dir()`) 하위. 사용자 working directory / repo 내부 / vault 내부에 절대 작성 안 함.
2. **내용**: 도구 자체 metadata. 사용자 데이터(파일 본문/원격 commit 등)의 사본·요약 0.
3. **소실 graceful**: cache 손상/미존재/parse 실패 시 통째 reset. 사용자 마찰 0(stderr warning 1줄 제외).
4. **호출자 contract 영향**: ScanReport JSON identical. cache 유/무로 결과 차이 0.
5. **권한 부족 graceful**: write 권한 없으면 warning + scan 정상 진행. 도구 종료 0.

cache 위치는 repo+branch별 파일 분리 — `<user-cache>/gitless-sync/<owner>__<repo>__<branch>.json` (filesystem-safe sanitize: `/` → `__`, 기타 특수문자 제거). vault iCloud sync 충돌 회피 + 사용자 .gitignore 박을 필요 0. 의존성으로 `dirs` crate 1개 추가(P4).

## Consequences

### `CLAUDE.md` § Critical Rules § 도구 본성 (본 task에서 갱신)
- 본문 한 줄 명확화: "Read-only는 **user 데이터·원격 보존**이 본질. Internal cache는 예외 (ADR 0009)."
- 기존 "도구는 파일·원격을 절대 수정하지 않는다" 표현은 그대로 유지(원칙 표현). 명확화 줄이 부속.

### `spec-config.md` § cache (P2에서 추가)
- cache 위치 / 파일명 sanitize 룰 / JSON 형식 / lifecycle / graceful fallback 박음.
- ADR 0009 cross-ref. 사용자 .gitignore 박을 필요 0 명시.

### `Cargo.toml` (P4에서 추가)
- `dirs = "5"` (또는 최신 안정) 추가. `Cargo.lock` 갱신.
- 라이선스: MIT. `deny.toml`에 화이트리스트 갱신 필요 가능성 — P8 cargo deny check에서 surface.

### 코드 (P4에서 박음)
- `crates/gitless-sync/src/shared/cache.rs` 신규. `Cache::load`, `Cache::save`, `Cache::lookup`, `Cache::insert`, `cache_path`.
- scan 진입점에서 cache load → walk + hash 시 lookup → scan 종료 직전 save. fail graceful.

### 사용자 운영
- cache 파일은 OS user-cache라 사용자가 직접 보거나 .gitignore 박을 일 0.
- 디스크 사용량은 path 수에 비례 (한 entry ~100 bytes 추정, 1000 path = ~100KB).
- 사용자가 cache 통째 reset 원할 시 user-cache 디렉토리의 해당 파일 수동 삭제 — 도구 측 reset CLI flag 미제공(yagni). P9 사용자 피드백 후 검토 가능.

### Phase 5 향후
- cache 효과가 P6c 측정에서 임계값 미달이면 ADR 0008에서 cache 제거 결정. 본 ADR 0009도 obsolete 마크 (P7b에서 처리).
- 효과가 충분하면 ADR 0008에서 confirm + 본 ADR이 그대로 active.

## References

- ADR 0001 (`docs/adr/0001-gh-subprocess-and-drop-push-tool.md`) § D2 (read-only 영구)
- ADR 0004 (`docs/adr/0004-init-stdout-redirect.md`) § Decision (도구 파일 작성 0)
- `docs/specs/spec-config.md` § cache (P2 추가 대상)
- `CLAUDE.md` § Critical Rules § 도구 본성 (본 task 갱신)
- `docs/ralph/implementation-plan.md` § Phase 4 사전 결정 §8 (Cache 본성), §9 (Cache 위치), §10 (Cache 형식)
- `dirs` crate: <https://crates.io/crates/dirs>
