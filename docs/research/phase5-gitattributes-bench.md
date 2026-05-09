# Phase 5 — `.gitattributes` parser performance baseline

> Task X (2026-05-09). Per-file `match_path` glob fnmatch P95 measurement on a
> 10K-path / 100-rule simulated vault. Recorded for regression reference; the
> hard regression gate lives in task R3 (large-vault scale + Phase 4 batching
> non-cancellation check).

## Hardware / Toolchain

| Field | Value |
|---|---|
| OS | Windows 11 Pro 10.0.26100 |
| Rust | stable 1.95.0 (project MSRV pin) |
| Profile | `bench` (release + debug-assertions off) |
| Backend | `ignore::gitignore::Gitignore` per-line matcher (single matcher per rule) |

Measurements taken in a single warm process via the Criterion 0.5 harness;
manual P95 dump uses `std::time::Instant` over 10K sequential calls.

## Fixture

- `crates/gitless-sync/benches/gitattributes_match.rs`
- 100 rules at the working-tree root `.gitattributes` (mix of `*.ext`,
  `dir/**`, `**/file.txt`, anchored `/path/*` patterns).
- 1 sub-level `sub/.gitattributes` (single rule) — exercises the multi-file
  shallowest-first walk.
- 10,000 round-robin paths spread across four shape buckets so most calls hit
  zero or one rule (long-tail distribution that mirrors a real vault).

## Baseline numbers (commit `7657f76`+, 2026-05-09)

### Manual `Instant` over 10K calls

| Statistic | nanoseconds | microseconds |
|---|---:|---:|
| P50 | 40,700 | 40.7 |
| Mean | 41,935 | 41.9 |
| P95 | 50,200 | 50.2 |
| P99 | 62,100 | 62.1 |
| Max | 535,400 | 535.4 |

`MAX` reflects an OS scheduler outlier; the P99-to-MAX gap is the long tail
typical of a single-pass micro-benchmark on a desktop with background load.

### Criterion (75K iterations in 3.01 s, sample size 50)

| Statistic | microseconds |
|---|---:|
| Lower 95% CI | 38.660 |
| Mean | 39.169 |
| Upper 95% CI | 39.821 |

Criterion's mean falls below the manual P50 because Criterion samples include
amortised cache warmth, while the manual loop measures cold-and-warm sequential
calls including the first few iterations.

## Reading the number

A 10K-file vault doing one `match_path` per file on a 100-rule fixture pays
roughly **0.5 s** of `.gitattributes` work end to end (`P95 × N = 50.2 µs ×
10,000 ≈ 502 ms`). On the rayon scan path (`MAX_COMMITS_CONCURRENCY = 8`) the
walltime budget is dwarfed by GitHub IO; `.gitattributes` does not dominate.

## Regression gate scope

This document is **record-only** (advisor guidance, 2026-05-09): no hard ms
threshold is enforced in CI yet. The threshold lives one task downstream:

- **R3** — large-vault scale (10K / 100K paths) regression check, including
  the Phase 4 GraphQL batching non-cancellation invariant. R3 will pin a
  ratio-based or absolute regression budget on top of the numbers above.

Re-running the bench locally:

```
cargo bench --bench gitattributes_match -- --warm-up-time 1 --measurement-time 3 --sample-size 50
```

`cargo bench` is intentionally not added to `.github/workflows/ci.yml`:
GitHub `windows-latest` runners are noisy enough that an absolute P95
ceiling would trip on benign variance. R3 will revisit once a stable
threshold strategy is chosen.

## Caveats

- Single host, single run — variance across machines / load is not measured.
  Re-record on a representative host before treating the numbers as a budget.
- The fixture is synthetic. Real vaults skew toward fewer rules and more
  paths; the per-call cost should be lower in practice (fewer matcher loops).
- `match_path` is benched, not `classify_path`. The latter adds a single
  linear reduce over the matched attributes (≪ 1 µs). Bench `classify_path`
  separately if/when it shows up in a profiler.
