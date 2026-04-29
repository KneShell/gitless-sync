# Roadmap (Backlog)

> 이 파일은 ralph가 자동 로드하지 않는다. 사람이 v0.1 완료 후 다음 phase 진입할 때 참조.

## Phase 2 — 편의 명령어
- `gitless-sync init` — `gitless-sync.toml` 설정 파일 생성 도우미.
  - 입력: `--repo`, `--branch` 등 인자 또는 prompt
  - 출력: 현재 디렉토리에 `gitless-sync.toml` 작성. 기존 파일 있으면 `--force` 없이 실패.
  - 실패 모드: 파일 권한, 기존 파일 충돌.
- `status` 명령어는 만들지 않는다. `scan --summary-only`로 대체.

## Phase 3 — Write 도구 분리
- 별도 바이너리 `gitless-push` (workspace 안에 추가).
- scan 결과(JSON)를 stdin 또는 파일로 받아 GitHub API로 실제 push 수행.
- gitless-sync는 read-only 불변 유지 (이 원칙은 v0.1부터 박힘).
- AI가 scan → 사용자 승인 → push로 명시적 단계 분리.
- workspace 안에서 `crates/gitless-sync/src/shared/`를 같이 사용 (또는 `crates/shared/`로 분리 검토).

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
- **Caveat**: GitHub은 batching을 공식 권장하지 않음 ("polling 대신 webhook events"). 합법적 사용이지만 너무 큰 batch는 abuse detection 가능성 — 실제 운영 데이터로 측정 필요. 보수적 batch 크기로 시작.
- **참조**: https://docs.github.com/en/graphql/overview/resource-limitations

#### gh subprocess 방식 회고
Phase 3 (`gitless-push`) 들어가기 전, v0.1의 ureq 직접 호출 vs gh CLI subprocess 두 방식을 비교한다. v0.1는 ureq로 갔지만 회고적으로 gh subprocess가 더 단순했을 가능성 — 결정의 적정성 재검토.

**Claude Code 친화성 기준 추가 (2026-04-29)**: 6인 tribunal은 4기준 (우아함/속도/GitHub 친화성/v0.1 정신)으로 평가, 이 기준 누락. 사용자가 사후에 "Claude Code 친화 CLI 만드는 게 핵심 use case"임을 명시. ureq 직접 HTTP면 Claude Code가 매번 `--token env:GITHUB_TOKEN` 인자 필요 또는 사용자 env var 사전 setup, gh subprocess면 Claude Code 환경의 gh 인증을 자동 활용 → 0-마찰. Phase 3 진입 시 재검토 시 이 기준이 가장 무거움.

- **gh로 갔을 때 얻는 것**: 인증·rate limit·abuse detection·GraphQL 호출 모두 gh에 위임. `shared/error.rs`의 HTTP 관련 variant + G-003 / G-011 guardrail 상당 부분 무용. 코드 베이스 슬림.
- **gh로 갔을 때 잃는 것**: gh 설치 의존성. 단 PRD 정신("git 없는 환경")과 모순 X — iCloud는 `git init`을 막지 gh API 호출을 막지 않음. subprocess 호출 비용은 GraphQL batching이면 무관 (한 번만 fork).
- **결정 산출물**: 이 회고 결과로 (1) Phase 3 push 도구의 HTTP 방식, (2) Phase 4 batching 구현 (ureq+GraphQL vs gh+GraphQL) 동시 결정. 결정 시 `gitless-sync` v0.1를 갈아엎을지 (사실 가능성 낮음, 단순 정리만), Phase 3+4 신규 코드만 새 방식으로 갈지 분기.

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

## v0.1 시점 미결 (Open Questions)

> Phase 1 진행 중 답을 찾아 해소되면 이 섹션에서 제거 + guardrails나 spec으로 옮긴다.

- **GitHub 토큰 최소 권한 범위.** 1차 smoke test 통과 (2026-04-29 vault 356 파일, OAuth token via `gh auth token`, 284 identical / 55 local_only_changed / 17 remote_only_changed / 0 drift). Fine-grained PAT (`Contents: Read`만)로 Trees + Commits API 정식 검증은 보류 — Phase 4 § gh subprocess 결정 후 재정의. gh subprocess 채택 시 PAT 권한 가이드 자체가 도구 책임 밖으로 이동하므로 검증 의미가 달라짐. v0.1 hard blocker에서 nice-to-have로 강등 (2026-04-29).
- **큰 파일 임계치.** 예: 10MB 이상 파일의 해시 메모리 사용량. Phase 4 캐시와 연결.
- **CI 플랫폼.** GitHub Actions Windows 러너에서 tarpaulin LLVM 백엔드 안정성 1차 검증 필요.

## 정책 메모 (v0.1 시점 결정)

- v0.1 비목표는 `CLAUDE.md` Critical Rules 참조. 위 Phase 2~5는 **언젠가 할 것**, 비목표는 **v0.1에는 안 한다**의 차이.
- LFS 추적 파일은 명시적 비목표 (Phase 5에도 포함 안 함). LFS 지원이 필요하면 별도 도구.
- 인터랙티브 UI는 영구 비목표. read-only CLI 본성에 어긋남.
- GitHub 외 호스팅(GitLab, Bitbucket)은 영구 비목표. fork 환영.
