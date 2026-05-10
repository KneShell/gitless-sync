# Phase 7 — vault scale 1000+ dogfood (raw data)

> Phase 7.3 task T (2026-05-10). 합성 vault 1000+ markdown scale에서
> end-to-end `gitless-sync scan` walltime + 4분류 결과 + failed[] surface
> 검증을 raw data로 박제. spec-domain-pitfalls.md § Phase 7 — 합성 vault
> generator 정책 정합. 본 file은 record-only — 분석은 task W
> (`docs/research/phase7-vault-scale-bench.md` 종합 §) 또는 ADR 갱신 시점.

## Hardware / Toolchain

| Field | Value |
|---|---|
| OS | Windows 11 Pro 10.0.26100 |
| Rust | stable 1.95.0 (project MSRV pin) |
| Profile | `release` (`target/release/gitless-sync.exe`) |
| `gh` CLI | 2.88.1 (2026-03-12) |
| Backend | `Backend::Graphql` (default; commit + tree fetch GraphQL batched) |
| Repo HEAD | `71177a0c678c174ae5cbd3f06f868cbf764b1225` (Phase 7.3 task T `[~]` start commit) |
| Synth seed | 42 (default, deterministic Xorshift64 PRNG) |
| Local files | 1000 markdown (`note-{i:05}.md`, NFC ASCII filename, LF content) |
| Local size span | 1059 ~ 102389 bytes (mean 50,178; total ~47.85 MB on disk) |
| Remote repo | `KneShell/gitless-sync` branch `main` |
| Remote tree size | 129 entries (visible to scan; gitless-sync project itself) |

### Repo selection rationale

`{public-test-repo}` placeholder는 task spec에 미고정. T는 main bench
(local 1000+ scale processing); U는 cross-check sanity (linux/torvalds
또는 동등 1000+ entry repo). T에서는 small remote (`KneShell/gitless-sync`,
129 entries)로 isolate해 local hash + walker + classification 시간이
remote tree fetch 시간에 가려지지 않게 함. 합성 vault path
(`note-{i:05}.md`)는 어느 real repo와도 안 겹쳐 결과는 전부 local_only +
remote_only — remote 선택은 Trees fetch 시간만 흔들고 local 처리 시간엔
영향 없음.

## Reproduction

```powershell
cd D:\00.Projects\02.Personal\05.gitless-sync
cargo build --release
cargo run --release --quiet --package xtask -- synth-vault `
    --out tmp/synth-vault-42 --count 1000 --seed 42

# 3 runs (cold + 2 warm)
1..3 | ForEach-Object {
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  & target/release/gitless-sync.exe scan `
      --local tmp/synth-vault-42 `
      --repo KneShell/gitless-sync `
      > "tmp/scan-$_.json" 2> "tmp/scan-$_.stderr"
  $sw.Stop()
  "$_=$($sw.Elapsed.TotalSeconds)"
}
```

`tmp/` 는 `.gitignore` (Phase 5.13.1 LL task) 정합 — 본 측정 결과는
repo에 commit되지 않는다.

## Setup walltime

| Step | Walltime | Notes |
|---|---:|---|
| `cargo build --release` (cold compile) | 15.58 s | gitless-sync workspace 1회 release rebuild |
| `cargo run --release --package xtask -- synth-vault` | 27.27 s | xtask cold compile + 1000 file write (file I/O 지배) |

xtask compile cost (`synth-vault` 첫 호출)이 27 s에 포함 — 두 번째 invocation
(이미 `target/release/xtask.exe` 빌드된 상태)은 file I/O만이라 1 ~ 2 s 예상
(본 측정에서는 cold만 박제, 본 task는 1회 generate 만 필요).

## scan walltime (3 runs)

| Run | Walltime | Cache | Exit | stdout JSON path | stderr |
|---|---:|---|---:|---|---|
| 1 (cold) | **0.880 s** | OS file cache cold | 0 | `tmp/scan-1.json` (203,457 bytes) | empty |
| 2 (warm) | **0.758 s** | warm | 0 | `tmp/scan-2.json` (203,460 bytes) | empty |
| 3 (warm) | **0.848 s** | warm | 0 | `tmp/scan-3.json` (203,457 bytes) | empty |

Best 0.758 s / mean 0.829 s / cold-warm delta 0.122 s. cargo overhead
제거된 native binary 호출 — `Stopwatch` 측정값.

## Output structure (run 1, identical for runs 2/3)

```json
{
  "schema_version": "1.2",
  "scanned_at": "2026-05-10T04:38:38.924033Z",
  "repo": "KneShell/gitless-sync",
  "branch": "main",
  "local_root": "tmp/synth-vault-42",
  "summary": {
    "identical": 0,
    "local_only_changed": 1000,
    "remote_only_changed": 129,
    "drift": 0,
    "failed": 0
  },
  "files": [/* 1129 entries */]
}
```

### Status breakdown (3 runs identical)

| status | count | 합계 |
|---|---:|---|
| `identical` | 0 | — |
| `local_only_changed` | 1000 | 합성 vault `note-{i:05}.md` 전부 (remote 부재) |
| `remote_only_changed` | 129 | KneShell/gitless-sync repo 전부 (local 부재) |
| `drift` | 0 | path 교집합 없음 → 자연 0 |
| `failed` | 0 | Phase 7.2 cap 100KB 합성 vault → file_too_large/memory_exceeded surface 안 함 (예상 정합) |

### `failed[]` surface 검증

`failed[]` 0건. 합성 vault 정책 (`MAX_FILE_BYTES = 100 KB`) ↔ Phase 7.2
임계 (>50MB memory_exceeded / >100MB file_too_large) gap 약 500× ~ 1000×
— 자연 미surface.

다른 함정 reason도 0건 (NFC filename / LF / ASCII / submodule·symlink·
LFS·long_path 모두 합성 정책상 generate되지 않음, spec § Phase 7 정합).

## Determinism observations

- `summary` 객체 — 3 runs 완전 동일 (`{"identical":0,...,"failed":0}`).
- Top-level metadata (`schema_version`, `repo`, `branch`, `local_root`)
  — 동일.
- `scanned_at` — run마다 다름 (실제 실행 시각).
- `tmp/scan-{1,2,3}.json` byte-level SHA-256 — 모두 다름. JSON file size도
  3바이트 변동 (203,457 / 203,460 / 203,457). `files[]` array entry
  ordering이 walker / Trees response interleave에 따라 변동 가능 — entry
  내용 자체는 동일.
- 본 항목은 raw observation 박제. byte-level determinism은 spec
  contract에 명시되어 있지 않음 — task W 종합 시 별도 검토 가능.

## Caveats

- Single host, single session. 다른 머신 / 부하 조건의 variance 미측정.
- 합성 vault path 전부 `local_only_changed` + remote 전부
  `remote_only_changed`라 Commits API call은 path 교집합 (drift 후보)이
  없어 0회 — diff/blob fetch 비용 미측정. 함정 surface 케이스 (drift
  분류된 path가 .gitattributes·LFS·BOM 등 cascade 진입)도 합성 vault에는
  존재 안 함.
- 합성 vault size 100KB cap이라 Phase 7.2 file_too_large/memory_exceeded
  surface 0 — 본 임계 검증은 별도 fixture (`tests/fixtures/large-files/`)
  + unit test (`hash_local::tests`) 으로 cover (Phase 7.2 task O 정합).
- Trees API truncation (G-002) — 합성 vault 1000 + remote 129 entries는
  truncation threshold (~7 MB / ~100K entries) 한참 아래. sub-tree
  fallback (Phase 7.1) 동작은 본 측정에 surface 안 함 — 별도 integration
  test (`tests/scan_trees_fallback.rs`) cover.

## Public repo cross-check (git/git, sanity)

> Phase 7.3 task U (2026-05-10). T main bench (small remote 129
> entries) 결과의 cross-check sanity — public 1000+ entry repo 1회 manual
> scan. spec-domain-pitfalls.md § Phase 7 — public repo cross-check 정합.
> 본 § 도 record-only — 분석은 task W (`docs/research/phase7-vault-scale-bench.md`
> 종합 §) 또는 ADR 갱신 시점.

### Repo selection rationale

spec § Phase 7 — public repo cross-check 명시: "linux/torvalds 또는
동등 1000+ entry repo. commit sha 박제 (HEAD floating 금지)". linux/torvalds
는 5 K+ directories 보유 → sub-tree fallback (`shared/github/trees/fallback/recursive/walk.rs`,
Phase 7.1 task D)이 directory마다 1 `gh api ...trees/{sub_tree_sha}`
호출이라 `MAX_TREE_CALL_BUDGET = 1000` cap 즉시 위반 →
`GitlessError::TreesTruncated` (exit 5)로 BLOCKED 보장. spec 명시
"또는 동등 1000+ entry repo" 동등 후보로 medium-large repo 선정 — Trees
API recursive=1 응답이 truncated=false 영역 (sub-tree fallback 미진입).

`git/git` 선정 근거:
- default_branch `master`, HEAD sha = `94f057755b7941b321fd11fec1b2e3ca5313a4e0`
  (2026-05-10 박제, HEAD floating 차단).
- raw Trees API recursive=1 응답: `truncated=false`, 4964 entries —
  1회 fetch (sub-tree fallback 미진입).
- type 분포: `blob=4739` (mode 100644: 3458 / 100755: 1278 / 120000: 3) +
  `tree=224` (sub-directory placeholder, classify_tree_entry silent drop) +
  `commit=1` (submodule, type=commit entry).
- 합성 vault path (`note-{i:05}.md`)와 git/git 전체 path 완전 disjoint
  → all local_only + remote_only pattern 유지 (T와 동일 구조).
- failed[] surface 가능성 — git/git에는 submodule (`sha1collisiondetection`)
  + 3 symlinks (`RelNotes`, `subprojects/git-gui`, `subprojects/gitk`) 존재
  → spec § Submodule / Symlink detect-only 실파일 검증.

Sub-tree fallback dogfood는 본 task 범위 외 — 별도 integration test
(`tests/scan_trees_fallback.rs`, Phase 7.1 task G) + unit test
(`shared/github/trees/fallback/recursive/walk.rs::tests`, task F)로 cover.

### Hardware / Toolchain

| Field | Value |
|---|---|
| OS | Windows 11 Pro 10.0.26100 |
| Rust | stable 1.95.0 (project MSRV pin) |
| Profile | `release` (`target/release/gitless-sync.exe`) |
| `gh` CLI | 2.88.1 (2026-03-12) |
| Backend | `Backend::Graphql` (default) |
| gitless-sync HEAD | `06a9d65` (Phase 7.3 task U `[~]` start commit) |
| Local files | 1000 markdown (T `tmp/synth-vault-42` 재사용 — 같은 seed 42) |
| Remote repo | `git/git` branch `master` |
| Remote HEAD sha | `94f057755b7941b321fd11fec1b2e3ca5313a4e0` |
| Remote tree raw entries | 4964 (Trees API recursive=1, `truncated=false`) |

### Reproduction

```powershell
cd D:\00.Projects\02.Personal\05.gitless-sync

# 1) HEAD sha 박제 일치 검증 (HEAD floating 차단)
$head = gh api repos/git/git/commits/master --jq '.sha'
# expected: 94f057755b7941b321fd11fec1b2e3ca5313a4e0

# 2) scan 1회 (manual sanity, single run)
$sw = [System.Diagnostics.Stopwatch]::StartNew()
& target/release/gitless-sync.exe scan `
    --local tmp/synth-vault-42 `
    --repo git/git --branch master `
    > tmp/scan-public-gitgit.json 2> tmp/scan-public-gitgit.stderr
$sw.Stop()
"exit=$LASTEXITCODE elapsed_seconds=$($sw.Elapsed.TotalSeconds)"
```

`tmp/` 는 `.gitignore` (Phase 5.13.1 LL task) 정합 — repo에 commit 안 함.

### scan walltime (1 run, manual sanity)

| Run | Walltime | Cache | Exit | stdout JSON path | stderr |
|---|---:|---|---:|---|---|
| 1 | **1.109 s** | warm (T 측정 직후 동일 세션) | 4 | `tmp/scan-public-gitgit.json` (940,267 bytes) | 1줄 PARTIAL_FAILURE |

T main bench (best 0.758 s, mean 0.829 s, small 129-entry remote)와
비교 — Trees recursive=1 응답이 ~129 → 4964 entry로 ~38× 증가했지만
walltime 증가는 +0.28 ~ +0.35 s (~+35%)에 그침. local-side 1000 file
hash (CRLF detect + SHA-1) 시간이 지배적 + remote-side는 단일 batch
응답 1회 + JSON parse — entry 수 증가에 sub-linear scale 정합. variance
미측정 (single run) — T 3 runs ~0.12 s variance 패턴이 본 측정에도
적용된다고 가정.

### Output structure

```json
{
  "schema_version": "1.2",
  "scanned_at": "2026-05-10T04:52:02.160188Z",
  "repo": "git/git",
  "branch": "master",
  "local_root": "tmp/synth-vault-42",
  "summary": {
    "identical": 0,
    "local_only_changed": 1000,
    "remote_only_changed": 4736,
    "drift": 0,
    "failed": 4
  },
  "files": [/* 5740 entries */]
}
```

stderr 1줄 (single-line JSON):
```json
{"error_code":"PARTIAL_FAILURE","message":"Partial failure: 4 files could not be hashed","context":{"failed_count":4}}
```

exit code `4` — spec-error-contracts.md § 부분 실패 정합.

### Status breakdown

| status | count | 합계 |
|---|---:|---|
| `identical` | 0 | path 교집합 0 |
| `local_only_changed` | 1000 | 합성 vault `note-{i:05}.md` 전부 (remote 부재) |
| `remote_only_changed` | 4736 | git/git blob entries (4739 type=blob - 3 symlinks promoted to failed) |
| `drift` | 0 | path 교집합 0 → 자연 0 |
| `failed` | 4 | 3 symlink + 1 submodule (git/git remote-side originated) |
| **`files[]` total** | 5740 | 1000 local + 4740 (4736 + 4) processed remote |

### `failed[]` surface 검증

| path | failed_reason | mode | is_binary | size_bytes |
|---|---|---|---|---|
| `RelNotes` | `symlink` | `120000` | `false` | (omit) |
| `sha1collisiondetection` | `submodule` | `160000` | `false` | (omit) |
| `subprojects/git-gui` | `symlink` | `120000` | `false` | (omit) |
| `subprojects/gitk` | `symlink` | `120000` | `false` | (omit) |

spec-domain-pitfalls.md § detect-only 정책 § Submodule / Symlink 정합:
- `120000` symlink → `Status::Failed` + `failed_reason: "symlink"` +
  `mode: "120000"`.
- `160000` submodule (Trees `type:"commit"`) → `Status::Failed` +
  `failed_reason: "submodule"` + `mode: "160000"`.
- `is_binary: false` — cascade 외부 (local read 차단으로 measurement
  없음, spec-output-schema.md § `is_binary` 정책 정합).
- `size_bytes` field omit — mode≠`100644` cascade에서 `try_hash_local`
  pre-flight 진입 안 함 (Phase 7.2 task K size pre-flight 정책 정합).

### Trees raw 4964 → processed 4740 gap (224 entries)

raw Trees API recursive=1 응답 4964 entries vs processed 4740 (4736
remote_only + 4 failed) gap 224. 분해:

| raw type | count | scan 처리 |
|---|---:|---|
| `blob` (mode 100644 / 100755 / 120000) | 4739 | 4736 remote_only + 3 symlink failed |
| `tree` (sub-directory placeholder) | 224 | classify silent drop (file 카운트 외) |
| `commit` (submodule pointer) | 1 | 1 submodule failed |

224 silent drop = `type:"tree"` directory placeholder entries. Trees API
recursive=1 응답에 directory entry도 포함되지만 scan은 file (blob) 단위로
분류하므로 directory는 file count에서 제외 — `process_entries` (`shared/github/trees/fallback/recursive/iter.rs`)
가 `Outcome::Subtree`로 분기 후 fallback 진입 시점에만 의미 있고, 정상
recursive=1 path에서는 silent drop. 본 측정 결과는 spec § processed file
count 정의와 정합.

### Trees API truncation / sub-tree fallback

본 측정에서는 truncation 미발생 (raw recursive=1 응답 `truncated=false`,
사전 검증). 따라서 sub-tree fallback (`fetch_subtree_recursive`,
Phase 7.1) 동작은 본 dogfood에 surface 안 함 — 별도 integration test
(`tests/scan_trees_fallback.rs` task G + unit test
`fallback::recursive::walk::tests` task F) cover.

linux/torvalds (~5 K dirs) 같은 큰 repo는 sub-tree fallback 진입 시
budget 1000 cap 즉시 위반 → `GitlessError::TreesTruncated` (exit 5).
public repo로 fallback 동작 dogfood는 별도 budget 정책 진화 task로
deferred (현 baseline은 unit/integration test cover).

### Caveats

- Single host, single session, **single run** (cross-check sanity 목적,
  spec § "manual 1회 sanity" 정합). T main bench의 3 runs (0.758 /
  0.848 / 0.880 s, variance ~0.12 s) 패턴이 본 측정에도 적용된다고 가정.
- 합성 vault `tmp/synth-vault-42` 재사용 (T와 동일 1000 file, NFC ASCII
  / LF / mtime epoch). 합성 vault 정책상 함정 surface 0 — `failed[]`
  4건은 모두 git/git remote-side originated (symlink/submodule).
- git/git는 markdown 위주 vault use case와 다른 분포 (C 소스 + shell +
  Perl + Tcl + Makefile 위주). vault dogfood는 T main bench로 cover, 본
  § 은 cross-check sanity 목적 — repo 분포 mismatch는 의도된 설계 선택.
- sub-tree fallback dogfood 부재 — git/git는 truncation 영역 외,
  linux/torvalds는 budget cap 위반. real public repo로 fallback 검증은
  별도 task로 deferred (현 baseline은 unit/integration test cover).
- Internal instrumentation 부재 — sub-tree fallback 진입 여부 stderr
  emit 등이 없어, 본 측정은 외부 신호 (Trees recursive=1 raw 응답
  `truncated=false` 사전 검증)로 fallback 미진입을 단정. fallback 진입
  dogfood 시 internal trace flag 도입 검토.

## 종합 (task W, 2026-05-10)

> Phase 7.3 task W — T/U raw data + ADR 0008 § Phase 7.3 재검토 위에
> 종합 분석. record-only § (T/U)가 명시한 "분석은 task W" 계약 정합.
> 본 § 은 추가 측정 도입 안 함 — 기존 raw data 위에 cross-comparison +
> spec contract 검증 + open items 정리만.

### Coverage

| 측정 | path scale | remote scale | runs | 목적 | 결과 |
|---|---:|---:|---:|---|---|
| T (main bench) | 1000 local | 129 entries (KneShell/gitless-sync) | 3 (cold + 2 warm) | local 1000+ scale processing isolate | 0 failed, exit 0, schema v1.2 |
| U (cross-check) | 1000 local | 4964 entries (git/git@94f0577) | 1 (manual sanity) | medium-large remote sub-linear scale + remote-side failed surface | 4 failed, exit 4 PARTIAL_FAILURE, schema v1.2 |

T는 small remote로 isolate해 local hash + walker + classification 시간이
remote tree fetch에 가려지지 않게 함. U는 38× 큰 remote로 sub-linear
remote scale + remote-side 함정 (`mode 120000` symlink + `mode 160000`
submodule) surface 검증.

### Cross-comparison — T vs U walltime

| 측정 | mean walltime | remote tree raw entries | remote-side failed | 비고 |
|---|---:|---:|---:|---|
| T (cold + 2 warm, mean N=3) | 829 ms (variance ~120 ms) | 129 | 0 | KneShell/gitless-sync 자체 (Rust + md mix) |
| U (single run) | 1109 ms | 4964 | 4 | git/git (C + shell + Tcl + Makefile mix) |

**관찰** — remote raw entries 38× 증가 (129 → 4964)에도 walltime 증가는
+0.28 ~ +0.35 s (~+35%)에 그침. local-side 1000 file hash (CRLF detect +
SHA-1) 시간이 지배적 + remote-side는 단일 batch 응답 1회 + JSON parse —
sub-linear scale 정합. Phase 4 GraphQL batching (ADR 0006/0007) + rayon 8c
local hash 병렬 (ADR 0003)이 remote scale 흡수.

**제약** — U는 single run, T variance ~0.12 s 패턴이 본 측정에도 적용된다
가정. 정확한 variance 측정은 본 task 범위 외 (spec § "manual 1회 sanity"
정합).

### Schema v1.2 + exit code contract 검증

T + U 둘 다:
- `schema_version: "1.2"` 정확 emit (`output.rs::SCHEMA_VERSION` Phase 7.2
  task P bump 정합).
- envelope (`schema_version` / `scanned_at` / `repo` / `branch` /
  `local_root` / `summary` / `files`) 7 field 모두 surface — `output.rs`
  v1.0/v1.1 backward-compat lock test (Phase 7.2 task P/Q) 정합.

Exit code 정합:
- T 3 runs: exit 0 (`stderr` empty) — failed 0건 정합 (`spec-error-contracts.md` § 정상 종료).
- U single run: exit 4 + stderr 1줄 PARTIAL_FAILURE JSON (`error_code:"PARTIAL_FAILURE"` + `failed_count:4`) — `spec-error-contracts.md` § 부분 실패 정합.

### `failed[]` surface — 함정 처리 정책 정합

T (`failed: 0`) — 합성 vault 정책 (`MAX_FILE_BYTES = 100 KB` / NFC ASCII
filename / LF / submodule·symlink·LFS·long_path 모두 generate 안 함)
정합. Phase 7.2 임계 (>50MB memory_exceeded / >100MB file_too_large)와 약
500× ~ 1000× gap.

U (`failed: 4`) — git/git remote-side originated:

| path | failed_reason | mode | spec § |
|---|---|---|---|
| `RelNotes` | `symlink` | `120000` | spec-domain-pitfalls.md § Submodule / Symlink |
| `subprojects/git-gui` | `symlink` | `120000` | (동) |
| `subprojects/gitk` | `symlink` | `120000` | (동) |
| `sha1collisiondetection` | `submodule` | `160000` | (동) |

`is_binary: false` + `size_bytes` field omit — short-circuit cascade
(`commands/scan/pipeline/short_circuit.rs` Phase 7.2 task L) 외부에서 격하
+ `try_hash_local` pre-flight 미진입 정합 (`spec-output-schema.md` §
`is_binary` 정책 + § Phase 7.2 size_bytes omit 정책).

### Sub-tree fallback (Phase 7.1) — 본 측정 surface 0

T 합성 vault (1000 + 129 entries) + U git/git (1000 + 4964 entries) 둘 다
Trees API recursive=1 응답 `truncated=false` 영역 — sub-tree fallback
(`shared/github/trees/fallback/recursive/walk.rs` Phase 7.1 task D)
미진입.

Truncation threshold (~7 MB / ~100K entries)에 도달하는 real public repo
dogfood는 별도 task로 deferred:
- linux/torvalds (~5 K dirs) → fallback 진입 시 budget 1000 cap 즉시 위반
  → `GitlessError::TreesTruncated` (exit 5) 보장. budget 정책 진화 후
  재검토.
- 현 baseline은 unit test (`fallback::recursive::walk::tests` Phase 7.1
  task F, call budget 1001 + entries 500_001 cap trip 시나리오) +
  integration test (`tests/scan_trees_fallback.rs` task G, multi-layer
  truncated descent → 합산 ScanReport)로 cover.

Internal trace flag 부재로 fallback 진입 여부 외부 신호 (Trees raw 응답
`truncated` field 사전 검증)로만 단정 — 본 측정에 두 측정 모두 미진입.

### mtime cache 재도입 트리거 — keep-drop 유지

ADR 0008 § Phase 7.3 재검토 (2026-05-10) 결론 박제 — **keep-drop 유지**
(cache 재도입 안 함).

근거 요약 — P6c 50 path 1324.8 ms (cold N=3, hash phase ~50 ms = 3-4%)
vs T 1000 path 829 ms / U 1000 path 1109 ms. path scale 20× 증가에도
walltime 오히려 작거나 비슷 → hash 비중 path linear 폭증 신호 없음.
ADR 0008 § Decision 임계 "speedup ≥ 2x"는 hash phase instrumentation 부재로
정량 검증 불가 — 임계 미달 신호도 도달 신호도 없음 → yagni 일관 적용.

향후 재도입 trigger (ADR 0008 § Phase 7.3 재검토 정합):
- (a) hash phase 별도 instrumentation 도입 후 측정 결과 hash 비중 ≥ 30%
  surface,
- (b) 또는 cache 도입 시 measured speedup ≥ 2x 직접 surface.

자세한 측정 + 분석은 `docs/adr/0008-mtime-cache-keep-or-drop.md` § Phase
7.3 재검토 본문 참조 — 본 § 은 결론만 박제 + cross-link.

### Open items (deferred, Phase 7 scope 외)

| Gap | 사유 | 향후 trigger |
|---|---|---|
| U single run variance 미측정 | spec § "manual 1회 sanity" 정합 | dogfood 측정 정책 강화 시 (yagni 일관 deferred) |
| Hash phase instrumentation 부재 | ADR 0008 § Phase 7.3 재검토 keep-drop 결정 정합 (정량 verify 없이 yagni) | mtime cache 재도입 검토 트리거 (a)/(b) surface 시 |
| sub-tree fallback real public repo dogfood 부재 | git/git는 truncation 영역 외, linux/torvalds는 budget cap 위반 | budget 정책 진화 task 도입 시 |
| internal trace flag 부재 | Phase 7 비목표 (외부 신호로 단정 가능) | fallback 진입 dogfood 도입 시점에 검토 |

본 task W는 raw data + cross-comparison + 정책 정합 surface — open items
는 모두 yagni 일관 deferred. 새 정책 결정 0건, 새 spec 변경 0건. Phase
7.4 release tag 진행 가능 baseline.
