# Phase 4 Measurements — M5a / P6a / P6b / P6c

> Phase 4 (성능 최적화) 결정 입력 raw data. ADR 0003 / 0006 / 0007 / 0008의 측정 본체가 본 file로 이전 (Phase 5.14 task QQ, 2026-05-09). ADR은 결정 근거 (speedup 한 줄)만 retain, 본 file이 raw data canonical home.

## 환경 (공통)

- Windows 11 Pro 10.0.26100 / `gh` 2.88.1 / cargo 1.95.0 / release binary `target/release/gitless-sync.exe`.
- wall-clock = PowerShell `Measure-Command`. warm-up 회는 dropped.
- 대상: `KneShell/gitless-sync` @ main, local = `D:\00.Projects\02.Personal\05.gitless-sync`.
- commits API 분기 강제: 측정 직전 13개 commited `.md` 파일에 trailing newline 임시 추가 → 13 path가 `local_sha != remote_sha` 분기로 commits map fetch. 측정 종료 후 `git restore`로 복원.
- 명령어: `gitless-sync.exe scan --repo KneShell/gitless-sync --branch main --local <local> --summary-only`

## M5a — rayon 8 concurrent vs sequential (2026-05-07T01:16~01:18Z, commit `5e95312`)

ADR 0002 § Consequences "병렬 subprocess spawn 비용 vs 순차 호출 시간 trade-off" 결정 입력.

### (a) rayon 8 concurrent (default `MAX_COMMITS_CONCURRENCY = 8`, `par_iter` 경로)

- warm-up dropped: 1395.1 ms
- N=3 raw ms: 1360.5 / 1354.8 / 1337.7
- mean **1351.0 ms** / min 1337.7 / max 1360.5 / `(max-min)/mean` 1.7% (≪30%)

### (b) sequential (`par_iter` → `iter`, ThreadPool 우회 — 측정 후 revert 완료)

- warm-up dropped: 6897.0 ms
- N=3 raw ms: 6054.3 / 6917.5 / 6718.9
- mean **6563.6 ms** / min 6054.3 / max 6917.5 / `(max-min)/mean` 13.2% (<30%)

### Speedup

`6563.6 / 1351.0 ≈ 4.86x`. 13 commits API 호출 기준 이론 max 6.5x; 실제 4.86x는 subprocess spawn 오버헤드 + Trees/walk 공통 비용(~1.3s) 분모 잔류로 발생한 비례 손실. variance 양쪽 모두 30% 미만 → N=5 확장 불필요. gh exit≠0 발생 0회.

→ **ADR 0003 — rayon 유지** (`MAX_COMMITS_CONCURRENCY = 8` 그대로).

## P6a — GraphQL alias batch size 100 vs 200 (2026-05-07)

ADR 0007 결정 입력.

| 측정 시퀀스 | N | warm-up dropped (ms) | mean (ms) | min/max (ms) | (max-min)/mean |
|---|---|---|---|---|---|
| (a) batch 200 (default) | 5 | 1821.8 | **2076.7** | 1556.9 / 3236.4 | 80.9% |
| (b) batch 100 (임시 변경) | 3 | 1768.3 | **1694.7** | 1651.4 / 1755.9 | 6.2% |
| (c) batch 200 재측정 | 3 | 1731.9 | **3142.2** | 1567.5 / 6044.3 | 142.5% |

### 분석

1. **13 paths × 1 chunk**: batch 100/200 모두 `paths.chunks(N)` 결과 1개 chunk (13 ≤ 100 ≤ 200). 발사 GraphQL request의 alias 개수·body 크기 동일. **batch size 차이가 wire/server 단위에서 식별 불가 scale.**
2. **GraphQL `committedDate` latency 자연 변동 지배**: batch 200 두 시퀀스(a/c) 모두 high outlier(3236.4 / 6044.3 ms)에 mean 왜곡. batch 100 시퀀스는 outlier 0회. 동일 코드 경로 분포 갈림 = server-side latency 단발 spike 지배 신호. P6b GraphQL g2 단발 spike 10115ms도 동일 패턴.
3. **mean 비교 함정**: 100 vs 200(a) 1.225x, 100 vs 200(c) 1.854x. 동일 코드 경로(1 chunk)에서 1.85x 격차 = measurement noise.
4. **250+ path scale 검증 부재**: 13 path 환경에서 batch 100 vs 200 분리 불가. chunk 분할 발생 250+ path scale은 KneShell/gitless-sync 외 repo 또는 synthetic 시나리오 필요 (v0.1/v0.2 비목표).

→ **ADR 0007 — `GRAPHQL_BATCH_SIZE = 200` default 유지** (yagni + roadmap.md § Phase 4 GraphQL batching 권장 상한과 일관, 250+ path scale에서 chunk 분할 시 200이 wire round-trip 우위).

## P6b — REST vs GraphQL backend default (2026-05-07)

ADR 0006 결정 입력.

13 path scale measurement: REST 2484 ms / GraphQL cluster 1437 ms = **1.73x speedup** (typical). 1000 path scale 추정 ~38x (단건 N× REST 호출 → 1 chunk GraphQL alias batching).

GraphQL g2 시퀀스에서 단발 spike 10115ms 1회 관측 (P6a outlier와 동일 패턴). server-side latency 자연 변동.

→ **ADR 0006 — default `--backend rest` → `graphql` 전환** (REST는 explicit fallback retain).

## P6c — mtime cache hit speedup (2026-05-07)

ADR 0008 결정 입력. 50 path scale.

- **N=3 sequence**: cold mean **1324.8 ms** / warm mean **1274.0 ms** → speedup **1.040x** (variance 6.7% / 3.7%, <30%).
- **N=5 sequence (변동 재확인)**: cold mean **1335.0 ms** / warm mean **1351.6 ms** → speedup **0.988x** — warm이 cold보다 살짝 느림. variance 8.6% / 9.4%, <30%.

### Cache 정상 채워짐 확인

- 1차 scan 후 cache 파일 size 9063 bytes. JSON entries 50 (= summary `identical 30 + local_only_changed 20`). 모든 local file이 cache에 등록 → 2차 scan에서 100% cache hit 기대.
- cache `version` field = 1 (CACHE_VERSION 일관). 모든 entry는 `mtime` (UTC ISO-8601) + `sha` (hex) + `is_binary` (bool) — 형식 spec-config.md § cache 일관.

### 분석

- 두 측정 모두 speedup 0.99 ~ 1.04 범위. variance ≈ 4~9%로 낮은데도 1차/2차 mean 차이가 변동 범위 안. **cache 효과가 wall-clock measurement variance보다 작음**.
- dominant cost 분포: 1300ms 안에서 hash 50 file은 ~50ms (1KB-10KB 텍스트 × 50). 나머지 ~1250ms는 (i) cargo binary fork, (ii) `gh api` subprocess fork × 2 (Trees + GraphQL), (iii) Trees API 응답 다운로드 + 파싱, (iv) walker 파일 시스템 walk, (v) GraphQL 응답 파싱 + JSON 직렬화. cache는 hash phase만 단축 → 전체 대비 ~3-4% 영향. measured 결과(±5% noise) 내부.
- cache save는 cold/warm 모두 매 호출 발생 (`commands/scan/mod.rs::build_report` end). 9KB JSON serialize + tmp write + rename atomic 비용은 cold/warm 동일하게 발생 → cache 도입에 따른 *추가 비용*만 양쪽에 발생. lookup 효과는 그 위에 누적되는데 net zero.

### 임계값 매핑

§ Phase 4 사전 결정 §15 임계값 (P7 결정 자의성 회피, P6 raw data 매핑):
- **유지**: cache hit speedup ≥ 2x.
- **제거**: speedup < 1.5x (코드/의존성 부담만 — `dirs` crate 1 + cache.rs ~360 LOC).
- **경계 1.5~2.0x**: yagni 일관 시 제거 default.

measured speedup **1.040x (N=3) / 0.988x (N=5)** → **둘 다 < 1.5x 제거 영역** (경계도 아님).

→ **ADR 0008 — mtime cache 제거** + **ADR 0009 obsolete cascade**.

## References

- ADR 0003 (`docs/adr/0003-rayon-keep-or-drop.md`) — M5a 결정
- ADR 0006 (`docs/adr/0006-default-backend-graphql.md`) — P6b 결정
- ADR 0007 (`docs/adr/0007-graphql-batch-size.md`) — P6a 결정
- ADR 0008 (`docs/adr/0008-mtime-cache-keep-or-drop.md`) — P6c 결정
- ADR 0009 (`docs/adr/0009-internal-cache-readonly-exception.md`) — ADR 0008 cascade obsolete
- M5a 측정 commit `5e95312` (chore: M5a measurement (rayon 8c vs seq, 4.86x speedup))
- `docs/roadmap.md` § Phase 4 — P6b 1.73x summary
