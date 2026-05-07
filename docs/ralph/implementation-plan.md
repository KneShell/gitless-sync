# Implementation Plan

## Status
- Last updated: 2026-05-07 (Phase 4 진입 — GraphQL batching + 로컬 SHA mtime cache)
- Total tasks: 9 (P1, P2, P3, P4, P5, P6, P7, P8, P9)
- Completed: 0 / 9

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵된 상태.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 해당 spec 파일과 정확히 매핑되어야 함. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- **모든 task는 `[AUTO]`** — 사람 개입 0으로 자율 루프 진행. Phase 4 사전 결정은 § Phase 4 사전 결정에 박힘 (P1 ADR 박제 + P2 spec 갱신으로 코드 baseline에 반영).
- **transient 실패 자동 회복**: G-015로 박힌 [!] task는 `prompt-build.md` § 1 [!] auto-recovery 룰에 따라 다음 iteration 자동 reset.

## Phase 4 사전 결정 (ralph 자율 진행 시 변경 금지)

1. **Scope**: GraphQL batching + 로컬 SHA mtime cache. roadmap "확정"(GraphQL) + "조건부"(cache) 카테고리 둘 다 박음. cache 효과 미달 시 P7 ADR 0008에서 제거 결정.
2. **Backend default**: 본 phase 완료 후 `--backend graphql`이 default. REST는 `--backend rest` explicit으로 유지 (v0.1 자산 보존, GraphQL 운영 이슈 시 fallback). ADR 0006.
3. **GraphQL endpoint**: `gh api graphql -f query=<query>` subprocess. 인증·rate limit·재시도 모두 gh 위임 (ADR 0001 일관).
4. **Alias batching 패턴**: 한 alias = `history(first: 1, path: ...)` = 1 node. batch size **default 200** alias/request (roadmap 권장 상한). P6 측정 + P7 ADR 0007에서 confirm/조정.
5. **Path → alias name mangling**: GraphQL alias는 `[A-Za-z_][A-Za-z0-9_]*` 제한. path는 `/` / `.` / 공백 / 한글 등 포함 가능 → `a` + sequential index (`a0`, `a1`, ...) 박음. 응답 매핑은 alias→path 역인덱스 (Vec).
6. **Partial errors 정책**: GraphQL 응답 `errors[]` 배열에 일부 alias만 실패 시 — **통째 fail** (보수적). 부분 결과 사용 안 함 (G-002 truncated 패턴 일관). `errors[].extensions.code`로 매핑.
7. **rayon 처분**: GraphQL backend는 rayon 미사용 (alias batching 자체가 병렬 효과). REST backend는 ADR 0003 그대로 rayon 유지. ADR 0005에 backend별 정책 박음.
8. **Cache 본성**: ADR 0009 — internal metadata, user-data 0. Read-only 본성(ADR 0001/0004)은 "user 데이터·원격 보존" 의미로 명확화. cache는 예외.
9. **Cache 위치**: OS user-cache (Linux/macOS: `$XDG_CACHE_HOME` 또는 `~/.cache/`, Windows: `%LOCALAPPDATA%`). repo+branch별 파일 분리 — `<user-cache>/gitless-sync/<owner>__<repo>__<branch>.json` (filesystem-safe sanitize). vault iCloud sync 충돌 회피 + 사용자 .gitignore 박을 필요 0. `dirs` crate 의존성 1 추가.
10. **Cache 형식**: JSON `{ "version": 1, "entries": { "<path>": { "mtime": "<ISO-8601>", "sha": "<self-hash>" } } }`. 파일 mtime 비교 → hit이면 SHA 재사용, miss/누락이면 hash + 갱신. 손상(parse fail) 시 통째 reset, graceful fallback (warning 1줄). save 권한 부족 시 warning + scan 결과는 정상.
11. **GraphQL error mapping**:
    - `data.errors[].extensions.code == "RATE_LIMITED"` → `RateLimitExceeded`
    - `errors[].extensions.code == "UNAUTHENTICATED"` → `AuthFailed`
    - `errors[].extensions.code == "NOT_FOUND"` → `Http`
    - fallthrough → `Http(stderr/errors[] 원문 보존)`
    - `gh api graphql` exit code 1 + stderr substring 매핑은 REST 매핑(spec-error-contracts.md)과 동일 우선순위 적용.
12. **dogfooding**: P9에서 `KneShell/gitless-sync` (43 files) minimum scale + cross-backend (REST/GraphQL 둘 다 실행, 결과 ScanReport 동일 검증). vault scale은 사용자 환경 의존이라 자율 검증 권고 (사람 박을 일).
13. **Performance baseline**: M5a 패턴 (warm-up drop + N=3 본 측정 + variance 30% 초과 시 N=5 확장). raw data를 task `[~]` commit message + acceptance 본문에 박음.

## Dependency Graph

```
P1 → P2 → P3 → P4 → P5 → P6 → P7 → P8 → P9
```

Linear chain. 각 task가 다음 task의 compile-clean baseline.

## Tasks

### P1. ADR 0005 + 0006 + 0009 박음 + CLAUDE.md `[AUTO, 문서/spec]`
- **Spec reference**: ADR 0001 (read-only 영구), ADR 0003 (rayon 유지), ADR 0004 (init read-only 정합)
- **Files**: `docs/adr/0005-rayon-backend-policy.md` (신규), `docs/adr/0006-default-backend-graphql.md` (신규), `docs/adr/0009-internal-cache-readonly-exception.md` (신규), `CLAUDE.md`
- **Depends on**: none
- **Acceptance criteria**:
  - `[AUTO]` ADR 0005 신규 작성:
    - § Status: Accepted, Date: 2026-05-07
    - § Context: ADR 0003 rayon 유지 결정은 REST backend 단독 시점. Phase 4에서 GraphQL alias batching 도입 → alias 자체가 병렬 효과 (한 request에 200 alias) → rayon 중복.
    - § Decision: GraphQL backend는 rayon 미사용. REST backend는 ADR 0003대로 rayon 유지 (backend별 정책).
    - § Consequences: G-011 본문 — REST backend 한정 활성으로 명시. spec-github-api § 병렬 호출 정책 갱신 (P2). MAX_COMMITS_CONCURRENCY 상수는 REST 분기에서만 active.
  - `[AUTO]` ADR 0006 신규 작성:
    - § Status: Accepted, Date: 2026-05-07
    - § Context: v0.1 default backend는 `--backend rest`. Phase 4 GraphQL이 1000 path 25초 → 수 초 (~5x speedup, P6 측정으로 확정). LLM 친화성 = 0 (호출자 ScanReport 동일).
    - § Decision: default backend `rest` → `graphql` 전환. `--backend rest`는 explicit fallback으로 유지 (운영 이슈 시).
    - § Consequences: spec-cli-interface § Backend 분기 갱신 (P2), main.rs clap default 변경 (P3).
  - `[AUTO]` ADR 0009 신규 작성:
    - § Status: Accepted, Date: 2026-05-07
    - § Context: ADR 0001/0004 read-only 본성은 user-data·원격 보존이 본질. Phase 4 mtime cache는 도구 internal metadata, 사용자 데이터 0 변경.
    - § Decision: read-only 본성을 "user 데이터·원격 0 변경"으로 명확화 — internal cache는 예외. cache 위치는 OS user-cache (`dirs::cache_dir() + "gitless-sync/"`), repo+branch별 파일 분리.
    - § Consequences: CLAUDE.md § Critical Rules § 도구 본성 명확화 한 줄 박음. spec-config § cache 추가 (P2). `dirs` crate 의존성 추가 (P4).
  - `[AUTO]` `CLAUDE.md` § Current State 갱신: "Phase 4 진행 중 — GraphQL batching + mtime cache (ADR 0005/0006/0009)" 한 줄 추가.
  - `[AUTO]` `CLAUDE.md` § Critical Rules § 도구 본성 한 줄 명확화: "Read-only는 **user 데이터·원격 보존**이 본질. Internal cache는 예외 (ADR 0009)."
  - `[AUTO]` `CLAUDE.md` § 사용자 취향 결정 (검증·토론 대상 X) section에 박음:
    - "default backend는 GraphQL (ADR 0006). REST는 explicit fallback 유지."
    - "GraphQL backend는 rayon 미사용 (ADR 0005, alias batching 자체가 병렬)."
- **Status**: `[ ]`

### P2. spec 갱신 — Phase 4 GraphQL backend + cache `[AUTO, spec-only]`
- **Spec reference**: `docs/specs/spec-github-api.md`, `docs/specs/spec-cli-interface.md`, `docs/specs/spec-error-contracts.md`, `docs/specs/spec-config.md`, `docs/roadmap.md`
- **Files**: 위 5개 spec/roadmap
- **Depends on**: P1
- **Acceptance criteria**:
  - `[AUTO]` `spec-github-api.md` § GraphQL backend 본체 신규:
    - 진입점 시그니처: `fetch_last_commit_at_batch(client, repo, branch, paths) -> Result<HashMap<String, DateTime<Utc>>, GitlessError>`.
    - alias batching 패턴 — 한 alias = `history(first: 1, path: X)` = 1 node. batch size default **200** (P7 ADR 0007에서 confirm).
    - query 빌더 의사코드 박음 (`repo.ref(qualifiedName: "refs/heads/{branch}").target { ... on Commit { history aliases ... } }`).
    - **timestamp 필드는 `committedDate` 사용** (REST `committer.date`와 일관). `authoredDate`는 commit author date라 cherry-pick / rebase 시 committer date와 달라짐 → cross-backend 정합 깨짐. 사용 금지.
    - path → alias mangling: `a` + sequential index (`a0`, `a1`, ..., `a199`). 응답 매핑은 Vec<&str>로 alias→path 역인덱스.
    - partial errors 통째 fail 정책 (errors[] 배열 비어 있지 않으면 즉시 매핑 후 fail).
    - § 병렬 호출 정책 갱신: backend별 분기 — REST = rayon 8c (ADR 0003), GraphQL = alias batching only (ADR 0005). MAX_COMMITS_CONCURRENCY는 REST 한정 active.
    - § Backend 선택 갱신: default `graphql` (ADR 0006), REST는 `--backend rest`로 explicit.
  - `[AUTO]` `spec-cli-interface.md` § Backend 분기 갱신: default `rest` → `graphql`. v0.1 stub 표현 ("`--backend graphql`: 즉시 exit 1") 제거. ADR 0006 cross-ref 박음.
  - `[AUTO]` `spec-error-contracts.md` § GraphQL error mapping 추가 — `data.errors[].extensions.code` 표 (RATE_LIMITED → RateLimitExceeded / UNAUTHENTICATED → AuthFailed / NOT_FOUND → Http / fallthrough → Http). REST stderr 매핑 우선순위와 일관.
  - `[AUTO]` `spec-config.md` § cache 추가 — 위치 (`dirs::cache_dir() + "gitless-sync/"`), 파일명 sanitize 룰, JSON 형식, lifecycle (load → lookup/insert → save), graceful fallback. ADR 0009 cross-ref. 사용자 .gitignore 박을 필요 0 명시.
  - `[AUTO]` `roadmap.md` § Phase 4: 진행 중 박스 박음 (Phase 2 COMPLETED 패턴 따라). § "조건부" 카테고리 cache는 "본 phase에서 도입, ADR 0008에서 confirm"으로 갱신.
- **Status**: `[ ]`

### P3. GraphQL backend 본체 + error mapping + main.rs default 전환 `[AUTO, 코드]`
- **Spec reference**: `spec-github-api.md` § GraphQL backend (P2 갱신본), `spec-error-contracts.md` § GraphQL error mapping (P2)
- **Files**: `crates/gitless-sync/src/commands/scan/graphql.rs` (신규 — vertical slice 일관), `crates/gitless-sync/src/commands/scan/mod.rs` (Backend enum 분기), `crates/gitless-sync/src/commands/scan/github.rs` (REST 본체는 그대로), `crates/gitless-sync/src/main.rs` (clap default 전환), `crates/gitless-sync/src/shared/error.rs` (GraphQL error mapping helper), `crates/gitless-sync/src/lib.rs` (graphql 모듈 export), `crates/gitless-sync/src/shared/hash.rs`, `crates/gitless-sync/src/shared/normalize.rs`, `crates/gitless-sync/src/commands/scan/compare.rs`, `crates/gitless-sync/src/commands/scan/output.rs` (lib export로 surface된 pedantic clippy 동반 정리 — v0.2 M4a / Phase 2 P3 cascade 선례, 발생 시만 수정)
- **Depends on**: P2
- **Acceptance criteria**:
  - `[AUTO]` `commands/scan/graphql.rs` 신규:
    - `pub(crate) fn fetch_last_commit_at_batch(client: &impl GhClient, repo: &str, branch: &str, paths: &[String]) -> Result<HashMap<String, DateTime<Utc>>, GitlessError>` 시그니처.
    - paths를 batch size (default `GRAPHQL_BATCH_SIZE = 200`)로 chunk → 각 chunk별 `gh api graphql` 호출.
    - GraphQL query 빌더: alias = `a0`, `a1`, ..., `a{N-1}`. 한 alias = `a{i}: object(expression: "{branch}:{path_quoted}") { ... on Commit { history(first: 1, path: "{path_quoted}") { nodes { committedDate } } } }`. path quote는 GraphQL string escape (`\"` / `\\` / `\n`).
    - 응답 파싱: `data.repository.{alias}` → committedDate 추출. errors[] 비어 있지 않으면 `errors[0].extensions.code` 매핑 후 통째 fail.
    - 빈 paths 입력 → `Ok(HashMap::new())` 즉시 반환 (외부 호출 0).
  - `[AUTO]` `shared/error.rs`에 `map_graphql_error(errors: &[GraphqlError]) -> GitlessError` helper. RATE_LIMITED → RateLimitExceeded, UNAUTHENTICATED → AuthFailed, NOT_FOUND → Http, fallthrough → Http(원문).
  - `[AUTO]` `commands::scan::run_with_client` 안에서 `args.backend` enum 분기. GraphQL 분기 = `fetch_last_commit_at_batch` 1회 호출, REST 분기 = 기존 `fetch_last_commit_at` rayon 8c 병렬 (그대로).
  - `[AUTO]` `main.rs` clap `Backend` enum default 변경: `#[arg(default_value_t = Backend::Graphql)]`. v0.1 stub error("GraphQL backend not implemented...")는 제거.
  - `[AUTO]` `lib.rs`에 `pub mod commands;` 또는 graphql 모듈 export 정렬 (통합 테스트 진입 가능).
  - `[AUTO]` 단위 테스트 minimal (`graphql.rs::tests`, MockGhClient stub):
    - 정상 batch 1개 (paths=["a.md"], stub 응답 → committedDate 매칭)
    - errors[].code == "RATE_LIMITED" → RateLimitExceeded
    - 빈 paths → Ok(empty)
    - (P5에서 매트릭스 확장)
  - `[AUTO]` **lib export cascade 정리**: 신규 `pub` surface로 발생한 pedantic clippy warning은 본 task Files 영역 안에서 동반 정리 (Phase 2 P3 패턴 일관). 영역 초과 시 [!] + 사람 escalate.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` 통과.
- **Status**: `[ ]`

### P4. 로컬 SHA mtime cache 본체 `[AUTO, 코드]`
- **Spec reference**: `spec-config.md` § cache (P2 갱신본), ADR 0009
- **Files**: `crates/gitless-sync/src/shared/cache.rs` (신규), `crates/gitless-sync/src/commands/scan/walker.rs` (cache 통합), `crates/gitless-sync/src/commands/scan/mod.rs` (cache load/save 진입점), `crates/gitless-sync/src/lib.rs` (cache 모듈 export), `crates/gitless-sync/Cargo.toml` (`dirs` crate 추가)
- **Depends on**: P3
- **Acceptance criteria**:
  - `[AUTO]` `Cargo.toml`에 `dirs = "5"` (또는 최신 안정) 추가. `Cargo.lock` 갱신.
  - `[AUTO]` `shared/cache.rs` 신규:
    - `pub(crate) struct Cache { version: u32, entries: HashMap<String, CacheEntry> }` (serde derive).
    - `pub(crate) struct CacheEntry { mtime: DateTime<Utc>, sha: String }` (serde derive).
    - `pub(crate) fn cache_path(repo: &str, branch: &str) -> Result<PathBuf, GitlessError>` — `dirs::cache_dir()` + `gitless-sync/` + `<owner>__<repo>__<branch>.json` (filesystem-safe sanitize: `/` → `__`, 기타 특수문자 제거).
    - `pub(crate) fn load(path: &Path) -> Cache` — read 시도 → parse 성공이면 반환, 실패(미존재/parse 에러)면 `Cache::default()` + stderr warning 1줄 (`cache reset: <reason>`). graceful fallback (return type: Cache, not Result).
    - `pub(crate) fn save(&self, path: &Path) -> Result<(), GitlessError>` — atomic write (`<path>.tmp` → rename). 디렉토리 미존재 시 create_dir_all. write 권한 부족 시 `Io` 매핑 + main.rs에서 stderr warning 처리.
    - `pub(crate) fn lookup(&self, path: &str, mtime: DateTime<Utc>) -> Option<&str>` — entry 존재 + mtime 일치 시 sha 반환.
    - `pub(crate) fn insert(&mut self, path: String, mtime: DateTime<Utc>, sha: String)` — 갱신.
  - `[AUTO]` `walker.rs` 또는 hash 진입점에서 cache 사용:
    - scan 시작 시 `Cache::load(cache_path(repo, branch)?)`.
    - 파일 walk 후 `cache.lookup(path, mtime)` → hit이면 SHA 재사용, miss이면 `hash::compute_blob_sha(path)` + `cache.insert(...)`.
    - scan 종료 직전 `cache.save(cache_path)` — fail 시 warning, scan 결과 영향 0.
  - `[AUTO]` cache 손상 graceful fallback: parse 실패 시 통째 reset, scan 진행 영향 0.
  - `[AUTO]` cache 권한 부족 graceful fallback: lookup은 가능 (default Cache), save 실패 시 warning + scan 결과 정상.
  - `[AUTO]` 단위 테스트 minimal (`cache.rs::tests`):
    - hit/miss/mtime 변경
    - parse 실패 → default Cache 반환
    - atomic save (tmp → rename 시 이전 파일 손상 0)
    - cache_path sanitize (`KneShell/gitless-sync` + `main` → `KneShell__gitless-sync__main.json`)
    - (P5에서 매트릭스 확장)
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` 통과.
- **Status**: `[ ]`

### P5. 단위 + 통합 테스트 매트릭스 (GraphQL + cache) `[AUTO, 코드]`
- **Spec reference**: `spec-github-api.md` Acceptance Criteria, `spec-error-contracts.md` PRD 시나리오 20~25, `spec-config.md` cache Acceptance
- **Files**: `crates/gitless-sync/src/commands/scan/graphql.rs::tests`, `crates/gitless-sync/src/shared/cache.rs::tests`, `crates/gitless-sync/tests/integration.rs`
- **Depends on**: P4
- **Acceptance criteria**:
  - `[AUTO]` GraphQL 단위 테스트 매트릭스 (MockGhClient stub):
    - 정상 batch (10 paths, 모두 commits 있음, 응답 매칭)
    - 빈 paths 배열 (즉시 Ok(empty), MockGhClient 호출 0)
    - paths > batch size (300 → chunk 두 번: 200+100, 응답 합산)
    - errors[].extensions.code == "RATE_LIMITED" → RateLimitExceeded
    - errors[].extensions.code == "UNAUTHENTICATED" → AuthFailed
    - errors[].extensions.code == "NOT_FOUND" → Http
    - errors[].extensions.code == "INTERNAL_SERVER_ERROR" (fallthrough) → Http
    - 일부 alias만 응답 + 나머지 errors → 통째 fail (partial errors 정책)
    - alias mangling: 200 paths → a0, a1, ..., a199 안전 매핑 + 응답 → path 역매핑 정합
    - GraphQL escape: path에 `"` / `\\` / `\n` 포함 시 query string 안전 escape (실제 호출 인자 검증)
  - `[AUTO]` Cache 단위 테스트 매트릭스:
    - hit: 동일 mtime → cached sha 반환
    - miss: 첫 호출 → None
    - mtime 변경 invalidate → None
    - parse 실패 graceful (default Cache + warning 1줄 stderr capture)
    - save atomic (tmp → rename, 이전 파일 손상 0)
    - save 실패 (시뮬레이션 fs error) graceful (warning, Cache 본체는 정상)
    - cache_path sanitize 매트릭스 (특수문자 / 공백 / 한글)
    - version 미스매치 시 reset (예: 미래 version 2 cache 만나면 default 반환)
  - `[AUTO]` 통합 테스트 시나리오 20~25 (`tests/integration.rs`):
    - 시나리오 20: GraphQL backend 정상 (`run_with_client(args, &MockGhClient stub graphql)` → ScanReport 정상)
    - 시나리오 21: GraphQL backend errors (rate limit / auth / not_found 매핑)
    - 시나리오 22: Cache miss → hit (2회 scan, 1차 timing > 2차 timing 또는 2차에서 MockGhClient 호출 횟수 줄어듦으로 검증)
    - 시나리오 23: Cache invalidate (파일 mtime 변경 → 2차 scan에서 re-hash 발생 확인)
    - 시나리오 24: Cross-backend 결과 동일 (`--backend rest` + `--backend graphql` 두 stub 같은 응답 → ScanReport `summary` + `files[]` set 동일)
    - 시나리오 25: cache 손상 graceful (`.json` 파일 임의 파괴 → scan 정상 + warning emit)
  - `[AUTO]` 테스트 패턴: library entry inject (Phase 2 P5 패턴 일관). `cargo run --` 자식 프로세스 호출 0.
  - `[AUTO]` `cargo test --workspace`, `cargo test --test integration` 전체 통과.
- **Status**: `[ ]`

### P6. 측정 task — batch size + REST vs GraphQL + cache `[AUTO]`
- **Spec reference**: ADR 0007 + 0008 박을 raw data 수집 (P7)
- **Files**: 박제 0. raw data를 task `[~]` commit message + acceptance 본문에 박음.
- **Depends on**: P5
- **Acceptance criteria**:
  - `[AUTO]` 환경: KneShell/gitless-sync (43 files) — minimum scale baseline. 측정 직전 `gh auth status` exit 0 확인 (실패 시 G-015 영구 신호 → [!]).
  - `[AUTO]` (a) **GraphQL batch size 100 vs 200** (M5a 패턴):
    - 코드 임시 변경 (`GRAPHQL_BATCH_SIZE = 100`) → 측정 → revert.
    - warm-up 1회 dropped, N=3 본 측정, variance 30% 초과 시 N=5 확장.
    - mean / min / max / variance 박제. raw timing (각 측정의 wall-clock ms) 박음.
    - speedup 또는 slowdown 명시 (200 baseline 대비).
  - `[AUTO]` (b) **REST vs GraphQL baseline**:
    - 동일 repo + paths(43) + warm-up drop + N=3.
    - REST mean (rayon 8c, ADR 0003 1351ms baseline 재현 — 큰 편차 시 환경 변동 분석).
    - GraphQL mean.
    - speedup ratio 박음. 1000 path scale 추정 박음 (linear extrapolation, sublinear 조심).
  - `[AUTO]` (c) **Cache hit rate**:
    - 1차 scan (cache miss, full hash) timing.
    - 2차 scan (cache hit, mtime 일치) timing.
    - speedup ratio 박음. 1차/2차 timing 차이 + cache hit 비율 (100% 기대) 검증.
  - `[AUTO]` 측정 도중 transient 실패 (gh exit≠0)는 G-015 retry policy 적용 (N=3 + 30s backoff). 3회 실패 시 [!] + G-015 reference (auto-recovery 가능).
  - `[AUTO]` raw data를 본 task acceptance 본문에 박음 (M5a 패턴 — 환경 / 명령어 / N=3 raw ms / mean / variance / speedup).
- **Status**: `[ ]`

### P7. ADR 0007 + 0008 박음 + spec 갱신 `[AUTO, 문서/spec/코드]`
- **Spec reference**: ADR 0003/0005/0009 패턴, P6 raw data
- **Files**: `docs/adr/0007-graphql-batch-size.md` (신규), `docs/adr/0008-mtime-cache-keep-or-drop.md` (신규), `docs/specs/spec-github-api.md` (batch size baseline 박제), `docs/specs/spec-config.md` (cache 결정 박제), `CLAUDE.md` (Current State 갱신), (cache 제거 결정 시) `crates/gitless-sync/src/shared/cache.rs` 통째 삭제 + `walker.rs`/`mod.rs` cache 통합 코드 삭제 + `Cargo.toml` `dirs` dep 삭제 + `Cargo.lock` 갱신
- **Depends on**: P6
- **Acceptance criteria**:
  - `[AUTO]` ADR 0007 신규 — § Status: Accepted, Date: 2026-05-07. § Context: P6 (a) raw data + 측정 환경. § Decision: 100 또는 200 (P6 raw data 기반). 더 빠른 쪽 + variance 안정 쪽 채택. § Consequences: spec-github-api § GraphQL backend batch size baseline 박제. cap 변경 시 본 ADR + spec 동시 갱신.
  - `[AUTO]` ADR 0008 신규 — § Context: P6 (c) raw data + cache hit 효과. § Decision: ① cache 유지 (의미 있는 speedup, 일반적 ≥ 2x) 또는 ② cache 제거 (효과 미달 — 코드/의존성 부담만). 결정 근거 raw data로 명시.
  - `[AUTO]` 결정에 따라 spec/code 처리:
    - 유지: spec-config § cache 확정 박제 (P2 cross-ref 박힌 ADR 0008 → confirmed). cache.rs 코드 그대로.
    - 제거: cache.rs + walker.rs cache 통합 통째 삭제 + Cargo.toml `dirs` 삭제 + Cargo.lock 갱신 + spec-config § cache 섹션 삭제 + ADR 0009 obsolete 마크 + CLAUDE.md cache 본성 한 줄 제거.
  - `[AUTO]` `CLAUDE.md` Current State 갱신: ADR 0007 + ADR 0008 결정 박스 추가.
  - `[AUTO]` `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo deny check`, `cargo audit` 모두 통과 (cache 제거 시 cargo deny에서 dirs 부재 확인).
- **Status**: `[ ]`

### P8. coverage 게이트 통과 검증 (phase-final, M7 패턴) `[AUTO]`
- **Spec reference**: `docs/ralph/project-ops.md` § Coverage, `CLAUDE.md` § Test coverage, G-007, G-012, G-013
- **Files**: 미달 모듈에 unit test 추가 (필요 시), `deny.toml` (신규 의존성 화이트리스트 갱신)
- **Depends on**: P7
- **Acceptance criteria**:
  - `[AUTO]` `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo audit` 모두 통과.
  - `[AUTO]` `cargo tarpaulin --engine llvm --workspace --out Stdout` 라인 커버리지 ≥ 80%.
  - `[AUTO]` `cargo tree`로 신규 의존성 점검:
    - `dirs` crate 박혀 있으면 (cache 유지 결정 시) deny.toml 라이선스 화이트리스트 갱신 + cargo deny check 재통과 확인.
    - 그 외 transitive로 박힌 신규 crate (GraphQL JSON 파싱용 등)도 동일 점검.
  - `[AUTO]` 결과 박제: tests 카운트 + tarpaulin %, ureq/mockito 부재 재확인.
- **Status**: `[ ]`

### P9. dogfooding contract step + cross-backend 정합성 `[AUTO]`
- **Spec reference**: ADR 0006 (default GraphQL), M8/Phase 2 P8 dogfooding 선례
- **Files**: 박제 0. 실행 결과는 task `[x]` commit message에 카운트만 인라인.
- **Depends on**: P8
- **Acceptance criteria**:
  - `[AUTO]` **진입 사전 점검**: `gh auth status` exit 0 확인. 실패 시 즉시 [!] + 명시 메시지 (Phase 2 P8 패턴 일관). G-015 영구 신호.
  - `[AUTO]` ralph 환경에서 release 빌드: `cargo build --release` exit 0.
  - `[AUTO]` GraphQL backend (default) 실행: `cargo run --release -- scan --repo KneShell/gitless-sync --branch main --local D:\00.Projects\02.Personal\05.gitless-sync` → exit 0 + stdout JSON 파싱 통과 + summary 5 카운트 invariant 일치 (M8/Phase 2 P8 게이트).
  - `[AUTO]` REST backend explicit 실행: 동일 명령에 `--backend rest` 추가 → exit 0 + summary 5 카운트 invariant 일치.
  - `[AUTO]` **Cross-backend 정합성**: 두 실행 결과의 summary 5 카운트 정확히 동일 + `files[]` 배열 set 비교 (path/status/sha 모두 동일, order 무관). 차이 발생 시 [!] + 사람 분석 (GraphQL 응답 정합성 이슈 가능성).
  - `[AUTO]` Cache 효과 검증 (cache 유지 결정 시): `<user-cache>/gitless-sync/KneShell__gitless-sync__main.json` 자동 생성 확인 + 같은 명령 2차 실행 timing 단축 확인 (P6 (c) raw data와 일관성 비교, ±20% 마진).
  - `[AUTO]` external command transient (network 5xx, gh exit≠0)는 G-015 retry policy 적용. 3회 실패 시 [!] + G-015 reference (auto-recovery 가능).
  - `[AUTO]` 박제 0. git log + commit message가 evidence trail. failed 비율 / cache hit 비율은 commit message에 기록만.
- **Status**: `[ ]`
