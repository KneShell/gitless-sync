# Guardrails

빌드 도중 새로 발견된 실패 패턴은 이 파일에 추가한다. 형식: `## G-NNN: 제목` → `- **문제**:` → `- **해결**:`.

## G-001: 자체 SHA는 git 표준이 아니다
- **문제**: `local_sha` / `remote_sha`를 `git hash-object`나 GitHub UI에서 본 SHA와 일치시키려는 시도가 발생할 수 있다. 본 도구의 해시는 LF-normalized + BOM-stripped 콘텐츠에 대한 자체 정의 SHA-1.
- **해결**: 검증 시 `git hash-object` 출력과 비교하지 말 것. empty blob (`e69de29...`) 같은 git 상수와 일치하는 건 우연이지 정합성 보장 아님. 자세한 정의는 `docs/specs/spec-hash-and-normalize.md`.

## G-002: GitHub Trees API truncation
- **문제**: 응답 7MB 또는 약 10만 entry 중 먼저 도달 시 `truncated: true`로 잘림. v0.1은 이 케이스 미지원.
- **해결**: `truncated == true` 시 `GitlessError::TreesTruncated` 즉시 반환, exit code 5. sub-tree 재귀 다운로드는 Phase 4/5에서 검토. v0.1에서 우회 시도 금지.

## G-003: GitHub API rate limit (2026-05-06 obsolete — gh가 rate limit 처리)
- **문제**: 인증 토큰 시간당 5,000 req. Commits API를 모든 파일에 호출하면 큰 vault에서 한도 초과.
- **해결**: Commits API(`fetch_last_commit_at`)는 **차이가 있는 파일에만** 호출. identical 파일에는 호출 금지. Rate limit 응답(403 + `X-RateLimit-Remaining: 0`) 감지 시 `GitlessError::RateLimitExceeded { reset_at }` 반환, exit code 3, 부분 결과 출력 금지.
- **2026-05-06 obsolete**: ADR 0002 마이그레이션으로 GitHub API 호출 통로가 `gh api` subprocess로 단일화. rate limit 자체 처리(retry/backoff) 책임이 도구 외부(`gh`)로 이동. `gh` CLI는 자체 rate limit 감지·재시도·메시지 출력을 한다. 본 도구는 `gh` exit≠0 + stderr 패턴(`API rate limit exceeded` 등) 매핑만 유지하며 (M1 spec § 에러 매핑 표), "Commits API는 차이 있는 파일에만 호출" 룰은 호출 비용 최적화 차원에서 그대로 유효. ADR 0002 § guardrail 처분 정렬.

## G-004: Windows 경로 vs forward slash
- **문제**: Windows는 백슬래시, GitHub은 forward slash. `path` 필드가 OS에 따라 달라지면 비교 키가 깨진다.
- **해결**: 비교 키와 출력 JSON `path` 필드는 항상 forward slash로 통일. `LocalFile::relative_path` 생성 시 `\` → `/` 변환.

## G-005: mtime 신뢰성 한계
- **문제**: 로컬 mtime은 touch / 복사 / iCloud 메타로 갱신되어 단조성이 없을 수 있다. 시간 비교만으로 push/pull 방향을 단정할 수 없다.
- **해결**: `local_mtime == remote_last_commit_at` 동률은 무조건 `Status::Drift`로 격하. 시간 비교는 휴리스틱일 뿐, 최종 판단은 호출자(사람 또는 AI)에게 맡긴다는 contract 유지.

## G-006: 비-UTF-8 텍스트 인코딩
- **문제**: EUC-KR 등 비-UTF-8 텍스트 파일은 NUL 바이트 휴리스틱에서 텍스트로 잘못 분류될 수 있고, UTF-8로 가정한 normalize에서 깨진다. 또는 바이너리 취급 시 CRLF 차이로 영구 drift.
- **해결**: v0.1은 UTF-8 가정. 비-UTF-8 처리는 Phase 5 (`docs/roadmap.md` 참조). 중간에 인코딩 감지 라이브러리(`encoding_rs` 등) 도입 금지 — 스코프 폭발.

## G-007: tarpaulin Windows 백엔드
- **문제**: `cargo tarpaulin` 기본 ptrace 백엔드는 Linux x86_64 전용. Windows는 LLVM 백엔드 필수. LLVM 백엔드는 non-zero exit code 처리·thread safety에 알려진 함정 있음.
- **해결**: `cargo tarpaulin --engine llvm` 명시 사용. Windows CI에서 false positive/negative 발생 시 `--engine` 옵션 또는 fallback 도입 검토. 페르소나가 "tarpaulin Windows 미지원"이라 단언해도 곧이듣지 말 것 (2026-04-27 fact check 완료).

## G-008: stdout / stderr 분리
- **문제**: 진행 로그·경고를 stdout에 섞으면 결과 JSON이 오염되어 AI 호출자가 파싱 실패.
- **해결**: stdout은 결과 JSON 한 덩어리만. 모든 진행 로그·경고·에러는 stderr. stderr 에러는 구조화 JSON 한 줄 (`error_code` + `message` + `context`). `println!` / `eprintln!`을 의식적으로 구분.

## G-010: 빈 파일 / 특수 mode entry
- **문제**: 빈 파일의 SHA-1 (`e69de29bb2d1d6434b8b29ae775ad8c2e48c5391`), Trees mode `160000` (submodule), `120000` (symlink), `100755` (executable) 등 v0.1에서 검증되지 않은 케이스.
- **해결**: 빈 파일은 처리하되 테스트로 검증 (이미 `hash::tests::empty_blob_matches_git`). submodule / symlink / executable mode entry는 v0.1에서 만나면 skip + warning(stderr) 또는 `failed[]` 처리. 본격 지원은 Phase 5.

## G-011: GitHub abuse detection / 동시 요청 제한
- **문제**: rate limit (5,000/h, G-003)와 별개로, GitHub은 burst가 큰 동시 요청에 abuse detection을 발동시켜 일시 차단할 수 있다. T09에서 rayon으로 commits API 병렬 호출 시 무제한으로 풀면 위험.
- **해결**: 동시 요청 수 = **8** (default). rayon thread pool 크기를 명시 제어: `rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap().install(|| paths.par_iter()...)` 또는 동등 수단. burst 시 server 측 throttle (429 응답) 가능성 있으나 exponential backoff은 v0.1 비목표 — `GitlessError::Http(...)`로 매핑 후 즉시 종료. 동시 요청 수 변경 시 본 G와 `spec-github-api.md` § 병렬 호출 정책 동시 갱신.

## G-012: 전체 80% 커버리지 게이트는 T12에서 통과
- **문제**: ralph build iteration의 step 4 (`cargo tarpaulin --engine llvm --workspace --out Stdout` ≥ 80%)는 T01~T11 진행 중에는 자연스럽게 미달. 다수 모듈이 `todo!()` 스텁 상태이므로 자기 task 내에서 80%를 끌어올릴 수단이 없다 (다른 task 파일을 건드리면 "Do NOT modify other tasks' files" 위배). T01~T03 모두 동일 조건에서 `[x]`로 완료된 선례가 있다.
- **해결**: 개별 task는 (a) 해당 task 파일의 라인 커버리지가 합리적 (≥80% 또는 100%), (b) `cargo fmt`/`clippy`/`cargo test` 통과를 충족하면 BLOCKED 처리하지 않고 진행. 전체 워크스페이스 80% 게이트는 T12의 명시적 책임이며, T12가 부족 모듈 식별 + 추가 테스트로 끌어올린다. 본 G의 예외는 T12에서 마지막으로 검증하므로 누적 위험 없음.
- **2026-05-06 추가 — spec-only task 케이스**: ADR 0002 마이그레이션의 M0/M1처럼 코드 변경 0인 spec-only task는 (a)/(b) 모두 trivially 통과 (코드 0이라 fmt/clippy/test 자동 pass + 라인 커버리지 변동 0, baseline 95.87% 그대로 유지). 본 G의 면제 룰을 일반화 적용. 명시적 추가 룰 불필요. M7이 ADR 0002 마이그레이션 최종 80% 게이트 책임 (T12 역할).

## G-013: cargo deny는 deny.toml 부재 시 모든 라이선스 reject
- **문제**: `cargo deny check`를 config 없이 실행하면 default 정책이 모든 라이선스를 reject (adler2 `0BSD OR MIT OR Apache-2.0`, aho-corasick `Unlicense OR MIT` 등). T07에서 base64 = "0.22"를 직접 의존으로 추가했을 때 첫 발현. 단 base64는 이미 Cargo.lock에 transitive로 박혀 있던 동일 버전이라 dep tree 자체는 무변화 (`Cargo.lock` diff 1 line, `+ "base64"` 한 줄만 추가).
- **해결**: T07~T08 같은 단일 dep 추가 task는 (a) `cargo audit` 통과(exit 0) + (b) Cargo.lock에 새 transitive crate가 등장하지 않음을 확인하면 cargo deny 게이트 보류 가능. 정식 `deny.toml`(허용 라이선스 화이트리스트, advisory 정책) 작성은 T09가 rayon 등 신규 transitive를 도입할 때 또는 별도 인프라 task로 분리. project-ops.md의 cargo deny 항목은 deny.toml 작성 후 강제. T09c에서 workspace root `deny.toml` 작성 완료(2026-04-28); 이후 cargo deny check 정상 강제.

## G-015: 외부 명령 transient 실패 retry policy
- **문제**: ralph가 외부 명령(`gh api`, `cargo run -- scan` 등)을 호출하는 task(M2a 환경 점검, M5a 측정, M8 dogfooding)에서 transient 실패(network 5xx, timeout, gh exit≠0 단발, rate limit transient)와 영구 실패(gh 미설치, 인증 만료, spec/code 충돌)를 구분 못 하면 단발 noise가 영구 [!]로 박힘 → fixpoint stuck. 사람 reset 필요해짐.
- **해결**: 외부 명령 transient 의심 실패는 동일 명령 N=3 + 30s backoff 재시도. 3회 모두 실패 시에만 [!] BLOCKED + 본 G-015 reference. Transient signal:
  - `gh api` exit code ≠ 0 + stderr에 `5xx` / `timeout` / `connection` / `rate limit` substring
  - `cargo run -- scan` exit code ≠ 0 + stderr에 network 키워드
  - 영구 signal (즉시 [!] + 별도 G-NNN 신규): gh stderr `HTTP 401`(인증 만료, 사람 회복 필요), `gh: command not found`/`Command::new` IO err(미설치), spec/code 정합 충돌, parse error 등.
- **auto-recovery**: G-015로 [!] 박힌 task는 `prompt-build.md` § 1 [!] auto-recovery 룰에 따라 다음 iteration 자동 [!]→[ ] reset. 사람 개입 0. 영구 사유(G-016+)는 사람 대기.

## G-016: ralph 환경 cargo/rustup 미설치 (코드 task만 BLOCKED 사유)
- **문제**: 2026-05-06 M0 진행 중 발견. ralph 실행 환경에 `cargo`/`rustup` 미설치 (PATH·`%USERPROFILE%\.cargo\bin`·`C:\Program Files\Rust` 등 표준 위치 모두 부재). `cargo fmt`/`clippy`/`test`/`tarpaulin` 실행 불가 → 영구 신호 (G-015 transient retry 대상 아님).
- **해결**: 사람이 ralph 환경에 rustup 설치 (https://rustup.rs/) 후 재진입. 설치 후 `MSRV 1.95.0` (`rust-toolchain.toml`) 자동 fetch. 설치 완료 시 본 G-016 obsolete 마크.
- **spec-only task 면제**: M0/M1 같은 spec-only task(`docs/specs/*.md` / `docs/ralph/*.md` / `docs/adr/*.md`만 수정)는 코드 변경 0이므로 baseline 무영향. `prompt-build.md` § 3 step 1~3을 자연 면제로 처리하여 진행 가능. step 4(coverage)는 § 2 spec-only 룰(G-012)로 이미 면제. 본 면제는 spec-only task 한정.
- **코드 task 도달 시**: 첫 코드 task(M2a 등)에서 `cargo --version` 실패 즉시 본 G-016 reference로 [!] BLOCKED. 사람이 rustup 설치 후 [!] → [ ] reset (G-015 auto-recovery 대상 아님 — 영구 신호).
- **2026-05-06 obsolete**: 사람이 사전 처리 완료 — `winget install Rustlang.Rustup` (rustup 1.29.0 + cargo 1.95.0, `rust-toolchain.toml` MSRV 1.95.0 자동 fetch) + Visual Studio BuildTools 2022 17.14.31 (이미 설치됨) 가용 확인. `cargo build` 통과 (28s, MSVC linker 정상). M2a `[!]` → `[ ]` reset 후 ralph 재진입 가능.

## G-014: scan 모듈의 `_with_base` 패턴이 비-`_with_base` `pub fn`을 dead code로 만듦
- **문제**: T09b에서 `GITLESS_API_BASE` env override를 위해 `run` / `fetch_tree` / `fetch_blob` / `fetch_last_commit_at` 각 함수에 `*_with_base(base, …)` 형제를 도입. main.rs는 base override capability 때문에 항상 `*_with_base`를 직접 호출하므로 비-`_with_base` 래퍼는 production·test 어디서도 호출되지 않음. binary crate에서 `pub` 가시성은 dead_code 분석을 막지 못하므로 T12 baseline cleanup(`#![allow(dead_code, clippy::needless_pass_by_value)]` 제거) 시 4개의 dead_code 에러로 surface. 추가로 `RemoteFile.mode` / `RemoteFile.size`는 `fetch_tree_with_base`가 채우지만 production read 경로 없음 (테스트에서만 read). 또한 `clippy::needless_pass_by_value`도 동시에 제거하면 `run_with_base(args: ScanArgs|DiffArgs)` 와 `walkdir_to_io(err: walkdir::Error)`가 함께 surface.
- **해결**: 클린업 자체는 단순 (deletion or `#[allow]` per-item)이지만 T12의 plan rule이 "잔존 dead_code 발견 시 별도 fix task + `[!]` BLOCKED" 명시. 따라서 (a) `crates/gitless-sync/src/main.rs`의 `#![allow(dead_code, clippy::needless_pass_by_value)]` 복원, (b) 별도 T14 task로 분리, (c) T12를 `[!]` BLOCKED. T14 acceptance: dead `pub fn` 4개 + dead `RemoteFile.{mode,size}` 필드 삭제, needless_pass_by_value 3건은 `&` 참조로 변경, `cargo clippy --all-targets -- -D warnings` 통과. T14 완료 후 사람이 T12를 `[ ]`로 reset하여 coverage gate 진행.

## G-017: gh `-F` 인자가 commits API GET 요청을 POST로 전환시킴 (영구 코드/spec 버그)
- **문제**: 2026-05-06 M5a 측정 진입 직전 발견. `spec-github-api.md` § fetch_last_commit_at + `crates/gitless-sync/src/commands/scan/github.rs`의 박힌 인자 패턴 `gh api repos/{}/{}/commits -F sha={branch} -F path={path} -F per_page=1`은 `-F`만 사용 시 gh CLI가 method를 POST로 자동 전환 (gh `--method` doc: "default GET ... but with -F/-f Override the method"). GitHub commits API는 GET endpoint이므로 POST 시 `gh: Not Found (HTTP 404)` 반환 → `fetch_last_commit_at`가 모든 차이 파일에 대해 실패 → exit 1.
  - 직접 검증 (2026-05-06): `gh api repos/KneShell/gitless-sync/commits -F sha=main -F path=CLAUDE.md -F per_page=1` → 404. 동일 인자에 `-X GET`만 추가 시 정상 commit 배열 반환.
  - M2b1/M2b2 unit test는 MockGhClient stub 기반이라 실제 호출 method 검증 부재. M5a에서 처음 surface (M8 dogfooding 직전 단계).
- **영구 신호**: spec/code 정합 충돌 (`prompt-build.md` § 3 G-015 영구 신호 분류). G-015 transient retry 무의미 — gh가 일관되게 404 반환.
- **fix 방향**: 별도 fix task (M5a-fix 또는 M2-followup) 분해 필요. 후보 패치 — (a) `args` 빌드에 `"-X".to_string(), "GET".to_string()` prepend, (b) 또는 `-F`를 query string으로 옮겨 `format!("repos/{}/{}/commits?sha={}&path={}&per_page=1")` + `-F` 제거. spec(`spec-github-api.md` § fetch_last_commit_at 예시) + 코드(`commands/scan/github.rs::commits_args`) + unit test(`commands/scan/mod.rs::commits_args` 헬퍼) + integration test(`tests/integration.rs`의 commits_args) 4곳 동시 갱신 필요.
- **task**: M5a는 본 G-017 reference로 [!] BLOCKED. 사람이 fix task 신규 추가 후 M5a [!] → [ ] reset (G-015 auto-recovery 대상 아님 — 영구 신호).
- **2026-05-07 fixed by M2d**: 후보 (a) 채택. M5a [!] → [ ] reset 후 ralph 재진입 가능.
