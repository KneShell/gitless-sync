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
