# Phase 5 v0.1 vs v0.2 Regression Diff (W task)

> Snapshot at task W commit time (2026-05-09). T task가 박은 high-level metric (drift / failed 0건) 위에 per-file regression 자동 분류 박음. v0.1 baseline binary와 v0.2 after binary가 동일 local state + 동일 remote state 박은 채 binary delta만 isolate한 비교.

## Method

### Binary isolation 원칙

T가 hand-off한 입력은 "v0.1 출력 baseline JSON" + 정확화 vs 회귀 자동 분류. T 시점 v0.1 baseline JSON은 미보존 (`tmp/phase5-scan-baseline.{json,err}` race noise 정리 시점에 사라짐, T § Limitations). W는 commit `68fb0f0` (task A 박힌 v0.1 코드 시점) 박은 binary를 별도 worktree에서 빌드 후 재실행하는 방식으로 재생성.

핵심 변수 통제:
- **Local state**: 동일 (둘 다 main repo cwd 박음, current Phase 5 state).
- **Remote state**: 동일 (둘 다 KneShell/gitless-sync@main 박음, scan 간격 ~1.2s — remote drift ~0).
- **Binary**: v0.1 (worktree, 68fb0f0) vs v0.2 (main, ac836c9 직전 last release build).

→ 두 JSON의 entry-level delta는 binary 기인 (정확화 화이트리스트) 또는 타이밍 race (W6) 둘 중 하나.

### Worktree setup

```
git worktree add D:/00.Projects/02.Personal/gitless-sync-v01baseline 68fb0f0 --detach
cd D:/00.Projects/02.Personal/gitless-sync-v01baseline
cargo build --release --quiet
```

worktree는 main repo 외부 (`D:/00.Projects/02.Personal/`) 박음 — main repo 안에 두면 walker가 worktree 자체를 local file로 잡는 cascade 회피.

### Scan execution

두 binary back-to-back 실행, output을 `target/regression/` (builtin ignore 박힌 디렉토리, walker descent 안 함) 박음 — race noise 0:

```
$base_bin = "D:/00.Projects/02.Personal/gitless-sync-v01baseline/target/release/gitless-sync.exe"
& $base_bin scan --repo KneShell/gitless-sync --branch main --pretty > target/regression/baseline.json 2> target/regression/baseline.err
& ./target/release/gitless-sync.exe scan --repo KneShell/gitless-sync --branch main --pretty > target/regression/after.json 2> target/regression/after.err
```

scan 간격 1.2s (baseline 06:49:54.981, after 06:49:56.153). main repo의 local state 변동 0 (사람 개입 X + git 변경 X).

process artifact는 `tmp/` 박음 (gitignored 컨벤션 — T task 정합, 재실행 가능):
- `tmp/phase5-regression-baseline.json` (v0.1 binary 출력, 38 KB)
- `tmp/phase5-regression-after.json` (v0.2 binary 출력, 41 KB)
- `tmp/phase5-regression-diff.py` (분류 스크립트, 210 LOC)
- `tmp/phase5-regression-result.txt` (스크립트 stdout 캡처)

실제 결과 자체는 본 doc § Result + 아래 § Diff Output 박음 — self-contained.

### Diff classifier

`tmp/phase5-regression-diff.py` 박음. 입력 두 JSON을 path 키로 inner-join 후 각 entry를 다음 bucket으로 분류:

| bucket | 의미 |
|---|---|
| `exact-match` | path + status + `(local_sha, remote_sha)` 모두 동일 |
| `status-same-sha-drift` | status 동일, sha pair 다름 (timing 의심 — local 변동) |
| `new-in-after` | path가 after에만 박힘 (Phase 5 cascade 추가) |
| `whitelist-W3` | identical → failed 전환, reason ∈ pitfall enum |
| `whitelist-W4` | drift\|local_only_changed → identical (NFC 정규화 후보) |
| `whitelist-W5` | drift → failed, reason ∈ pitfall enum (`.gitattributes` binary) |
| `whitelist-W6` | `tmp/*` race noise (path-only 또는 status swap) |
| `REGRESSION` | 위 어디에도 안 박히는 status 변화 → 자동 fail |

pass 조건: `REGRESSION` == 0 + `status-same-sha-drift` == 0.

## Whitelist (정확화)

`docs/specs/spec-domain-pitfalls.md` § "v0.1 vs v0.2 회귀 정의" 박힌 화이트리스트 박음:

| ID | 변화 | 분류 근거 |
|---|---|---|
| **W1** | `schema_version` `"1.0"` → `"1.1"` | task O 시점 schema bump (mode/failed_reason/lfs_pointer 필드 추가). spec § "정확화 (의도된 변화)" 박음 |
| **W2** | `mode` / `failed_reason` / `lfs_pointer` optional 필드가 v0.2에만 박힘 | task O backward-compat lock test (output.rs::tests 5건) 박음 — v1.0 parser는 새 필드 ignore + status enum 5값 그대로 |
| **W3** | v0.1 `identical` → v0.2 `failed` (reason ∈ {submodule, symlink, lfs_pointer, encoding, long_path, nfd_collision, gitattributes_unsupported, case_collision}) | **v0.1이 함정을 detect 못 해서 우연히 Identical로 박힌** path를 v0.2가 정확 mark — 정확화 (LFS pointer가 v0.1에서 raw text로 박혀 양쪽 동일 hash 박힌 케이스 등) |
| **W4** | v0.1 `drift`\|`local_only_changed` → v0.2 `identical` (NFC 정규화 기인) | v0.1 NFD/NFC 다른 path key로 false drift, v0.2 NFC 정규화로 collapse — 정확화 |
| **W5** | v0.1 `drift` → v0.2 `failed` (reason ∈ pitfall enum) | **v0.1이 mismatch로 mis-classify한** path를 v0.2가 함정 reason으로 정확 mark — 정확화 (LFS pointer가 raw text로 박혀 mismatch 박힌 케이스, `.gitattributes` binary 분류로 LF normalize 안 박은 케이스 등) |
| **W6** | `tmp/*` path race noise (scan 자체 redirect 산출물, baseline과 after에서 비대칭) | scan 명령 자체의 부산물, 도메인 함정 아님 — baseline doc § race noise 정합 |

화이트리스트 외 status 변화는 회귀 (`REGRESSION` bucket).

## Result

| Bucket | Count |
|---|---:|
| `exact-match` | **121** |
| `status-same-sha-drift` | 0 |
| `new-in-after` | 0 |
| `whitelist-W3` | 0 |
| `whitelist-W4` | 0 |
| `whitelist-W5` | 0 |
| `whitelist-W6` | 0 |
| **`REGRESSION`** | **0** |

### Envelope (W1)

| 필드 | baseline | after | 분류 |
|---|---|---|---|
| `schema_version` | `"1.0"` | `"1.1"` | W1 (정확화) |
| `repo` | `KneShell/gitless-sync` | `KneShell/gitless-sync` | unchanged |
| `branch` | `main` | `main` | unchanged |
| `local_root` | `.` | `.` | unchanged |

### W2 — new optional fields

| field | after entries with field | baseline entries with field | 정합 |
|---|---:|---:|---|
| `mode` | 121 / 121 | 0 / 121 | ✓ — Trees API mode 필드 박힘 (v1.1 enrichment) |
| `failed_reason` | 0 / 121 | 0 / 121 | ✓ — KneShell/gitless-sync 함정 surface 0건이라 자연 0 |
| `lfs_pointer` | 0 / 121 | 0 / 121 | ✓ — same |

### Summary delta

| Status | baseline (v0.1) | after (v0.2) | Delta |
|---|---:|---:|---:|
| identical | 81 | 81 | 0 |
| local_only_changed | 40 | 40 | 0 |
| remote_only_changed | 0 | 0 | 0 |
| **drift** | **0** | **0** | **0** |
| **failed** | **0** | **0** | **0** |
| **Total** | **121** | **121** | **0** |

> **T baseline 117 vs W baseline 121 — 4 file delta**: T가 commit 박은 후 (W [~] 진입 + scan output 박음) 4 file 추가됨 — `docs/research/phase5-vault-after.md` (T docs) + `tmp/phase5-scan-after.{json,err}` 보존 (T race noise) + `tmp/phase5-scan-v01baseline.{json,err}` (W 첫 reckless 박은 race noise) + `tmp/phase5-regression-diff.py`. 본 W diff는 두 binary 동일 local state 박은 121-file pool 박음, T 시점 117 baseline은 무관.

## OVERALL: PASS

- REGRESSION 0건 — 스크립트 exit 0.
- 121/121 path가 binary delta 0 (status + local_sha + remote_sha 정확 일치).
- 정확화 화이트리스트 W1 + W2 적용 — schema_version bump + mode 필드 추가만 v0.2 박힘 변화, spec 정합.

## Diff Output

본 task run 시점 스크립트 stdout 박음 (self-contained 검증 박음):

```
======================================================================
W task — v0.1 baseline regression diff
======================================================================

baseline JSON: phase5-regression-baseline.json
after JSON   : phase5-regression-after.json
baseline scanned_at: 2026-05-09T06:49:54.981468800Z
after    scanned_at: 2026-05-09T06:49:56.153997Z

baseline summary: {'identical': 81, 'local_only_changed': 40, 'remote_only_changed': 0, 'drift': 0, 'failed': 0}
after    summary: {'identical': 81, 'local_only_changed': 40, 'remote_only_changed': 0, 'drift': 0, 'failed': 0}

Envelope deltas (whitelist W1):
  schema_version: '1.0' -> '1.1'

Bucket counts:
  exact-match: 121
  status-same-sha-drift: 0
  new-in-after: 0
  whitelist-W3: 0
  whitelist-W4: 0
  whitelist-W5: 0
  whitelist-W6: 0
  REGRESSION: 0

REGRESSION: 0 entries — PASS

Whitelist W2 — new optional fields in after:
  mode: present in 121/121 after entries
  lfs_pointer: present in 0/121 after entries
  failed_reason: present in 0/121 after entries
  (baseline entries with these fields: 0 — should be 0)

OVERALL: PASS
```

스크립트 exit 0 → CI에서 동일 framework 박혀있으면 자동 fail trigger 박지 않음.

## Limitations

1. **Dogfood target 함정 surface 0건**: KneShell/gitless-sync는 NFD path / `.gitattributes` / LFS / submodule / symlink / 비-UTF-8 / Windows long path 모두 결여. W3/W4/W5/W6 화이트리스트 분기는 0건만 박음 — 각 분기 trigger 검증은 W가 아닌 cross-reference integration tests chain 박음 (`tests/scan_modes.rs`, `tests/scan_gitattributes.rs`, `tests/scan_nfd_real_file.rs`, `tests/scan_robustness.rs`, `pipeline_tests_lfs.rs`, `pipeline_tests_long_path.rs`, `decode.rs::tests`). 본 task는 화이트리스트 trigger 검증이 아닌 **회귀 0건 자동 검출 framework** 박음이 scope.
2. **Vault 접근 불가**: 본 W run은 self-dogfood (KneShell/gitless-sync) 한정. Vault (`C:\Users\admin\iCloudDrive\iCloud~md~obsidian`) 박힌 환경에서 본 framework 그대로 재실행 시 W3~W5 화이트리스트 분기에 entry surface 가능 — Phase 5+ 별도 task 박음.
3. **첫 try 박힌 timing artifact**: 첫 v0.1 binary run은 main worktree cwd + tmp/ 박음 (advisor 권고 따른 외부 worktree와 별개) — T after.json (06:32:02) vs W baseline (06:47:44) 15분 간격이라 local 변동 박힘 (3 REGRESSION + 2 status-same-sha-drift surface). 본 doc은 timing-aligned 재실행 (06:49:54 vs 06:49:56) 결과 박음. timing artifact 흔적은 `tmp/phase5-scan-v01baseline.{json,err}` 박힌 채 commit 박음 (process 박음 reproducibility).
4. **v0.1 binary 정의 모호**: 본 task는 v0.1 = commit `68fb0f0` (task A 시점, gh subprocess 박힌 v0.2 마이그레이션 완료 후) 박음. ureq 시절 v0.1 (2026-04-29 vault 검증 시점) 박음 별도 binary build는 시도 안 함 — gh subprocess vs ureq backend는 ScanReport 동일 박음 (ADR 0002 마이그레이션 acceptance). v0.1 → v0.2 핵심 변화는 (a) gh subprocess 마이그레이션 (출력 동일) + (b) Phase 5 함정 처리 박음 — (b)만 본 W의 scope.

## Acceptance

- [x] v0.1 baseline JSON 박음 — `tmp/phase5-regression-baseline.json` (38 KB, schema 1.0, 121 entries)
- [x] v0.2 출력과 자동 diff 분류 — `tmp/phase5-regression-diff.py` Python 스크립트, exit 0 = pass
- [x] 정확화 화이트리스트 박음 — W1~W6 (envelope schema bump / new optional fields / identical→failed pitfall / drift→identical NFC / drift→failed `.gitattributes` / tmp/ race noise) 본 doc § Whitelist 박음
- [x] 화이트리스트 외 status 변화는 회귀 (자동 fail) — REGRESSION bucket 박힘, `OVERALL: FAIL` exit 1
- [x] `docs/research/phase5-regression.md` 박음 — 본 doc

## Hand-off

- **U task** (CI gate): 본 W diff framework는 vault-with-pitfalls 박힌 환경에서 재사용 가능. `tmp/phase5-regression-diff.py`를 CI에서 실행할 트리거 박음 가능 (Phase 5+ vault dogfood 실 환경 박힌 시점).
- **Vault 검증**: vault path 박힌 환경에서 동일 worktree + diff framework 박으면 W3~W5 화이트리스트 분기 trigger surface 박힘 가능. Phase 5+ 별도 task 박음.
