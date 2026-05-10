# Spec: GitHub API Integration

> **2026-05-06 (M0)**: ADR 0001 + ADR 0002 정합 통째 재작성. v0.1 ureq baseline 표현(직접 HTTP 호출 / mockito / Agent thread-safety / HTTP 헤더 송신 검증) 제거. 모든 GitHub API 호출은 `gh api` subprocess 단일 통로.

## 목적

GitHub Trees / Blobs / Commits API를 `gh api` subprocess로 호출 (ADR 0001 + ADR 0002). 인증·rate limit·truncation 등 운영 책임은 `gh`에 위임하고, 본 도구는 종료 코드 + stderr를 좁은 substring 매칭으로 `GitlessError`에 매핑하는 책임만 진다.

## 현재 상태

ADR 0001 (gh subprocess 단일 통로 + read-only 영구) + ADR 0002 (ureq → gh 마이그레이션 단일 baseline, 2026-05-07) 완료. backend 정책: REST는 rayon 8 concurrent (ADR 0003), GraphQL backend는 alias batching 단독 (ADR 0005). default backend `rest` → `graphql` 전환 (ADR 0006), batch size 200 default confirmed (ADR 0007).

## 작업 범위

### `GhClient` trait + `GhResponse`

```rust
pub(crate) struct GhResponse {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub exit_code: i32,
}

pub(crate) trait GhClient {
    fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError>;
}
```

설계 근거:

- `GhResponse`에 `headers` / `duration` 등 추가 필드는 yagni. v0.1 매핑은 `exit_code` + `stderr` substring + `stdout` JSON으로 충분.
- `&[&str]`은 lifetime juggling, `IntoIterator<Item = impl AsRef<str>>` generic은 `dyn GhClient` trait object를 깬다. `&[String]`이 호출 측 `format!` 결과를 `vec![...]`에 담기 가장 자연.
- `api()`는 raw `GhResponse`를 transparent 반환한다. `exit_code`/`stderr` → `GitlessError` 매핑은 호출 측(`fetch_*`) 책임. 매핑 표는 `spec-error-contracts.md` (M1) 한 곳에만 명시.

### `RealGhClient` (production)

- `pub(crate) fn new() -> Self` — 인자 0개. PATH lookup으로 `gh` 찾는다.
- `binary_path: Option<PathBuf>` 같은 inject 옵션은 yagni 적용으로 빠짐.
- 내부 호출: `std::process::Command::new("gh").args(args).output()`.
- `gh` 미존재 시 첫 호출에서 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환. (`Command::new` IO 에러를 본 variant로 매핑.)

### `MockGhClient` (테스트)

- 인자별 응답을 HashMap 또는 클로저로 stub.
- 단위 테스트 + 통합 테스트 모두 `MockGhClient` inject. mockito 호출 0회. v0.1 ureq baseline 시기에 사용된 mockito 시나리오는 모두 `MockGhClient` stub 응답으로 재작성.

### `main.rs` entry pattern

- production 분기에서 `RealGhClient::new()`를 1회 inject:
  ```rust
  let client = RealGhClient::new();
  commands::scan::run_with_client(&args, &client)
  ```
- 통합 테스트는 library entry `commands::scan::run_with_client(args: &ScanArgs, client: &impl GhClient)`를 직접 호출 + `MockGhClient` inject. 테스트가 production CLI 진입(`main`)을 거치지 않는다.

### `fetch_*` 인터페이스

v0.1 ureq baseline 시그니처에서 `token` 인자 제거 + `client: &impl GhClient` 추가:

- `fn fetch_tree(client: &impl GhClient, repo: &str, branch: &str) -> Result<Vec<RemoteFile>, GitlessError>`
- `fn fetch_blob(client: &impl GhClient, repo: &str, sha: &str) -> Result<Vec<u8>, GitlessError>`
- `fn fetch_last_commit_at(client: &impl GhClient, repo: &str, branch: &str, path: &str) -> Result<DateTime<Utc>, GitlessError>`

### `gh api` 호출 인자 패턴

**`--paginate` flag 사용 금지.** paging이 필요한 경우 `per_page`를 인자에 명시 (Commits API). `--paginate`는 다중 페이지 stdout concat 동작이 본 도구 단일 응답 파싱 가정과 충돌.

#### `fetch_tree`

- 호출: `gh api repos/{owner}/{repo}/git/trees/{branch}?recursive=1`
- args 빌드 예: `vec!["api".to_string(), format!("repos/{owner}/{repo}/git/trees/{branch}?recursive=1")]`
- 응답 처리 (stdout JSON):
  - `tree` 배열에서 `type == "blob"`만 추출. `type == "tree"`(디렉토리)는 무시.
  - mode `100755` / `120000` / `160000` 등 v0.1 비목표 entry는 skip + warning(stderr) (G-010).
  - `truncated == true` → § Trees truncation handling sub-tree fallback 진입 (Phase 7부터). v0.2.x까지는 `GitlessError::TreesTruncated` 즉시 반환 + exit 5 (G-002).

#### `fetch_blob`

- 호출: `gh api repos/{owner}/{repo}/git/blobs/{sha}`
- args 빌드 예: `vec!["api".to_string(), format!("repos/{owner}/{repo}/git/blobs/{sha}")]`
- 응답 처리 (stdout JSON):
  - `{"content": "<base64>", "encoding": "base64", ...}`.
  - base64 디코딩 후 raw bytes 반환.
- Phase 7부터 size pre-flight + post-flight 적용 (§ Blob size 한도 참조).

#### Blob size 한도 (Phase 7)

> **공식 한도** (2026-05-10 fact check, [source: https://docs.github.com/en/rest/git/blobs]): Git Blobs API는 **100MB 단일 파일 hard limit** 지원. 100MB 초과 blob은 endpoint 자체 미지원. base64 응답 크기 별도 한도 명시 없음 (raw bytes 100MB → base64 약 134MB).

- Phase 7부터 fetch_blob은 응답 본문 size 측정 + 100MB 초과 시 `GitlessError::Http("blob too large: <sha>, size=<bytes>")` 반환. 100MB 직전 (예: 99MB) entry는 정상 처리 — caller가 hash 계산 후 결과 비교.
- 메모리 임계치 50MB 초과는 spec-hash-and-normalize.md § Phase 7 — 큰 파일 처리 § 한도 정의 + § 검출 알고리즘 fetch_blob_with_size_gate 그대로 적용.
- **Contents API 사용 금지** — 1MB inline base64 + 1~100MB raw media type 2-tier 분기는 fetch_blob 단일 통로 정책과 호환 안 됨 [source: https://docs.github.com/en/rest/repos/contents]. Blobs API 단일 통로 + 100MB 단일 hard limit 일관.
- **LFS pointer 분기** — 100MB 미만 LFS pointer text는 본 endpoint가 raw pointer text 반환 (실제 LFS 객체 0). pointer detect는 spec-domain-pitfalls.md § LFS pointer + spec-hash-and-normalize.md § LFS pointer 그대로 처리. fetch_blob은 pointer text를 raw bytes로 반환만 함 (LFS 객체 fetch 책임 없음, 영구 비목표).

#### Trees truncation handling (Phase 7 sub-tree fallback)

> **공식 한도** (2026-05-10 fact check, [source: https://docs.github.com/en/rest/git/trees]): Trees API recursive=1 응답은 **100,000 entry + 7MB 둘 중 먼저 도달 시 `truncated: true`** 반환. 공식 권장: "use the non-recursive method of fetching trees, and fetch one sub-tree at a time". truncated 시 부분 결과는 incomplete (사용 금지).

##### 진입 조건

- `fetch_tree` 1차 호출 응답 `truncated: true` 검출 시 본 § 진입.
- v0.2.x까지는 `GitlessError::TreesTruncated` 즉시 반환 + exit 5 (G-002). Phase 7부터 sub-tree fallback 진입 후 실패 시에만 동일 error 반환.

##### sha 일관성 (sub-claude finding 5 정합)

- Trees fallback 진입 직전 1회 ref → commit sha → root tree sha resolve.
- 모든 sub-tree 호출은 `gh api repos/{owner}/{repo}/git/trees/{sub_tree_sha}` (immutable tree sha 직접 사용). branch 이름 / ref 사용 금지 (resolve 시점과 sub-tree 호출 시점의 HEAD drift 차단).
- root tree sha resolve 호출 (1회 추가):
  1. `gh api repos/{owner}/{repo}/git/refs/heads/{branch}` → 응답 `object.sha` (commit sha)
  2. `gh api repos/{owner}/{repo}/git/commits/{commit_sha}` → 응답 `tree.sha` (root tree sha)
- 1회 추가 호출 비용으로 모든 sub-tree 호출 immutable 보장.

##### sub-tree 재귀 알고리즘

```
fn fetch_tree_with_fallback(client, repo, branch) -> Vec<RemoteFile>:
    let resp = fetch_tree_recursive(repo, branch);  # recursive=1 1차 호출
    if not resp.truncated:
        return resp.entries;  # 정상 path (Phase 7 이전 동작 유지)

    # 본 § sub-tree fallback 진입
    let root_sha = resolve_root_tree_sha(repo, branch);  # ref → commit → tree sha 1회
    let mut entries = vec![];
    fetch_subtree_recursive(client, repo, root_sha, "", &mut entries, &mut budget)?;
    return entries;

fn fetch_subtree_recursive(client, repo, tree_sha, path_prefix, entries, budget):
    if budget.calls_used >= MAX_TREE_CALL_BUDGET:
        return Err(TreesTruncated);  # 호출 budget 초과
    if entries.len() >= MAX_TREE_ENTRIES:
        return Err(TreesTruncated);  # entry cap 초과

    let resp = fetch_tree_non_recursive(client, repo, tree_sha);  # recursive 없이 1 layer
    budget.calls_used += 1;

    for entry in resp.tree:
        let full_path = if path_prefix.is_empty() { entry.path } else { format!("{path_prefix}/{}", entry.path) };
        match entry.type:
            "blob" => entries.push(RemoteFile { path: full_path, sha: entry.sha, mode: entry.mode }),
            "tree" => fetch_subtree_recursive(client, repo, entry.sha, full_path, entries, budget)?,
            _ => skip,  # submodule "commit" 등 v0.2 정책 그대로
```

##### 한도 상수 (Phase 7)

| 상수 | 값 | 근거 |
|---|---|---|
| `MAX_TREE_CALL_BUDGET` | 1000 | linux/torvalds 기준 sub-tree 호출 약 5000 (truncated 케이스 가정). 1000 cap = 약 200K entry vault 한도 추정 + GitHub rate limit (5000/h auth) safety. |
| `MAX_TREE_ENTRIES` | 500_000 | 누적 entry 한도. 도달 시 early-abort (메모리 안전). |

depth cap / wall-clock cap은 ADR 0011 raw data (monorepo 측정 — depth 20+ 또는 호출 시간 600s+ 발생) 도달 시 추가 검토. 초기 spec은 call budget + entries 2 cap만 (yagni 일관). 상수 변경은 ADR 0011 갱신 동반.

##### early-abort 정책

- 위 2 상수 중 하나라도 초과 시 `GitlessError::TreesTruncated` 즉시 반환 + entries 무시.
- 부분 결과 사용 금지 — G-002 정책 일관 (sub-tree fallback도 부분 결과 정책 동일).
- Phase 7 신규 unit test: call budget 1001 / entries 500_001 각 cap trigger 시 TreesTruncated 검증 (2 시나리오).

##### GraphQL backend 정합

- GraphQL backend는 commits API 한정 (§ GraphQL backend). Trees는 REST 단일 통로 — backend 분기 없음.
- 같은 commit sha 위에서 GraphQL eventual consistency window 우려 (sub-claude finding 6 정합) — Trees REST 호출이 시점 t의 root tree sha를 resolve 후 모든 sub-tree REST 호출이 동일 sha 위에서 평가 → 단일 backend 안 일관.

##### G-002 update

- G-002 본문 update: "Phase 7부터 sub-tree fallback 진입 (본 spec § Trees truncation handling 참조). v0.2.x까지는 즉시 fail."

#### `fetch_last_commit_at`

- 호출: `gh api -X GET repos/{owner}/{repo}/commits -F sha={branch} -F path={path} -F per_page=1`
- `-X GET` prepend은 필수 (G-017): `gh`는 `-F` 플래그가 하나라도 있으면 method를 POST로 자동 전환한다. commits endpoint는 GET 전용이라 POST 시 404 반환. path 인자 앞에 `-X GET`을 명시해 method를 GET으로 고정.
- args 빌드 예:
  ```rust
  vec![
      "api".to_string(),
      "-X".to_string(), "GET".to_string(),
      format!("repos/{owner}/{repo}/commits"),
      "-F".to_string(), format!("sha={branch}"),
      "-F".to_string(), format!("path={path}"),
      "-F".to_string(), "per_page=1".to_string(),
  ]
  ```
- 응답 처리 (stdout JSON 배열의 첫 번째 commit):
  - `commit.committer.date` (ISO-8601) → `DateTime<Utc>`.
- **호출 측(`scan::run_with_client`) 책임**: 차이 있는 파일에 한해서만 호출 (G-003은 ADR 0002로 도구 책임 종료 표시 예정이지만 호출 빈도 자체는 그대로 절약).

### 에러 매핑 (위임)

매핑 표는 `spec-error-contracts.md` (M1)에 한 곳에만 명시. 본 spec은 매핑 종류만 정의:

- 인증 실패 → `GitlessError::AuthFailed` (exit 2)
- Rate Limit → `GitlessError::RateLimitExceeded { reset_at }` (exit 3)
- Trees truncated → `GitlessError::TreesTruncated` (exit 5)
- 5xx / 기타 비정상 → `GitlessError::Http(String)` (exit 1)
- gh 미설치 → `GitlessError::Config(String)` (exit 1)

매칭 신호는 좁은 stderr substring + exit_code 조합. **정규식 사용 금지** (M1 룰).

### Backend 선택

- `--backend graphql` (default): § GraphQL backend 정의대로 `fetch_last_commit_at_batch` 진입점. 인증·rate limit·재시도 모두 gh 위임 (ADR 0001 일관).
- `--backend rest` (explicit fallback): 본 spec § fetch_tree / fetch_blob / fetch_last_commit_at + § 병렬 호출 정책 (REST 분기) 그대로 동작. v0.1/v0.2 자산 보존, GraphQL 운영 이슈(rate limit, alias batching 응답 정합성, partial errors 등) 발생 시 즉시 fallback (ADR 0006 § Decision).
- `fetch_tree` / `fetch_blob`은 GraphQL backend에서도 REST를 그대로 사용 (Trees / Blobs API는 GraphQL 대체 우위 0). backend 분기는 commits API 호출에 한정.
- 호출자(LLM) 인터페이스 변경 0 — `--backend graphql` 명시 불필요, 결과 ScanReport JSON identical.

### GraphQL backend

> **확정 (ADR 0006, 2026-05-07)**: default backend. Commits API의 N×round-trip을 alias batching 단일 request로 단축. 인증·rate limit은 `gh api graphql` subprocess가 처리 (ADR 0001 일관).

#### 진입점 시그니처

```rust
pub(crate) fn fetch_last_commit_at_batch(
    client: &impl GhClient,
    repo: &str,
    branch: &str,
    paths: &[String],
) -> Result<HashMap<String, DateTime<Utc>>, GitlessError>
```

- 입력: 차이 있는 파일 path 배열. 빈 배열 → `Ok(HashMap::new())` 즉시 반환 (외부 호출 0).
- 반환: `path → committed DateTime<Utc>` 매핑. 일부 path 결과 누락 시 통째 fail (아래 § Partial errors 정책).

#### Alias batching 패턴

- 한 alias = `history(first: 1, path: ...)` = 1 node. GitHub GraphQL node 한도 500,000 기준 1 path = 1 node로 직선 환산.
- batch size **default 200** alias/request (ADR 0007 confirmed). `roadmap.md` § Phase 4 GraphQL batching 권장 상한과 일관 — P6a 측정 결과 13 path scale에서 batch 100 vs 200은 1 chunk로 처리되어 functional 동등 + measurement noise 지배. yagni 일관으로 권장값 200 default 유지 결정.
- paths가 batch size 초과 시 `paths.chunks(GRAPHQL_BATCH_SIZE)`로 순차 호출. chunk 응답을 `HashMap`으로 합산 (REST rayon과 달리 request 단위 추가 병렬화 없음 — ADR 0005).

#### Path → alias mangling

GraphQL alias 식별자는 `[A-Za-z_][A-Za-z0-9_]*` 제한. path에는 `/` / `.` / 공백 / 한글 등이 포함될 수 있어 직접 매핑 불가.

- mangling 규칙: `a` + 0부터 시작하는 sequential index — `a0`, `a1`, ..., `a199` (batch size 200 기준).
- 응답 매핑: 한 batch 안에서 `Vec<&str>` (alias index → path) 역인덱스를 빌드. 응답 JSON `data.repository.ref.target.{alias}` 키를 `Vec` 인덱스로 다시 path로 환원.
- chunk 별 alias namespace는 reset (`a0`부터 다시 시작). 한 batch 안에서만 unique 보장하면 충분.

#### GraphQL query 빌더 (의사코드)

한 alias entry (Commit 안에 평탄 배치):

```graphql
a{i}: history(first: 1, path: "{path_quoted}") {
  nodes { committedDate }
}
```

전체 query (한 batch = N alias):

```graphql
query {
  repository(owner: "{owner}", name: "{name}") {
    ref(qualifiedName: "refs/heads/{branch}") {
      target {
        ... on Commit {
          a0: history(first: 1, path: "{path0_quoted}") { nodes { committedDate } }
          a1: history(first: 1, path: "{path1_quoted}") { nodes { committedDate } }
          ...
          a{N-1}: history(first: 1, path: "{path_{N-1}_quoted}") { nodes { committedDate } }
        }
      }
    }
  }
}
```

- `ref(qualifiedName: "refs/heads/{branch}").target` 분기는 한 batch에 1회. 그 안의 N alias가 모두 같은 Commit 위에서 평가됨 (`object(expression: "{branch}:{path}")`처럼 path별로 중복 분기하지 않음).
- 호출 args 빌드: `["api", "graphql", "-f", "query={query_string}"]` 4-element `Vec<String>`. `-f`는 form parameter (string-typed) — graphql endpoint에 query body로 전달.

#### Path quote (GraphQL string escape)

GraphQL string literal 내부의 `path_quoted`는 다음 escape:

- `\` → `\\`
- `"` → `\"`
- `\n` → `\\n`
- 기타 control character는 v0.1 비목표 (path에 control char 들어오면 graceful 실패 acceptable).

`{branch}` / `{owner}` / `{name}` 인자는 도구 진입 시 검증된 값이라 추가 escape 불필요 (`-f query=...`에 raw 사용). path는 walker에서 `\` → `/` 정규화된 상대경로라 GraphQL 측 escape만 처리.

#### Timestamp 필드 — `committedDate` 사용 (`authoredDate` 금지)

REST `commits[].commit.committer.date`는 commit이 repository에 기록된 시점이다. GraphQL 측 등가 필드는 `committedDate`로 동일.

`authoredDate`는 commit author date라 cherry-pick / rebase / squash 시 committer date와 달라진다. cross-backend 정합성(P9 dogfooding `--backend rest` ↔ `--backend graphql`)을 깨므로 사용 금지.

#### 응답 파싱

stdout JSON:

```json
{
  "data": {
    "repository": {
      "ref": {
        "target": {
          "a0": { "nodes": [{ "committedDate": "2026-05-07T..." }] },
          "a1": { "nodes": [] }
        }
      }
    }
  },
  "errors": []
}
```

- `data.repository.ref.target.{alias}.nodes[0].committedDate` → `DateTime<Utc>`.
- `nodes`가 빈 배열이면 (해당 path commits 0개 — 신규 파일 케이스) → `GitlessError::Http("path '{path}': no commits found")` 또는 동등. v0.1 REST `fetch_last_commit_at`의 빈 commits 배열 매핑과 일관.
- alias 응답이 누락(map에 키 자체 없음)되면 `Http("path '{path}': missing in response")` — 통째 fail 정책 일관.

#### Partial errors 정책

GraphQL 응답에는 `data`와 `errors[]`가 공존할 수 있다 — 일부 alias만 실패한 부분 결과 케이스.

- `errors[]` 배열이 비어 있지 않으면 즉시 `errors[0].extensions.code` 매핑 후 통째 fail. 부분 결과 사용 안 함 (G-002 truncated 패턴 일관).
- 매핑 표는 `spec-error-contracts.md` § GraphQL error mapping (P2 신설). `data` 부분 결과는 무시.

#### batch size 변경 정책

batch size 변경 시 본 § GraphQL backend + ADR 0007 동시 갱신 (P6a raw data → P7a ADR 0007 확정, batch 200 default confirmed). 단위 테스트의 chunk 분할 시나리오 (300 paths → 200+100 등)도 결정값에 정렬.

변경 트리거 (ADR 0007 § 향후 재평가 트리거):
- secondary rate limit (점수 기반) 발생 시 batch size 하향 + ADR 0007 갱신.
- vault scale (수백~천 path) 측정에서 batch 100 vs 200 식별 가능한 차이 surface 시 raw data 기록 후 재결정.

### 병렬 호출 정책 (Latency)

| Backend | 정책 | 근거 |
|---|---|---|
| REST | `paths.par_iter()` rayon 8 concurrent | ADR 0003 (4.86x speedup, M5a 측정) |
| GraphQL | alias batching only, request 단위 순차 | ADR 0005 (한 request = 200 alias = 200 node 병렬) |

`MAX_COMMITS_CONCURRENCY = 8` 상수는 **REST 분기에서만 active**. GraphQL 모듈에서는 참조 0 (G-011 본문 — REST backend 한정 활성).

#### REST 분기 (ADR 0003 그대로)

- `fetch_last_commit_at`은 차이 있는 파일 N개에 대해 직렬 호출 시 N × subprocess spawn + GitHub round-trip latency 누적 → 큰 vault에서 사용자 인내심 한계.
- 해결안: rayon으로 병렬 호출, default **8 concurrent**. M5a 측정(commit `5e95312`)에서 13 path 기준 sequential 6.56s → rayon 8c 1.35s (4.86x speedup).
- 패턴: `paths.par_iter().map(|p| github::fetch_last_commit_at(client, repo, branch, p)).collect::<Result<Vec<_>, _>>()` 를 `rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap().install(...)`로 thread pool 명시 제어.
- 동시 요청 수 상한 = 8 (G-011, GitHub abuse detection 회피). 변경 시 G-011 + 본 섹션 + ADR 0003 동시 갱신.
- burst 시 gh stderr `429` 또는 abuse detection 신호 → `GitlessError::Http(...)`로 매핑 후 즉시 종료. exponential backoff은 v0.1 비목표 (Phase 4).

#### GraphQL 분기 (ADR 0005)

- request 단위 추가 병렬화 없음. paths를 `paths.chunks(GRAPHQL_BATCH_SIZE)`로 순차 호출.
- 한 request 안의 200 alias는 GitHub 측이 병렬 평가 — 도구 측 thread pool 0.
- secondary rate limit (점수 기반) 발생 시 batch size 하향 + ADR 0007 갱신 (rayon 재도입은 plan 외).

#### 공통

- `fetch_tree`(scan에서 1회) / `fetch_blob`(diff 명령에서만) 병렬화 대상 아님. backend 분기 없음 (GraphQL backend도 Trees / Blobs는 REST 사용 — § Backend 선택).

## Acceptance Criteria

마이그레이션 task M2a~M2c가 본 spec § REST backend를 충족한다 (ADR 0002 완료). Phase 4 task P3a/P5a/P9가 본 spec § GraphQL backend를 충족한다. 단위 테스트는 모두 `MockGhClient` stub 기반.

### REST backend

- `[AUTO]` `fetch_tree`가 MockGhClient stub 정상 응답에서 `Vec<RemoteFile>` 반환 (blob entry만 필터, `tree`/`160000`/`120000`/`100755` skip).
- `[AUTO]` `fetch_tree`가 MockGhClient stub 응답 `truncated: true` → `GitlessError::TreesTruncated` (PRD 검증 시나리오 12).
- `[AUTO]` `fetch_tree`가 MockGhClient stub 인증 실패 stderr 패턴 → `GitlessError::AuthFailed`.
- `[AUTO]` `fetch_tree`가 MockGhClient stub rate limit stderr 패턴 → `GitlessError::RateLimitExceeded { reset_at }` (PRD 검증 시나리오 11).
- `[AUTO]` `fetch_tree`가 MockGhClient stub 5xx stderr 패턴 → `GitlessError::Http(...)`.
- `[AUTO]` `fetch_blob`가 MockGhClient stub 200 base64 응답을 raw bytes로 디코딩.
- `[AUTO]` `fetch_blob`가 잘못된 base64 응답 → `GitlessError::Http(...)` 또는 적절한 매핑.
- `[AUTO]` `fetch_last_commit_at`가 MockGhClient stub 응답에서 첫 commit의 date를 `DateTime<Utc>`로 파싱.
- `[AUTO]` `fetch_last_commit_at`가 빈 commits 배열 응답 → `GitlessError::Http(...)` (예상 외 케이스).
- `[AUTO]` `RealGhClient::new()` 호출 후 `gh` 미존재 환경에서 첫 `api()` 호출이 `GitlessError::Config("gh CLI not found in PATH; install from https://cli.github.com/")` 반환.

### GraphQL backend (Phase 4)

- `[AUTO]` `fetch_last_commit_at_batch`가 MockGhClient stub 정상 응답에서 `path → DateTime<Utc>` HashMap 반환. 모든 입력 path가 결과에 포함.
- `[AUTO]` 빈 paths 입력 → `Ok(HashMap::new())` 즉시 반환 (`MockGhClient` 호출 0회).
- `[AUTO]` `paths.len() > GRAPHQL_BATCH_SIZE` (예: batch 200 + 300 paths) → `ceil(N / batch)` 회 호출 (300 → 2회: 200+100), chunk 응답이 합산된 단일 HashMap 반환.
- `[AUTO]` `errors[].extensions.code == "RATE_LIMITED"` → `GitlessError::RateLimitExceeded`.
- `[AUTO]` `errors[].extensions.code == "UNAUTHENTICATED"` → `GitlessError::AuthFailed`.
- `[AUTO]` `errors[].extensions.code == "NOT_FOUND"` → `GitlessError::Http(...)`.
- `[AUTO]` `errors[].extensions.code == "INTERNAL_SERVER_ERROR"` 또는 fallthrough 코드 → `GitlessError::Http(stderr/errors[] 원문)`.
- `[AUTO]` 응답에 `data` 부분 결과 + `errors[]` 비어 있지 않음 → 통째 fail (data 무시, errors 매핑 후 반환).
- `[AUTO]` 200 paths batch alias mangling: `a0` ~ `a199` 안전 매핑 + 응답 → path 역매핑 정합 (한 path 누락 0).
- `[AUTO]` path에 `"` / `\\` / `\n` 포함 시 GraphQL string escape 적용 (실제 호출 인자에 escape 형태 전달).
- `[AUTO]` `committedDate` 필드를 사용해 timestamp 추출 (`authoredDate` 사용 0). cross-backend P9 dogfooding `--backend rest` ↔ `--backend graphql` 결과 ScanReport 동일 검증.
