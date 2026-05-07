# Guardrails

빌드 도중 새로 발견된 실패 패턴은 이 파일에 추가한다. 형식: `## G-NNN: 제목` → `- **문제**:` → `- **해결**:`.

## G-001: 자체 SHA는 git 표준이 아니다
- **문제**: `local_sha` / `remote_sha`를 `git hash-object`나 GitHub UI에서 본 SHA와 일치시키려는 시도가 발생할 수 있다. 본 도구의 해시는 LF-normalized + BOM-stripped 콘텐츠에 대한 자체 정의 SHA-1.
- **해결**: 검증 시 `git hash-object` 출력과 비교하지 말 것. empty blob (`e69de29...`) 같은 git 상수와 일치하는 건 우연이지 정합성 보장 아님. 자세한 정의는 `docs/specs/spec-hash-and-normalize.md`.

## G-002: GitHub Trees API truncation
- **문제**: 응답 7MB 또는 약 10만 entry 중 먼저 도달 시 `truncated: true`로 잘림. v0.1은 이 케이스 미지원.
- **해결**: `truncated == true` 시 `GitlessError::TreesTruncated` 즉시 반환, exit code 5. sub-tree 재귀 다운로드는 Phase 4/5에서 검토. v0.1에서 우회 시도 금지.

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
- **문제**: GitHub은 burst가 큰 동시 요청에 abuse detection을 발동시켜 일시 차단할 수 있다. rayon으로 commits API 병렬 호출 시 무제한으로 풀면 위험.
- **해결**: 동시 요청 수 = **8** (default). rayon thread pool 크기를 명시 제어: `rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap().install(|| paths.par_iter()...)` 또는 동등 수단. burst 시 server 측 throttle (429 응답) 가능성 있으나 exponential backoff은 v0.1 비목표 — `GitlessError::Http(...)`로 매핑 후 즉시 종료. 동시 요청 수 변경 시 본 G와 `spec-github-api.md` § 병렬 호출 정책 + ADR 0003 동시 갱신.
- **ADR 0003 (2026-05-07) confirmed**: rayon 유지 결정. M5a 측정(commit `5e95312`)에서 8 concurrent vs sequential 4.86x speedup 입증, abuse detection 0회 발동. cap = 8.

## G-012: Coverage 게이트는 phase-final task 책임
- **문제**: ralph build iteration의 step 4 (`cargo tarpaulin --engine llvm --workspace --out Stdout` ≥ 80%)는 phase 진행 중 다수 모듈이 `todo!()` 스텁 상태에서 자연스럽게 미달. 자기 task 내에서 80%를 끌어올릴 수단이 부재 (다른 task 파일 수정은 plan rule 위배).
- **해결**: 개별 task는 (a) 해당 task 파일의 라인 커버리지가 합리적 (≥80% 또는 100%), (b) `cargo fmt`/`clippy`/`cargo test` 통과를 충족하면 BLOCKED 처리하지 않고 진행. 전체 워크스페이스 80% 게이트는 phase-final task의 명시적 책임 (해당 task가 부족 모듈 식별 + 추가 테스트로 끌어올림).
- **spec-only task 면제**: 코드 변경 0인 spec-only task(`docs/specs/*.md` / `docs/ralph/*.md` / `docs/adr/*.md`만 수정)는 (a)/(b) 모두 trivially 통과 (코드 0이라 fmt/clippy/test 자동 pass + 라인 커버리지 변동 0, baseline 유지). 본 G의 면제 룰을 일반화 적용.

## G-013: cargo deny는 deny.toml 부재 시 모든 라이선스 reject
- **문제**: `cargo deny check`를 config 없이 실행하면 default 정책이 모든 라이선스를 reject (adler2 `0BSD OR MIT OR Apache-2.0`, aho-corasick `Unlicense OR MIT` 등).
- **해결**: workspace root `deny.toml`(허용 라이선스 화이트리스트, advisory 정책)이 박혀 있어야 cargo deny check가 정상 통과. 신규 transitive 도입 시 deny.toml 갱신 동반. project-ops.md의 cargo deny 항목은 deny.toml 부재 시 fail로 surface.

## G-015: 외부 명령 transient 실패 retry policy
- **문제**: ralph가 외부 명령(`gh api`, `cargo run -- scan` 등)을 호출하는 task에서 transient 실패(network 5xx, timeout, gh exit≠0 단발, rate limit transient)와 영구 실패(gh 미설치, 인증 만료, spec/code 충돌)를 구분 못 하면 단발 noise가 영구 [!]로 박힘 → fixpoint stuck. 사람 reset 필요해짐.
- **해결**: 외부 명령 transient 의심 실패는 동일 명령 N=3 + 30s backoff 재시도. 3회 모두 실패 시에만 [!] BLOCKED + 본 G-015 reference. Transient signal:
  - `gh api` exit code ≠ 0 + stderr에 `5xx` / `timeout` / `connection` / `rate limit` substring
  - `cargo run -- scan` exit code ≠ 0 + stderr에 network 키워드
  - 영구 signal (즉시 [!] + 별도 G-NNN 신규): gh stderr `HTTP 401`(인증 만료, 사람 회복 필요), `gh: command not found`/`Command::new` IO err(미설치), spec/code 정합 충돌, parse error 등.
- **auto-recovery**: G-015로 [!] 박힌 task는 `prompt-build.md` § 1 [!] auto-recovery 룰에 따라 다음 iteration 자동 [!]→[ ] reset. 사람 개입 0. 영구 사유는 사람 대기.
- **경계 모호 case**: stderr 패턴이 transient/permanent 분류 모호한 경우(예: gh `HTTP 503` 단발 vs backend 영구 issue). default는 transient retry 시도(N=3 + 30s backoff). 3회 실패 시 [!] + commit message에 stderr 본문 박음(grep 가능 형태). 사람이 패턴 보고 G-015 substring 추가 또는 새 G-NNN 박은 후 task reset.
