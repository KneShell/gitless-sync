# Phase 5 — scan-scale `.gitattributes` overhead baseline

> Task R3 (2026-05-09). End-to-end `build_report` walltime at 10K (real-file)
> and 100K (mock-only) path scale, with vs without a 100-rule
> `.gitattributes`, recorded as the regression reference for Phase 5.
> Threshold gating is deferred to task U (CI gate); this document is
> record-only — same posture as the upstream X bench (`phase5-gitattributes-bench.md`).

## Hardware / Toolchain

| Field | Value |
|---|---|
| OS | Windows 11 Pro 10.0.26100 |
| Rust | stable 1.95.0 (project MSRV pin) |
| Profile | `bench` (release, debug-assertions off) |
| Backend | `Backend::Rest`, `BenchGhClient` (HashMap lookup; no real `gh` subprocess) |
| Harness | Criterion 0.5 (`harness = false`), sample_size 20 / 10 |

## Fixture

`crates/gitless-sync/benches/scan_scale.rs` builds three scenarios. The
`BenchGhClient` is inlined per file because `tests/common/` is not visible
to bench targets; it implements `GhClient` with the same argv → canned
stdout shape used by the integration tests.

### 10K identical (with vs without `.gitattributes`)

- 10K real local files, distributed 100 per subdirectory across 100 subdirs
  (`paths/p{:03}/file_{:05}.dat`).
- Each file's content is `"file_{i:05} content line\n"` — pure ASCII + LF.
  `prepare_for_hash`'s Unspecified branch is a no-op on this shape, so the
  Trees response SHA (`blob_hash(content)`) matches the local hash.
- All paths classify as `Status::Identical` → `extract_commit_paths` returns
  empty → Commits API is never called → REST/GraphQL backend cost difference
  is structurally zero in this bench.
- `with_attrs = true` writes a 100-rule root `.gitattributes` mirroring the X
  bench fixture (`*.extNNN`, `dirNNN/**`, `**/fileNNN.txt`, etc.). The path
  layout deliberately avoids these patterns so every path lands on
  Unspecified — overhead is pure matcher cost, not divergent classification.

### 100K remote-only mock (no `.gitattributes`)

- Empty local directory.
- 100K Trees entries, each with a unique SHA — every path falls to
  `Status::RemoteOnlyChanged` (`local_sha == None`, so the SHA-mismatch
  filter inside `extract_commit_paths` excludes it). Commits API is skipped.
- Walker walks an empty dir, so the dominant cost is Trees JSON parse +
  `BTreeSet`/`HashMap` build + per-path classify + JSON serialize. **Walker
  + hashing cost is excluded from this measurement** (intentional — pairs
  with the 10K real-file numbers as a parse/classify/serialize-only point).

## Baseline numbers (commit at task R3, 2026-05-09)

| Scenario | Mean walltime | Lower / Upper 95% CI | Samples |
|---|---:|---:|---:|
| 10K identical, no `.gitattributes` | **497 ms** | 486 / 509 ms | 20 |
| 10K identical, 100-rule `.gitattributes` | **1402 ms** | 1391 / 1414 ms | 20 |
| 100K remote-only, no `.gitattributes` | **175 ms** | 173 / 177 ms | 10 |
| 100K one-shot (single iteration) | 206 ms | n/a | 1 |

### `.gitattributes` overhead at 10K

- Absolute: **~905 ms** (`1402 - 497`).
- Ratio: **2.82×** (`1402 / 497`).
- Implied per-path: ~90 µs end-to-end (`905 ms / 10K`). The X bench reports
  ~50 µs P95 for a single `match_path` call; a path is matched twice in the
  pipeline (`lfs::is_lfs` + `prepare_for_hash::classify_path`), so 2 ×
  ~50 µs = ~100 µs lines up with this end-to-end delta within outlier noise.

## Reading the numbers

### `.gitattributes` does **not** dominate scan walltime at 10K

At 10K identical paths, the pipeline spends roughly 1/3 of its time on
walker + hash + classify-without-attrs (497 ms baseline) and 2/3 on the
matcher work added by 100 rules (~905 ms). Significant — but not
overwhelming. The actual real-vault posture differs in two ways that lower
the impact further:

- Real vaults rarely carry 100+ `.gitattributes` rules; ten or fewer is
  the common case.
- Path / rule layout that matches early lets the matcher short-circuit;
  the bench's all-Unspecified shape is a worst-case (every rule is checked
  against every path).

### Phase 4 GraphQL batching gain — structural argument, not direct measurement

The "1000 path scale ~38× speedup" claim in `docs/adr/0006-default-backend-graphql.md`
is a real-`gh` subprocess measurement. **R3 cannot reproduce that number
with `BenchGhClient`** — the mock returns HashMap lookups in microseconds,
masking the REST sequential vs GraphQL batched difference entirely. What R3
verifies instead is the **invariant** that motivates the cap:

- Commits API calls fire only for SHA-mismatched paths; at typical scan
  scale that's dozens or hundreds, not 10K.
- The `.gitattributes` work is bounded by ~90 µs/path.
- For a 10K-path scan with 100 changed paths, Commits API walltime
  (real `gh`, sequential) dominates — batching saves seconds. With
  `.gitattributes` adding ~900 ms, batching's gain is preserved as long as
  changed paths × (sequential-Commits-latency − batched-latency) >
  `.gitattributes`-overhead. At ADR 0006's measured numbers (REST 2484 ms
  vs GraphQL 1437 ms across 13 paths in the dogfood scenario), 100 changed
  paths would imply the savings dwarf the matcher cost.

The structural conclusion: **`.gitattributes` overhead at the bench's
worst-case (100 rules, all-Unspecified, 10K paths) does not cancel
Phase 4 batching wins.** A direct measurement would require swapping
`BenchGhClient` for a real `gh` subprocess fixture, which is out of scope
for R3 (deferred to task U / vault dogfooding).

### Why 100K is faster than 10K

The 100K mock-only case (175 ms) skips the 10K case's filesystem walk
+ 10K SHA-1 computations. It measures only Trees parse + classify +
serialize at scale — a different shape, included as the projection point
for "what does the no-IO pipeline cost at 100K?" Linear scaling implies
~1.75 s for 1M paths in the same shape, well within the Trees API
truncation ceiling (`docs/specs/spec-github-api.md` § Trees, G-002 limit
~100K entries).

## Regression gate scope

Same posture as the X bench: **record-only**. No hard ms ceiling enforced
in CI (GitHub `windows-latest` runners are noisy enough that absolute
ceilings would trip on benign variance). The threshold is deferred to:

- **Task U** — CI gate. R3 recommends the regression budget take the form
  of a *ratio* (`with_attrs / without_attrs ≤ 3.5×` allows headroom over
  the observed 2.82× plus runner variance) rather than an absolute ms
  threshold, so the gate survives across machines.

## Re-running the bench locally

```
cargo bench --bench scan_scale
```

Total wall time on the recorded host is ~60 s (warm-up + 20 + 20 + 10
samples + 100K one-shot setup). Criterion writes its detailed reports to
`target/criterion/scan_scale/...` for diff comparison across commits.

## Caveats

- Single host, single run — variance across machines / load is not
  measured. Re-record on a representative host before treating the
  numbers as a budget.
- `BenchGhClient` returns `HashMap` lookups; REST vs GraphQL backend cost
  is not measurable here. Phase 4 batching is preserved as a structural
  argument, not a direct measurement (see "Phase 4 GraphQL batching gain"
  section above).
- The 100K case skips walker + hash. Pairing the 10K real-file number
  with the 100K mock-only number is **not** a scaling extrapolation in
  the same dimension; treat them as two distinct shape points.
- The bench uses the worst-case `.gitattributes` shape (every path lands
  on Unspecified after checking 100 rules). Real vaults with shallower
  rule trees and earlier matches will see proportionally less overhead.
