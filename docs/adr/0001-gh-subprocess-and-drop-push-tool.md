# ADR 0001: gh CLI subprocess 채택 + `gitless-push` 폐지

- **Status**: Accepted
- **Date**: 2026-04-30
- **Supersedes**: `docs/roadmap.md` § Phase 3 (이전 안), § Phase 4 § gh subprocess 회고 (미결 항목)
- **Related**: `docs/ralph/implementation-plan.md` T13 (obsoleted)

## Context

`gitless-sync`의 본질은 **LLM(특히 Claude Code)이 확률성 없이 호출할 수 있는 정량적 drift 보고 도구**다. 사용자가 매번 "전수조사해서 드리프트 확인해줘"를 자연어로 반복 지시하던 마찰을, 결정론적 CLI 호출로 대체하는 것이 핵심 use case다.

이 본질을 기준으로 두 가지를 재검토했다:

### 1. HTTP 호출 방식 (ureq vs gh subprocess)

v0.1는 6인 페르소나 tribunal이 4기준(우아함 / 속도 / GitHub 친화성 / v0.1 정신)으로 평가해 `ureq` 직접 호출을 채택했다. 그러나 회고 시점에 **"Claude Code 친화성"이 핵심 기준임에도 평가에서 누락**됐음이 드러났다 (2026-04-29 사용자 지적).

| 기준 | ureq 직접 HTTP | gh subprocess |
|---|---|---|
| Claude Code 호출 마찰 | 매번 `--token env:GITHUB_TOKEN` 인자 필요 또는 사전 env setup | `gh auth login` 한 번이면 0-마찰 |
| 인증·rate limit·재시도 | 우리 코드 책임 (`shared/error.rs`, G-003, G-011) | `gh`가 위임 처리 |
| 의존성 | `ureq` crate | 시스템에 `gh` CLI 설치 |
| 도메인 로직 비중 | HTTP 배관층 비중 높음 | 도메인 로직(walk/hash/normalize/4-classify) 비중 ↑ |

"단순 gh wrapper가 되는 것 아니냐"는 우려가 제기됐으나, 도구의 본질(로컬 walk + LF normalize + 자체 blob hash + 4분류 + LLM 친화 JSON)은 gh가 대체할 수 없는 영역임이 확인됐다. gh는 HTTP 배관층 한 줄에 불과하며, 라면집이 밀가루를 직접 제분하지 않는 것과 같다.

### 2. Phase 3 `gitless-push` 존재 의의

원안: scan 결과(JSON)를 받아 GitHub에 실제 push하는 별도 바이너리. read-only 원칙을 지키기 위해 write 책임을 분리.

재검토 결과: **Claude Code는 이미 `gh` 명령으로 push 작업을 잘 수행한다.** drift report만 정상적으로 산출되면, "어떻게 푸시할지"는 LLM의 자연어 처리 영역이지 도구의 영역이 아니다. `gitless-push`를 만드는 것은 LLM이 이미 잘 하는 일을 도구로 굳이 옮기는 over-engineering이다.

## Decision

### D1. Phase 4 GraphQL batching부터 `gh` CLI subprocess 채택

- 신규 GitHub API 호출 코드는 `gh api graphql` 자식 프로세스 호출 + stdout JSON 파싱으로 구현.
- 인증(`gh auth`), rate limit, abuse detection, retry는 `gh`에 위임.
- GraphQL batching 자체는 v0.1 인터페이스(`--backend graphql` flag)를 통해 활성화. 호출자(LLM) 코드 변경 0.

### D2. Phase 3 `gitless-push` 영구 폐기

- `gitless-push` 바이너리를 만들지 않는다.
- v0.1의 "read-only" 제약은 **잠정 분리(Phase 3 후 write 도구 도입)가 아니라 영구 결정**이다.
- push가 필요한 사용자는 Claude Code(또는 사람)가 `gitless-sync scan` 결과를 보고 `gh` 명령으로 직접 처리한다.
- `crates/gitless-push/`는 생성하지 않으며, `crates/shared/` 분리도 불필요.

## Consequences

### 코드 베이스
- Phase 4 신규 코드는 ureq 의존 없이 작성. `std::process::Command`로 `gh` 호출.
- v0.1 기존 ureq 코드는 그대로 둔다 (rest backend는 호환성 유지). 선택적 마이그레이션은 아래 Open Question 참조.
- `shared/error.rs`의 HTTP 관련 variant(`Http`, `RateLimitExceeded` 등)는 v0.1 rest backend에서는 계속 유효, GraphQL backend에서는 `gh` 종료 코드 + stderr 매핑으로 대체.
- G-003 (Commits API 호출 비용), G-011 (rayon 8 concurrent abuse 회피) guardrail은 v0.1 rest backend 한정 유효. gh subprocess 경로에서는 무관.

### 의존성
- `gh` CLI가 PATH에 있어야 함. Claude Code 환경엔 기본 설치되어 있어 마찰 없음. 일반 사용자는 README에 설치 안내 필요.
- gh가 없을 때 명확한 에러 메시지 (e.g. `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")`).

### 문서 / 작업 항목
- T13 (Fine-grained PAT 최소 권한 검증) **obsolete**. `gh`가 인증 책임을 맡으므로 도구가 PAT 권한 가이드를 작성할 필요 없음. `gh auth login` 한 줄로 끝.
- `docs/roadmap.md` Phase 3 섹션 cancelled로 표기.
- `docs/roadmap.md` Phase 4 § gh subprocess 회고는 본 ADR로 이관·해소.
- `CLAUDE.md` Critical Rules § Read-only는 "Phase 3로 분리" 문구 제거, 영구 결정으로 갱신.

### 본질 재확인
- 도구의 가치(walk + normalize + hash + classify + JSON)는 그대로. HTTP 레이어 교체는 본질 영향 0.
- "wrapper화" 우려 해소: 도메인 로직은 그대로 우리 책임.

## Follow-up Open Questions

1. **v0.1 ureq 코드 마이그레이션 시점.** Claude Code 마찰을 즉시 제거하려면 v0.1 scan 명령도 gh subprocess로 전환해야 한다. 점진 전략(Phase 4 신규만 gh) vs 일괄 전환(v0.1 rest backend도 gh로) 선택 필요. 결정 시 별도 ADR.
2. **GraphQL alias batching의 abuse detection 한도.** GitHub은 batching을 공식 권장하지 않으므로 보수적 batch 크기(100~200 alias/request)로 시작하고 운영 데이터로 측정. (`docs/roadmap.md` Phase 4 § GraphQL batching § Caveat)
3. **Phase 5 도메인 함정(NFD/case/encoding/submodule/symlink)** 우선순위는 별도 결정.

## References

- 사용자 결정 회의 (2026-04-30, 본 세션 대화)
- `docs/roadmap.md` Phase 3, Phase 4 § gh subprocess 회고 (이관 대상)
- `CLAUDE.md` Current State (2026-04-29) — 미결정 → 본 ADR로 해소
