# ADR 0003: rayon 유지 결정 (commits API 병렬 호출)

- **Status**: Accepted
- **Date**: 2026-05-07
- **Resolves**: ADR 0002 § Consequences "병렬 subprocess spawn 비용 vs 순차 호출 시간 trade-off는 측정해 rayon 유지/제거 결정 — 별도 task" open question
- **Related**: ADR 0001 (gh subprocess 채택), ADR 0002 (v0.1 ureq → gh 일괄 마이그레이션), M5a 측정 commit `5e95312`, `docs/ralph/guardrails.md` G-011

## Context

ADR 0002는 ureq → gh subprocess 마이그레이션을 결정하면서 한 가지를 미정으로 남겼다: rayon 병렬 호출 정책(default 8 concurrent)을 그대로 유지할지, 아니면 gh subprocess spawn 비용 때문에 순차 호출이 오히려 빠른지 — 이 결정은 실측 후로 미뤘다.

M5a (2026-05-07T01:16~01:18Z, commit `5e95312`)에서 자율 측정을 마쳤다. 환경·명령어·raw data는 다음과 같다.

- **환경**: Windows 11 Pro 10.0.26100 / gh 2.88.1 / cargo 1.95.0 / release binary (`target/release/gitless-sync.exe`). wall-clock은 PowerShell `Measure-Command`.
- **대상**: `KneShell/gitless-sync` @ main, local = `D:\00.Projects\02.Personal\05.gitless-sync`. 측정 직전 8개 commit된 `.md` 파일에 trailing newline 임시 추가 → 13개 path가 commits API 호출 분기 (양쪽 SHA 상이). 측정 종료 후 `git restore`로 복원, 코드 임시 변경(`par_iter` → `iter`)도 동시 revert.
- **명령어**: `gitless-sync.exe scan --repo KneShell/gitless-sync --branch main --local <local> --summary-only`

### (a) rayon 8 concurrent (default `MAX_COMMITS_CONCURRENCY = 8`, `par_iter` 경로)

- warm-up dropped: 1395.1 ms
- N=3 raw ms: 1360.5, 1354.8, 1337.7
- mean **1351.0 ms** / min 1337.7 / max 1360.5 / `(max-min)/mean` 1.7% (≪30%)

### (b) sequential (`par_iter` → `iter`, ThreadPool 우회 — 측정 후 revert 완료)

- warm-up dropped: 6897.0 ms
- N=3 raw ms: 6054.3, 6917.5, 6718.9
- mean **6563.6 ms** / min 6054.3 / max 6917.5 / `(max-min)/mean` 13.2% (<30%)

### Speedup

`6563.6 / 1351.0 ≈ 4.86x` (rayon 8c가 sequential보다 약 5배 빠름). 13 commits API 호출 기준 이론 max speedup 6.5x; 실제 4.86x는 subprocess spawn 오버헤드 + Trees/walk 공통 비용(약 1.3s)이 분모에 남아 발생한 비례 손실.

variance 양쪽 모두 30% 미만 → N=5 확장 불필요. gh exit≠0 발생 0회 → G-015 retry 미발동.

## Decision

**rayon 유지 (① keep).** `MAX_COMMITS_CONCURRENCY = 8` 그대로. 변경 없음.

근거:

- 4.86x speedup은 vault scale(수백 파일 중 수십 차이 파일) repo에서 사용자 인내심 한계와 직접 충돌하는 비용이다. M5a baseline 13 path에서 6.5초 → 1.4초. vault scale에서는 절대 시간 차이가 더 벌어진다.
- gh subprocess spawn 비용이 rayon 효과를 상쇄할 거란 사전 우려는 측정으로 기각. spawn 오버헤드는 존재하지만 round-trip latency가 지배적이라 병렬 효과가 그대로 살아남는다.
- 의존성 비용 0: rayon은 이미 `Cargo.lock`에 transitive로 박혀 있고, `cargo deny`/`cargo audit` 게이트는 통과 상태. 제거해도 추가 이득은 binary size 한 줄 차이 정도.
- 제거 시 코드 변경(par_iter → for 또는 iter) + spec/guardrail 갱신 + 테스트 영향 검토가 필요하나, 5x speedup을 포기할 만한 동기는 부재.

abuse detection 위험은 default 8로 cap된 상태에서 M5a 측정 중 0회 발생. burst가 GitHub abuse detection을 트리거하면 stderr `429`/abuse 신호 → `GitlessError::Http(...)` 매핑 후 즉시 종료(v0.1 정책). exponential backoff은 v0.1 비목표.

## Consequences

### G-011 (rayon 8 concurrent abuse 회피)
- **유지.** ADR 0002에서 "도구 책임 종료" 가능성으로 거론됐으나, 본 결정으로 rayon이 살아 있는 한 G-011은 활성 guardrail. `MAX_COMMITS_CONCURRENCY = 8` cap이 도구 측 abuse 방지 정책으로 그대로 유효.
- guardrail 본문에 "**2026-05-07 confirmed by ADR 0003**: rayon 유지 결정. M5a 측정 결과 8 concurrent + sequential 비교에서 4.86x speedup. cap 변경 시 본 G + spec § 병렬 호출 정책 + ADR 0003 동시 갱신." 한 줄 추가.

### `Cargo.toml`
- **변경 없음.** `rayon = "1"` 그대로.

### `spec-github-api.md` § 병렬 호출 정책
- "⚠️ M5b 결과 미정 박스" blockquote 제거. baseline 정책을 확정 박제로 격상.
- 본문에 ADR 0003 + M5a measurement 참조 한 줄 추가.

### 코드
- `crates/gitless-sync/src/commands/scan/mod.rs`의 `MAX_COMMITS_CONCURRENCY = 8` 상수 + `fetch_commit_dates_parallel`의 rayon ThreadPool 패턴 그대로 유지. 변경 0.

### 향후 재측정 트리거
- abuse detection이 production에서 발생하기 시작하면 cap 하향 또는 exponential backoff 도입 검토. v0.1 비목표 — Phase 4에서.
- GraphQL backend 활성화 (Phase 4) 시 단건 N× 호출 패턴 자체가 alias batching으로 대체되므로 rayon 정책 재평가. 별도 ADR 분기 가능성 있음.

## References

- ADR 0001 § D1 (gh subprocess 채택)
- ADR 0002 § Consequences "병렬 subprocess spawn 비용 vs 순차 호출 시간 trade-off"
- M5a 측정 commit `5e95312` (chore: M5a measurement (rayon 8c vs seq, 4.86x speedup))
- `docs/ralph/guardrails.md` § G-011
- `docs/specs/spec-github-api.md` § 병렬 호출 정책 (Latency)
