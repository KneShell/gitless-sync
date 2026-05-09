# 외부 Rust 프로젝트 LOC 통계 (사후 sanity check)

> 작성: 2026-05-09 (task K). 본 프로젝트의 300 LOC 임계 결정(`docs/specs/spec-architecture.md` § LOC 임계, 사용자 취향 박제 2026-05-08)을 외부 통계와 비교하는 **post-hoc sanity check** artifact. 임계를 도출한 자료가 아니라 합리성 사후 점검용 — 결정은 인지부하 기반 구조적 선택, 본 통계는 그 선택이 성숙한 Rust 생태계 기준 어디 위치하는지만 측정.

## TL;DR

- 본 프로젝트(gitless-sync) post-split: 39 file, **0% > 300 LOC** (enforcement, task J deny 게이트).
- 비교 대상 4 프로젝트: **19% ~ 43% file이 300 LOC 초과**. ripgrep 43%, bat 25%, tokio 23%, cargo 19%.
- 결론: 우리 300 임계는 성숙한 Rust 프로젝트가 자연 도달하는 분포 대비 **엄격한 편**. 그러나 "비현실적으로 엄격"하지는 않음 — 모든 비교 프로젝트가 ≤ 200 LOC bin에 가장 많은 file을 두고 있어, 임계 자체는 자연 분포 안에 있음. 우리가 0%로 도달한 건 enforcement 결과지 분포 정상값이 아님.
- **임계 결정은 외부 stats가 아닌 인지부하 + 구조적 선택**. 본 문서는 "기괴하지 않다"의 사후 확인일 뿐.

## Methodology

### 측정 환경

- 측정일: 2026-05-09
- Clone 경로: `$env:TEMP\rust-loc-stats\` (Windows TEMP, 본 프로젝트 트리 외부 격리)
- Clone 옵션: `git clone --depth=1` (shallow, branch 최신 tip만)
- 측정 도구: PowerShell `[System.IO.File]::ReadAllLines($file).Length`
  - 본 도구 `cargo xtask check-line-limits`의 Rust `content.lines().count()`와 일치 (둘 다 trailing newline 미포함, `\n`/`\r\n` split)
  - tokei/scc 같은 외부 도구는 사용 안 함 — install 의존성 + `code`/`comments`/`blanks` 분리 통계가 본 프로젝트 xtask와 다른 셈법이라 비교 무의미
- File scope: `*.rs` 만 (Rust source). `Cargo.toml` / `*.md` / build script 등 제외
- 디렉토리 scope: 모든 sub-directory (`src/`, `tests/`, `examples/`, `benches/` 등). 본 프로젝트 임계가 tests 포함이라 비교도 동일 룰 적용
- 디렉토리 exclude: `target/`, `vendor/`, `.git/`

### Sample Selection

- **ripgrep** (BurntSushi/ripgrep): 성숙 CLI 도구 (grep 대체). v14+, 활발히 유지됨.
- **cargo** (rust-lang/cargo): Rust 빌드 시스템. 거대 코드베이스, integration test heavy.
- **tokio** (tokio-rs/tokio): async runtime. 모듈성 강조, sub-crate 다수.
- **bat** (sharkdp/bat): cat 대체 CLI. 본 프로젝트와 가장 가까운 size peer (수십 파일 규모 single-binary CLI).

> 4 프로젝트는 **maturity peer**이지 **size peer**가 아님. ripgrep/cargo/tokio는 본 프로젝트보다 5–30× 큼. bat이 가장 size 비슷하지만 그래도 더 큼 (67 vs 39 file). 분포 비교는 의미 있으나, 본 프로젝트가 이들 분포에 수렴해야 한다는 뜻은 아님 — 다른 도메인 (build system / runtime / domain CLI).

## Per-Project Stats

### Summary Table

| Project | files | mean | median | p75 | p90 | p95 | p99 | max | files > 300 | % > 300 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| **gitless-sync (post-split)** | 39 | 124.8 | 95 | 236 | 286 | 288 | n/a | 288 | 0 | **0.00%** |
| ripgrep | 100 | 522.7 | 178.5 | 608 | 1174 | 1686 | 7779 | 7779 | 43 | **43.00%** |
| cargo | 1356 | 246.1 | 30 | 180 | 676 | 1223 | 3314 | 8032 | 259 | **19.10%** |
| tokio | 777 | 224.0 | 101 | 273 | 550 | 850 | 1621 | 2699 | 177 | **22.78%** |
| bat | 67 | 254.3 | 82 | 315 | 669 | 823 | 4237 | 4237 | 17 | **25.37%** |

### Histogram (% per project)

| LOC bin | gitless-sync | ripgrep | cargo | tokio | bat |
|---|---:|---:|---:|---:|---:|
| 0–100 | 51.3% | 29.0% | 68.7% | 49.9% | 56.7% |
| 101–200 | 17.9% | 22.0% | 7.9% | 18.4% | 11.9% |
| 201–300 | **30.8%** | 6.0% | 4.3% | 8.9% | 6.0% |
| 301–500 | 0.0% | 13.0% | 5.6% | 11.5% | 11.9% |
| 501–1000 | 0.0% | 13.0% | 7.2% | 7.5% | 11.9% |
| 1001+ | 0.0% | 17.0% | 6.3% | 3.9% | 1.5% |

> gitless-sync의 30.8% 201–300 bin은 enforcement 효과 — task F-I 분할 시 300 임계 직전까지 끌어올려놓은 결과. 자연 분포 아님. ripgrep/cargo/tokio/bat는 모두 "0–100이 최대 bin" + "tail이 길다"는 공통 모양.

### Top-5 Largest Files (각 프로젝트)

| Project | LOC | Path | 카테고리 |
|---|---:|---|---|
| ripgrep | 7779 | `crates/core/flags/defs.rs` | 구조적 config table (flag 정의 macro) |
| ripgrep | 3987 | `crates/printer/src/standard.rs` | production rendering |
| ripgrep | 2494 | `crates/ignore/src/walk.rs` | production walker |
| ripgrep | 1719 | `tests/regression.rs` | regression test 통합 file |
| ripgrep | 1686 | `crates/globset/src/glob.rs` | production glob parser |
| cargo | 8032 | `tests/testsuite/package.rs` | integration test |
| cargo | 6814 | `tests/testsuite/build_script.rs` | integration test |
| cargo | 6543 | `tests/testsuite/build.rs` | integration test |
| cargo | 5655 | `tests/testsuite/test.rs` | integration test |
| cargo | 5053 | `tests/testsuite/metadata.rs` | integration test |
| tokio | 2699 | `tokio/src/net/windows/named_pipe.rs` | platform impl |
| tokio | 2358 | `tokio/src/net/udp.rs` | runtime io primitive |
| tokio | 1942 | `tokio/src/runtime/builder.rs` | builder API |
| tokio | 1936 | `tokio/src/sync/mpsc/bounded.rs` | channel impl |
| tokio | 1854 | `tokio/src/process/mod.rs` | process abstraction |
| bat | 4237 | `tests/integration_tests.rs` | integration test |
| bat | 991 | `src/printer.rs` | production rendering |
| bat | 953 | `src/vscreen.rs` | production VT escape |
| bat | 823 | `src/assets.rs` | production asset bundle |
| bat | 785 | `src/bin/bat/clap_app.rs` | CLI definition |
| gitless-sync | 288 | `commands/diff/compute.rs` | production domain |
| gitless-sync | 288 | `commands/scan/mod.rs` | production orchestrator |
| gitless-sync | 286 | `shared/github/commits.rs` | production IO |
| gitless-sync | 286 | `commands/scan/pipeline.rs` | production domain |
| gitless-sync | 270 | `commands/scan/graphql/batch.rs` | production IO |

> **관찰**: 외부 프로젝트들은 integration test file이 압도적으로 큼 (cargo top-5 모두 testsuite). production code도 1000–2700 LOC 빈번. 본 프로젝트는 모든 file이 270–288 사이 — enforcement 효과로 임계 직전에 모임.

## Interpretation

### 300 임계는 어디 위치하나

비교 프로젝트 4개 모두 300을 자연 임계로 쓰지 않음. 평균 약 25%의 file이 300 이상이고, 일부는 1000+ file이 6–17% 차지. 즉 **"300 LOC 가 가장 큰 file 임계"라는 조건은 성숙한 Rust 프로젝트에서 거의 관찰되지 않는다.**

그러나 분포 자체는 임계 안에 있음:

- 4개 프로젝트 모두 **0–100 bin이 최대** (29–69%)
- **median**: 30 (cargo) ~ 178 (ripgrep) — 모두 임계 아래
- **p75**: 180 (cargo) ~ 608 (ripgrep) — 자연 분포로 제2/3 사분위가 임계 근처에 분포

즉 본 임계 300은 "엄격한 편이지만 자연 분포 안에 있는 위치". ≤ 300 LOC 비중은 ripgrep 57% (29+22+6) ~ cargo 81% (68.7+7.9+4.3) 범위, 4개 프로젝트 median 약 75%. 즉 우리 임계는 외부 자연 분포의 다수 sub-set과 정렬됨 — 비-자연 위치가 아님. 우리는 enforcement로 100%를 임계 아래로 강제한 것.

### 왜 이런 차이가 나는가

| Project | 임계 정책 |
|---|---|
| gitless-sync | enforcement 300 (xtask deny) |
| ripgrep | 없음 (43% > 300) |
| cargo | 없음 (19% > 300, integration test가 끌어올림) |
| tokio | 없음 (23% > 300) |
| bat | 없음 (25% > 300) |

비교 프로젝트들은 LOC 임계를 enforce하지 않음. file 크기가 자연스럽게 결정됨 — 도메인 복잡도, integration test 통합도, 모듈 분리 비용에 따라. 본 프로젝트는 의식적 제약을 적용했기에 분포가 인위적 — 임계 직전(270–288)에 다섯 file이 몰려있음 (top-5).

### 결론

본 프로젝트의 300 임계 결정은:

1. **외부 stats로부터 도출한 게 아님** — 인지부하 + AI 코드 리뷰 슬라이딩 윈도우 + 구조적 분리 강제 위한 의식적 선택.
2. **외부 분포 대비 엄격한 편** — 4개 비교 프로젝트 모두 임계 미준수 file 19% 이상.
3. **그러나 비합리적이지 않음** — 분포 자체는 임계가 자연 분포 sub-set이고, 50% 이상 file은 자연 임계 아래에 위치.
4. **enforcement 비용**: file 분할로 partial fan-out 증가 (gitless-sync 사례: F-I 4-task 분할 + Q error/ + P tests 분리). 작은 프로젝트라 cost 낮음. 큰 프로젝트(cargo 1356 file)에 동일 정책 적용은 대량 분할 작업 필요.

post-hoc sanity check 통과 — 임계는 합리적 위치에 있음. 단, "성숙한 Rust 프로젝트가 모두 따르는 default"는 아님.

## Limitations

- **4 프로젝트 sample**: 통계적 대표성 없음. 다른 프로젝트(starship, uv, helix, alacritty 등) 추가 측정 시 분포 다를 수 있음.
- **Maturity ≠ size**: 비교는 maturity 기준. ripgrep/cargo/tokio는 size 5–30× 큼 — 단순 LOC 분포 비교로 직접 transferable한 결론 도출은 어려움.
- **카운트 방법**: blank line + comment 포함. tokei `code` (comment/blank 제외)로 측정 시 % > 300 모두 더 낮아질 것 — 우리 임계도 같은 카운트 방법이라 비교는 일관, 그러나 절대값 자체는 "코드 라인" 의미가 아님.
- **도메인 차이**: build system (cargo) / async runtime (tokio) / CLI tool (ripgrep, bat) — 함수 길이, 데이터 구조, error handling 패턴 모두 다름. 분포 패턴 자체가 도메인 영향 받음.
- **enforcement 효과 격리**: 외부 프로젝트들이 LOC 임계를 enforce 한다면 분포가 어떻게 변할지는 본 측정으로는 알 수 없음.

## 재현 명령

```powershell
$tempDir = Join-Path $env:TEMP "rust-loc-stats"
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

# Clone (shallow)
git clone --depth=1 https://github.com/BurntSushi/ripgrep.git (Join-Path $tempDir 'ripgrep')
git clone --depth=1 https://github.com/rust-lang/cargo.git    (Join-Path $tempDir 'cargo')
git clone --depth=1 https://github.com/tokio-rs/tokio.git     (Join-Path $tempDir 'tokio')
git clone --depth=1 https://github.com/sharkdp/bat.git        (Join-Path $tempDir 'bat')

# Measure (ripgrep 예)
$repoPath = Join-Path $tempDir 'ripgrep'
$exclude  = @('\target\', '\vendor\', '\.git\')
Get-ChildItem -Path $repoPath -Filter *.rs -Recurse -File `
  | Where-Object { $f = $_.FullName; -not ($exclude | ForEach-Object { $f -like "*$_*" } | Where-Object { $_ }) } `
  | ForEach-Object {
      [PSCustomObject]@{ Path = $_.FullName; LOC = [System.IO.File]::ReadAllLines($_.FullName).Length }
    } `
  | Sort-Object LOC -Descending
```

본 프로젝트 측정은 `cargo xtask check-line-limits` (Rust `content.lines().count()`)와 동일 셈법 — 직접 비교 가능.
