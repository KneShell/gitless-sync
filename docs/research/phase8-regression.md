# Phase 8 — schema v1.3 regression check (raw data)

> Phase 8.5 task AA (2026-05-10). Phase 8 신규 field
> (`diff_meaningful: Option<bool>` + `presence: "local_only"|"both"|"remote_only"`)
> 도입 + schema v1.2 → v1.3 bump이 Phase 7 task T baseline (1000 합성 vault +
> KneShell/gitless-sync remote scan, 4-state 카운트 1000 / 0 / 0 / 0)에 회귀
> 일으키지 않는지 확인. v0.4.0 release 직전 baseline 검증. 본 file은
> record-only — 분석은 task BB CHANGELOG entry 시점.

## Purpose / scope

Phase 7 task T는 schema v1.2 baseline (1000 합성 vault `local_only_changed` +
129 KneShell/gitless-sync `remote_only_changed` + 0 drift + 0 failed).
Phase 8은 같은 합성 vault 위에 v1.3 envelope + entry-level 신규 field
(`diff_meaningful`, `presence`)을 emit. 본 task는 binary regression 검증 —
- (a) 4-state status 카운트가 P7 baseline과 동일 의미 유지 (synth-vault local
  카운트 1000 + drift 0 + failed 0).
- (b) 신규 field가 envelope·기존 field에 영향 없음 (extra field만 add, 기존
  field 변경 0).
- (c) `presence`가 status와 일관 (local_only_changed → local_only,
  remote_only_changed → remote_only).
- (d) `diff_meaningful`은 spec § F1 정합 (Both 케이스에만 surface, 그 외
  None → `skip_serializing_if` omit).

## Hardware / Toolchain

| Field | Value |
|---|---|
| OS | Windows 11 Pro 10.0.26100 |
| Rust | stable 1.95.0 (project MSRV pin) |
| Profile | `release` (`target/release/gitless-sync.exe`) |
| `gh` CLI | 2.88.1 (2026-03-12) |
| Backend | `Backend::Graphql` (default; ADR 0006/0007) |
| Repo HEAD | `1603257` (Phase 8 task AA `[~]` start commit) |
| Synth seed | 42 (Phase 7 task T 동일 seed, 결정적 Xorshift64 PRNG) |
| Local files | 1000 markdown (`note-{i:05}.md`, NFC ASCII filename, LF content) |
| Local size span | 1059 ~ 102389 bytes (Phase 7 task T 정합, seed 42 결정) |
| Remote repo | `KneShell/gitless-sync` branch `main` |
| Remote tree size | 143 entries (visible, Phase 8 시점 — Phase 7 task T 시점 129 → +14 증가, 8.1~8.4 commits 누적 정합) |

## Reproduction

```powershell
cd D:\00.Projects\02.Personal\05.gitless-sync
cargo build --release

cargo run --release --quiet --package xtask -- synth-vault `
    --out tmp/synth-vault-42 --count 1000 --seed 42

# 3 runs (cold + 2 warm) — Phase 7 task T 패턴 mirror
1..3 | ForEach-Object {
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  & target/release/gitless-sync.exe scan `
      --local tmp/synth-vault-42 `
      --repo KneShell/gitless-sync `
      > "tmp/phase8-scan-$_.json" 2> "tmp/phase8-scan-$_.stderr"
  $sw.Stop()
  "$_=$($sw.Elapsed.TotalSeconds)"
}
```

`tmp/` 는 `.gitignore` (Phase 5.13.1 LL task) 정합 — repo에 commit 안 함.

## scan walltime (3 runs)

| Run | Walltime | Cache | Exit | stdout JSON | stderr |
|---|---:|---|---:|---|---|
| 1 (cold) | **0.830 s** | OS file cache cold | 0 | `tmp/phase8-scan-1.json` (233,642 bytes) | empty |
| 2 (warm) | **0.580 s** | warm | 0 | `tmp/phase8-scan-2.json` (233,642 bytes) | empty |
| 3 (warm) | **0.590 s** | warm | 0 | `tmp/phase8-scan-3.json` (233,639 bytes) | empty |

Best 0.580 s / mean 0.667 s / cold-warm delta 0.245 s. Phase 7 task T (best
0.758, mean 0.829) 대비 mean ~20% 짧음 — entry-level field 2개 추가는 SHA
hash + Trees fetch 지배 phase에 측정 가능 영향 없음 (variance noise 안). cargo
overhead 제거된 native binary 호출.

## Output structure (run 1, identical 4-state for runs 2/3)

```json
{
  "schema_version": "1.3",
  "scanned_at": "2026-05-10T...Z",
  "repo": "KneShell/gitless-sync",
  "branch": "main",
  "local_root": "tmp/synth-vault-42",
  "summary": {
    "identical": 0,
    "local_only_changed": 1000,
    "remote_only_changed": 143,
    "drift": 0,
    "failed": 0
  },
  "files": [/* 1143 entries */]
}
```

Top-level envelope 7 field (`schema_version` / `scanned_at` / `repo` / `branch`
/ `local_root` / `summary` / `files`) — Phase 7 task T와 동일 surface, 추가
field 0건. backward-compat lock test (Phase 8 task L `output.rs::tests` v1.0/
v1.1/v1.2 parser 정합) 충족.

### Status breakdown (3 runs identical)

| status | count | 합계 |
|---|---:|---|
| `identical` | 0 | path 교집합 0 (합성 vault `note-*` ⊥ KneShell/gitless-sync) |
| `local_only_changed` | 1000 | 합성 vault 전부 (remote 부재) |
| `remote_only_changed` | 143 | KneShell/gitless-sync repo 전부 (local 부재) |
| `drift` | 0 | path 교집합 없음 → 자연 0 |
| `failed` | 0 | 합성 vault 정책 (NFC ASCII / LF / no submodule·symlink·LFS) + KneShell/gitless-sync 함정 surface 0건 |

Phase 7 task T baseline (1000 / 129 / 0 / 0) 대비:
- `local_only_changed` — 1000 동일 (합성 vault 결정적, seed 42 동일).
- `remote_only_changed` — 129 → 143 (+14). KneShell/gitless-sync repo 자체가
  Phase 7→8 commits 누적으로 14 entry 증가 (Phase 8.1 spec/ADR + Phase 8.5
  xtask check_readme_examples 등). 본 도구 binary regression 아님.
- `drift` 0 / `failed` 0 — Phase 7 task T 정합.

### Presence × status cross-tabulation (run 1)

| status | presence | count |
|---|---|---:|
| `local_only_changed` | `local_only` | 1000 |
| `remote_only_changed` | `remote_only` | 143 |

ADR 0014 § F2 contract 정합 — `local_only_changed` ⇒ presence `local_only`,
`remote_only_changed` ⇒ presence `remote_only`. 합성 vault path 교집합 0이라
`Both` 케이스 0건. 일관성 100% (1143 / 1143).

### `diff_meaningful` surface (run 1)

`Some(_)` emit count: **0**.

ADR 0014 § F1 정합 — Hashed/Both 케이스에만 `Some(true|false)` emit. 합성
vault 측정에서는 path 교집합 0이라 모든 entry가 LocalOnly·RemoteOnly →
`diff_meaningful: None` → serde `skip_serializing_if` omit.

`Some(true)` (sha differ + normalize-diff) / `Some(false)` (sha differ +
normalize-equal) / `Some(false)` (identical) 분기 검증은 본 task 외 — Phase
8.2 task J (unit, 6 시나리오) + task K (integration, CRLF vs LF fixture)가
cover. 본 task는 4-state 회귀 부재 + presence 일관성 + diff_meaningful
omit-when-None만 surface.

### `failed[]` surface

`failed: 0` — 3 runs 일관. 합성 vault 정책 (`MAX_FILE_BYTES = 100 KB` / NFC
ASCII filename / LF / no submodule·symlink·LFS·long_path) + KneShell/gitless-sync
함정 surface 0건 (Phase 5 dogfood 정합). Phase 7.2 임계 (>50MB
memory_exceeded / >100MB file_too_large)와 약 500× ~ 1000× gap.

## Determinism observations

- `summary` 객체 — 3 runs 완전 동일 (`{"identical":0,"local_only_changed":1000,"remote_only_changed":143,"drift":0,"failed":0}`).
- Top-level metadata (`schema_version`, `repo`, `branch`, `local_root`)
  — 동일.
- `scanned_at` — run마다 다름 (실제 실행 시각).
- stdout JSON byte size — 233,642 / 233,642 / 233,639 (3바이트 변동, Phase 7
  task T 0.829 ms 측정과 동일 ordering noise pattern). `files[]` array entry
  ordering이 walker / Trees response interleave에 따라 변동 가능 — entry
  내용 자체는 동일.
- 본 항목은 raw observation 박제. byte-level determinism은 spec contract에
  명시되어 있지 않음.

## Cross-comparison — Phase 7 task T vs Phase 8 task AA

| 측정 | schema_version | local | remote | failed | mean walltime |
|---|---|---:|---:|---:|---:|
| Phase 7 task T (3 runs) | 1.2 | 1000 | 129 | 0 | 829 ms |
| Phase 8 task AA (3 runs) | 1.3 | 1000 | 143 | 0 | 667 ms |

**관찰** — 4-state status 의미 (synth-vault local 1000 / drift 0 / failed 0)
완전 보존. remote 카운트 +14는 KneShell/gitless-sync repo 자체 성장
(8.1~8.4 commits 누적), 도구 binary regression 아님. mean walltime 감소
~20% 는 OS 캐시 / system load variance 범위 — 본 task는 회귀 부재 검증
scope, walltime delta는 informative만.

**신규 field surface 영향** — `presence` 100% populate (1143 / 1143 entries),
`diff_meaningful` 0건 surface (정합, omit-when-None). envelope unchanged,
기존 field unchanged.

## Schema v1.2 → v1.3 backward-compat 검증

본 측정 출력 stdout JSON은 v1.3 (`schema_version: "1.3"`).

- Phase 7 task T 시점 v1.2 parser는 신규 field (`presence` / `diff_meaningful`)
  를 ignore — JSON spec § "additional properties" 정합 + Phase 8 task L
  `output.rs::tests` v1.0/v1.1/v1.2 parser lock test (각 envelope round-trip)
  통과로 ensure.
- v1.3 emitter는 기존 v1.2 client에게 새 field 보내고 client는 무시 — 호환
  대칭 충족.
- 4-state status enum 5값 (`identical` / `local_only_changed` /
  `remote_only_changed` / `drift` / `failed`) 변경 0 — 호출자 분기 그대로.

## Caveats

- Single host, single session. 다른 머신 / 부하 조건의 variance 미측정.
- 합성 vault path 전부 `local_only_changed` + remote 전부
  `remote_only_changed`라 path 교집합 (drift 후보)이 없어 `Both` presence +
  `diff_meaningful: Some(_)` surface 0건 — 본 분기는 unit/integration test
  (Phase 8.2 J/K/M task)이 cover.
- 합성 vault size 100 KB cap이라 Phase 7.2 file_too_large/memory_exceeded
  surface 0 — 본 임계 검증은 별도 fixture (`tests/fixtures/large-files/`)
  + unit test cover (Phase 7.2 task O 정합).
- Trees API truncation (G-002) — 합성 vault 1000 + remote 143 entries는
  truncation threshold (~7 MB / ~100K entries) 한참 아래. sub-tree fallback
  (Phase 7.1) 동작은 본 측정에 surface 안 함.
- 본 PC (dasgut user) eval 본문 vault (`C:\Users\admin\iCloudDrive\iCloud~md~obsidian`,
  다른 PC admin user) 접근 불가 — 합성 vault로 대체 (plan § Phase 8 환경
  주의 정합).

## Limitations

1. **`Both` 케이스 surface 0건**: 본 측정은 path 교집합 0 vault 구조라
   `presence: both` + `diff_meaningful: Some(_)` (true/false 분기) 직접
   surface 못 함. Phase 8.2 task J (6 시나리오 unit) + task K (CRLF vs LF
   integration fixture) + task M (v1.3 신규 acceptance N 시나리오) 분기
   검증 cover.
2. **단일 vault**: 합성 markdown 위주 → encoding/long-path/submodule/symlink
   함정 surface 0건. 함정 cascade는 spec § Phase 7 정합 별도 cover.
3. **단일 binary**: Phase 7 task T 시점 v0.3.0 binary와 본 측정 v0.4.0-prep
   binary 직접 byte-level diff 미수행 — 본 task는 의미 회귀 부재만
   surface (4-state + envelope + presence/diff_meaningful contract). v0.1
   → v0.2 회귀 자동 분류 framework (Phase 5 task W `phase5-regression.md`)
   같은 path-키 inner-join 자동 분류는 본 task scope 외 — schema v1.3 신규
   field add는 W2 패턴 (new optional field) 자연 정합.

## Conclusion: PASS

- 4-state count contract (1000 local + 0 drift + 0 failed) 완전 보존.
- envelope 7 field unchanged, 기존 entry field unchanged.
- 신규 field `presence` 100% populate + `diff_meaningful` omit-when-None 정합.
- schema v1.2 → v1.3 minor bump은 W2 패턴 (new optional field) — backward-compat
  parser lock test 통과 (Phase 8 task L).
- Phase 8 v0.4.0-prep release tag (task CC) 진입 baseline 충족.

## References

- `docs/research/phase7-vault-scale-bench.md` — Phase 7 task T baseline (1000
  / 129 / 0 / 0, schema 1.2).
- `docs/specs/spec-output-schema.md` § v1.3 — 신규 field spec authoritative.
- `docs/adr/0014-scan-diff-metadata-contract.md` — F1+F2 결정 trail.
- `docs/research/phase5-regression.md` — Phase 5 v0.1 → v0.2 binary regression
  framework (path-키 inner-join 자동 분류, W1~W6 화이트리스트). 본 task는
  schema v1.3 add-only 변화라 W2 (new optional field) 자연 정합 → 자동 diff
  framework 재실행 불필요.
