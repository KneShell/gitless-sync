# Roadmap (Backlog)

> 이 파일은 ralph가 자동 로드하지 않는다. 사람이 v0.1 완료 후 다음 phase 진입할 때 참조.

## Next Up — v0.1 ureq → gh subprocess 일괄 마이그레이션

> ADR 0002 (2026-05-06) 결정. 상세는 `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md`.

- v0.1 scan/diff REST 호출을 `gh api` subprocess로 전환. ureq + mockito 의존성 제거.
- `--token` 인자 + `resolve_token` 경로 제거. 인증은 `gh auth login`로 단일화.
- 통합 테스트는 mockito 기반에서 gh stub 기반으로 재설계. 전략 결정이 선행 task.
- guardrail G-003 / G-011은 도구 책임 종료. rayon 유지 여부는 마이그레이션 후 측정으로 결정.
- 에러 매핑(`GitlessError::AuthFailed` 등)은 gh 종료 코드 + stderr 파싱으로 재정의. `spec-error-contracts.md` 갱신.

## Phase 2 — 편의 명령어
- `gitless-sync init` — `gitless-sync.toml` 설정 파일 생성 도우미.
  - 입력: `--repo`, `--branch` 등 인자 또는 prompt
  - 출력: 현재 디렉토리에 `gitless-sync.toml` 작성. 기존 파일 있으면 `--force` 없이 실패.
  - 실패 모드: 파일 권한, 기존 파일 충돌.
- `status` 명령어는 만들지 않는다. `scan --summary-only`로 대체.

## Phase 3 — Write 도구 분리 (CANCELLED, ADR 0001)

> **2026-04-30 폐기.** `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` 참조.
>
> push 작업은 Claude Code(또는 사람)가 `gh` 명령으로 직접 처리한다. 도구가 잘 하는 일(drift 정량 보고)과 LLM이 잘 하는 일(자연어 → push 명령)을 굳이 합치지 않는다. `gitless-sync`는 영구 read-only.
>
> 이전 안(historical, 참고용):
> - 별도 바이너리 `gitless-push` (workspace 안에 추가).
> - scan 결과(JSON)를 stdin 또는 파일로 받아 GitHub API로 실제 push 수행.
> - AI가 scan → 사용자 승인 → push로 명시적 단계 분리.

## Phase 4 — 성능 최적화

### 조건부 (측정 후 결정)
- 로컬 SHA mtime 기반 캐시. 큰 vault에서 매번 전체 해시 계산 비용이 문제일 때만 도입.
- v0.1 성능 측정 결과를 보고 도입 여부 결정 (premature optimization 방지).
- Trees API sub-tree 재귀 fallback (truncated repo 지원, G-002 해소).

### 확정 (반드시 도달)

#### GraphQL batching 도입
`fetch_last_commit_at`의 N×round-trip 비효율을 GraphQL alias batching으로 단축. v0.1의 REST + rayon 8 concurrent (1000 path = 25초)를 1~10 round-trip (수 초)로 줄임.

**v0.1 인터페이스 박힘**: `--backend graphql` flag는 v0.1에 stub로 이미 존재 (`spec-cli-interface.md` § Backend 분기, `spec-github-api.md` § Backend 선택). Phase 4에서 backend 본체만 채우면 호출자(LLM) 코드 변경 0. forward-compat 보장.

- **GitHub GraphQL 공식 한도 (fact-checked 2026-04-28)**:
  - 단일 query 최대 **500,000 nodes**
  - rate limit: **5,000 points/hour** + **2,000 points/minute** (per user)
  - alias 자체엔 명시적 한도 없음
  - node 계산: `first`/`last` 인자 곱, 평행 분기 합산
- **우리 use case 분석**: 한 alias = `history(first: 1, path: ...)` = 1 node. 1000 path = 1000 node = 한도의 0.2%. 한 request에 다 박아도 node 한도 기준 OK.
- **권장 batch 크기**: 보수적으로 100~200 alias/request. 1000 drift = 5~10 round-trip.
- **GraphQL endpoint**: `https://api.github.com/graphql`. 인증은 동일 PAT.
- **Caveat**: GitHub은 batching을 공식 권장하지 않음 ("polling 대신 webhook events"). 합법적 사용이지만 너무 큰 batch는 abuse detection 가능성 — 실제 운영 데이터로 측정 필요. 보수적 batch 크기(100~200 alias/request)로 시작 후 운영 데이터로 한도 확정 (ex-ADR 0001 Open Question #2).
- **참조**: https://docs.github.com/en/graphql/overview/resource-limitations

#### gh subprocess 방식 회고 (RESOLVED, ADR 0001)

> **2026-04-30 결정.** `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` 참조.
>
> - Phase 4 GraphQL batching은 `gh api graphql` subprocess로 구현. 인증·rate limit·재시도 모두 gh 위임.
> - Phase 3 `gitless-push`는 ADR 0001로 폐기됐으므로 push 도구 HTTP 방식 결정은 무관.
> - v0.1 ureq 코드의 마이그레이션 시점은 ADR 0001 follow-up open question #1로 별도 결정.

## Phase 5 — 도메인 함정 정리

> "언젠가는 터질 폭탄"이므로 비목표가 아닌 명시적 후속 단계로 박는다.

- macOS HFS+/APFS의 NFD 정규화 vs GitHub의 NFC 보존 (한글·악센트 파일명 깨짐).
- 대소문자 충돌 (Windows에서 `README.md` vs `Readme.md`가 동일 path key).
- 비-UTF-8 텍스트 인코딩 (EUC-KR 등) — v0.1은 바이너리 취급으로 영구 drift 발생 (G-006).
- submodule (Trees mode `160000`) entry 처리.
- 심볼릭 링크 (Trees mode `120000`).
- 빈 파일 (`SHA-1("blob 0\0") = e69de29...`) 실파일 통합 검증 — v0.1에서 unit test로는 통과했으나 실파일 케이스 검증 필요.
- 실행 권한 (Trees mode `100755` vs `100644`).
- `.gitattributes` 파싱 → git 표준 blob SHA 정확 재현 (선택적, 큰 변경).

> **우선순위 결정 미정** (ex-ADR 0001 Open Question #3). Phase 4 완료 후 운영 데이터(어떤 함정이 실제 사용 중 자주 발생하는지)와 사용자 요청 빈도로 순서 정함.

## v0.1 시점 미결 (Open Questions)

> Phase 1 진행 중 답을 찾아 해소되면 이 섹션에서 제거 + guardrails나 spec으로 옮긴다.

- ~~**GitHub 토큰 최소 권한 범위.**~~ **OBSOLETE (ADR 0001, 2026-04-30).** gh subprocess 채택으로 인증 책임이 도구 외부(gh CLI)로 이동. PAT 권한 가이드는 `gh auth login` 한 줄로 충분하므로 도구가 별도 검증할 필요 없음. 1차 smoke test (2026-04-29 vault 356 파일, OAuth token via `gh auth token`, 284 identical / 55 local_only_changed / 17 remote_only_changed / 0 drift)로 도구 동작 자체는 입증됨.
- **큰 파일 임계치.** 예: 10MB 이상 파일의 해시 메모리 사용량. Phase 4 캐시와 연결.
- **CI 플랫폼.** GitHub Actions Windows 러너에서 tarpaulin LLVM 백엔드 안정성 1차 검증 필요.

## 정책 메모 (v0.1 시점 결정)

- v0.1 비목표는 `CLAUDE.md` Critical Rules 참조. 위 Phase 2~5는 **언젠가 할 것**, 비목표는 **v0.1에는 안 한다**의 차이.
- LFS 추적 파일은 명시적 비목표 (Phase 5에도 포함 안 함). LFS 지원이 필요하면 별도 도구.
- 인터랙티브 UI는 영구 비목표. read-only CLI 본성에 어긋남.
- GitHub 외 호스팅(GitLab, Bitbucket)은 영구 비목표. fork 환영.
