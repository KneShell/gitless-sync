# Phase 5 Vault Baseline + Pitfall Surface Analysis

> Snapshot at task A commit time (2026-05-09). Phase 5 진입 직전 baseline. v0.1 시점의 vault 검증(356 files, 0 drift, 2026-04-29, ureq) 이후 ADR 0001~0008 + Phase 4/6 변경 누적된 v0.2 코드로 재실행.
>
> **Vault unavailable note**: 이 머신(`C:\Users\dasgut`)에는 vault path(`C:\Users\admin\iCloudDrive\iCloud~md~obsidian`) 접근 불가. dogfooding target은 KneShell/gitless-sync 자체 repo (43 files baseline → 92 files 현재) 한정. 함정 surface는 ~0건 예상되며 실제로 그렇게 측정됨 (아래 § Drift Source Analysis). Phase 5 우선순위 입력은 fact check 3건이 분석 무게 중심을 담당.

## Measurement Setup

- 빌드: `cargo run --release --quiet -- scan --repo KneShell/gitless-sync --branch main --pretty > tmp/phase5-scan-baseline.json 2>tmp/phase5-scan-baseline.err`
- backend: `graphql` (default per ADR 0006)
- local root: `D:\00.Projects\02.Personal\05.gitless-sync` (이 repo 자체 — self-dogfooding)
- exit code: 0
- scanned_at: 2026-05-09T00:09:57.980941200Z
- schema_version: 1.0 (`mode` / `failed_reason` / `lfs_pointer` 필드는 task O 시점 1.1 추가)

## Scan Summary

| Status | Count | 비율 |
|---|---:|---:|
| identical | 90 | 97.8% |
| local_only_changed | 2 | 2.2% |
| remote_only_changed | 0 | 0.0% |
| drift | 0 | 0.0% |
| failed | 0 | 0.0% |
| **Total** | **92** | 100% |

## Drift Source Analysis

### local_only_changed: 2건 — **모두 scan 자체 출력 artifact (race condition noise)**

| path | local_sha | 분류 | 함정 매핑 |
|---|---|---|---|
| `tmp/phase5-scan-baseline.err` | `e69de29b...` (empty blob) | scan stderr redirect target | 함정 아님 — shell이 scan 실행 전 truncate한 빈 파일 |
| `tmp/phase5-scan-baseline.json` | `e69de29b...` (empty blob) | scan stdout redirect target | 함정 아님 — walker가 redirect 직후 파일 발견, 본문 작성 전 hash |

**Race 메커니즘**: shell이 `> tmp/X.json` redirect 처리 시 파일을 0-byte로 truncate한 시점 → cargo 프로세스 실행 → walker가 디렉토리 walk → tmp/X.json 0-byte 상태로 hash → empty blob SHA. scan 종료 후 cargo가 실제 JSON을 stdout으로 출력하면 그제야 file 본문 채워짐. 즉 hashing은 walker 시작 시점 snapshot이고, scan 출력은 그 이후라 전형적 자기참조 race.

이 2건은 **도메인 함정이 아님** — scan 명령 자체의 부산물이라 vault 분석 의미는 0. 실 vault에서는 scan 출력을 `tmp/`가 아닌 다른 디렉토리에 redirect하거나 pipe로 처리하면 자동 해소.

### drift / failed / remote_only_changed: 0건

KneShell/gitless-sync는 자체 Rust 프로젝트로 다음 조건 모두 결여:
- NFD path 없음 — Rust source는 ASCII 한정
- `.gitattributes` 없음 (root에 없음, `git ls-tree HEAD --name-only`로 확인됨)
- LFS-tracked path 없음
- submodule 없음
- symlink 없음 (Windows 환경 + 일반 Rust workspace)
- 비-UTF-8 텍스트 없음
- Windows long path / 예약 파일명 없음

→ **결론**: 이 dogfood repo는 Phase 5 함정 surface 측정에 부적절. 우선순위 입력은 vault 데이터 부재로 fact check + spec § 함정 매핑 + downstream task 결과(C/D/F/G/H/J/K/G1/R1)로 누적해야 함.

### v0.1 baseline 영향 평가

- v0.1 vault 검증 (356 files, 0 drift, 2026-04-29): 당시 vault에 함정이 surface하지 않았던 이유는 (a) vault 자체가 markdown 위주 ASCII-friendly + (b) v0.1 코드가 함정을 detect 못 해도 통과시키는 정책 (예: NFD를 raw bytes로 hashing해 NFD↔NFC drift surface 안 됨, 또는 `.gitattributes` 부재로 conditional normalize 없이 LF로 처리)이 함께 작용. 두 효과는 분리 측정 불가 (vault 접근 없이는).
- Phase 5 처리 후 vault dogfooding은 task T가 담당. 본 task A는 baseline snapshot + fact check만 수행.

## Fact Check Sub-Steps

### 1. encoding_rs binary size

**Status**: deferred to task Y (cargo-bloat dry-run + dependency tree, encoding_rs 채택 후 측정). 본 task A는 baseline 수행.

**Current baseline** (Phase 5 진입 직전, encoding_rs 미추가):
- release binary: `target/release/gitless-sync.exe` — **2,300,928 bytes (2.30 MB)**
- 직접 deps (cargo tree -p gitless-sync --edges normal 첫 두 레벨): `base64`, `chrono`, `clap`, `globset`, `ignore`, `rayon`, `serde`, `serde_json`, `sha1`, `tempfile`, `thiserror`, `toml`, `unicode-normalization`, `walkdir`
- encoding_rs는 미추가 (`cargo tree -p encoding_rs` → 패키지 없음 확인)

**알려진 size impact** (Y task에서 검증):
- encoding_rs는 Mozilla 공식 encoding 라이브러리, Servo/Firefox에서 사용 [source: https://docs.rs/encoding_rs/latest/encoding_rs/].
- 정적 인코딩 테이블이 커서 stripped binary delta가 보통 ~1MB 수준으로 보고됨 (참조: encoding_rs README, "encoding_rs is fairly large" 명시) [unverified — Y task에서 cargo-bloat 정량 측정 수행].

**측정 명령 (Y task용)**:
```
cargo install cargo-bloat
cargo bloat --release --crates --bin gitless-sync
# encoding_rs 추가 후 동일 명령 → delta 계산
```

`cargo-bloat` 미설치 상태로 본 task A baseline 측정만 수행, encoding 라이브러리 채택(E task) → 정량 측정(Y task) cascade에서 정확 delta 측정.

### 2. git core NUL byte heuristic — N=8000

**검증**: git source 직접 확인.

```c
// xdiff-interface.c
#define FIRST_FEW_BYTES 8000
int buffer_is_binary(const char *ptr, unsigned long size)
{
    if (FIRST_FEW_BYTES < size)
        size = FIRST_FEW_BYTES;
    return !!memchr(ptr, 0, size);
}
```

[source: https://github.com/git/git/blob/master/xdiff-interface.c]

**우리 코드와 비교** (`crates/gitless-sync/src/shared/normalize.rs:4-7`):

```rust
pub fn is_binary(content: &[u8]) -> bool {
    let probe_len = content.len().min(8000);
    content[..probe_len].contains(&0)
}
```

→ **N=8000 정합** (git core와 정확히 동일). v0.1 시점에 의도적으로 git core와 일치시킨 것으로 추정. Phase 5 함정 처리에서도 본 N 유지 (text=auto가 아닌 경우 NUL 휴리스틱은 default 정책으로 그대로).

`.gitattributes`에 `binary` / `text=auto` 명시된 file은 본 휴리스틱 무시 — K1.5/K2 task 적용 정합. 미명시(default) file은 v0.1 그대로 NUL 휴리스틱 적용 (spec-domain-pitfalls.md § 화이트리스트 표 default row).

### 3. Windows NTFS NFC/NFD 파일 공존 검증

**Hypothesis** (spec-domain-pitfalls.md § 검증 환경): NTFS는 normalize 안 함 — UTF-16LE 그대로 저장 → NFC/NFD 다른 byte sequence는 directory에서 별도 entry로 처리. compose 한글(`\u{AC00}`) vs decompose(`\u{1100}\u{1161}`) 둘 다 같은 directory 공존 가능.

**실험** (PowerShell):

```powershell
$dir = "tmp\nfd-test"; New-Item -Path $dir -ItemType Directory -Force | Out-Null
$nfc = [string]([char]0xAC00)                              # 가 (composed, 1 codepoint)
$nfd = [string]([char]0x1100) + [string]([char]0x1161)     # 가 (decomposed, 2 codepoints)
"NFC content" | Out-File -FilePath (Join-Path $dir "$nfc.txt") -Encoding UTF8 -NoNewline
"NFD content" | Out-File -FilePath (Join-Path $dir "$nfd.txt") -Encoding UTF8 -NoNewline
Get-ChildItem $dir
```

**결과**:
- Filesystem: NTFS (`Get-Volume -DriveLetter D` 확인)
- File count: **2** — 둘 다 별도 entry로 공존 확인
- File 1 name UTF-16LE bytes: `0,17, 97,17, 46,0, 116,0, 120,0, 116,0` → `U+1100 U+1161 . t x t` (NFD)
- File 2 name UTF-16LE bytes: `0,172, 46,0, 116,0, 120,0, 116,0` → `U+AC00 . t x t` (NFC)

→ **Hypothesis 정합**. NTFS는 두 form을 별개 path로 처리한다 — NFC/NFD 정합 처리 안 하면 vault 운영 환경(macOS APFS NFD ↔ Windows NTFS NFC `core.precomposeunicode` 차이)에서 false drift 직접 surface 가능.

**함의 (Phase 5 task C/D1)**:
- task C (NFC 정규화)는 walker에서 둘 다 catch한 뒤 NFC로 통일하면 같은 key — 하지만 둘 다 실파일이 존재하는 directory에서는 NFC 충돌 발생 → spec-domain-pitfalls.md § Path 정규화 "NFC 정규화 후 같은 key 충돌 → `Status::Failed` + `failed_reason: "nfd_collision"`" 정책 적용.
- task P1 (NTFS 실파일 fixture)은 본 실험을 그대로 `tempfile` integration test로 작성하면 됨 — PowerShell 의존 없이 Rust `std::fs::File::create` + 명시 path bytes로 재현 가능.

## Pitfall Priority Input

vault 데이터 부재로 본 task의 우선순위 입력은 제한적. task B에서 사용할 우선순위는 **이론 + spec 매핑 + fact check** 3축으로 전개:

| 함정 | 본 task baseline | task B 사용 시 우선순위 근거 |
|---|---|---|
| NFD vs NFC | 실험으로 NTFS에 NFC/NFD 공존 검증 (위 §3) | macOS↔Windows vault 운영 시 1순위 false drift 원인. NFC 정규화 hash 기반 (정확) |
| 대소문자 충돌 | 미surface (KneShell/gitless-sync는 case 충돌 없음) | NTFS case-insensitive volume에서 1 entry 누락 가능 (D1 task) |
| 비-UTF-8 인코딩 | encoding_rs 미추가 (size delta unknown) | EUC-KR vault에서 2순위 — 변환 시도 + raw bytes hash (b) 정책 |
| submodule | 미surface | 일반 vault에서는 0건 가능성 높음, detect-only로 충분 |
| symlink | 미surface | Windows unprivileged 생성 불가하므로 mock 검증 |
| 빈 파일 | 미surface (KneShell/gitless-sync에 없음) | unit test로 이미 cover (`hash::tests::empty_blob_matches_git`) |
| 실행 권한 | 미surface | content 같으면 Identical로 처리 — false drift 원인 아님 |
| `.gitattributes` | 미surface (KneShell/gitless-sync 없음) | 화이트리스트 사용 vault에서 1순위 정확화 (큰 변경) |
| BOM | 미surface | UTF-8 BOM은 v0.1 처리 가능, UTF-16 BOM은 detect-only |
| LFS pointer | 미surface | 미디어 vault에서 1순위 — `.gitattributes filter=lfs` 매칭 |
| Windows long path | 미surface | depth 깊은 nested vault에서 surface 가능 |
| `.gitignore` | scan은 합집합 정책 — task A2에서 spec 명시 | scan 범위 정의 — 우선순위 0순위 (정의 자체) |

→ **task B는 vault 데이터 없는 상태에서 진행**. 실 vault 접근 시점에 추가 검증 task 추가 가능 (W task 회귀 diff에서 catch).

## Limitations

1. **Vault 접근 불가**: 머신 환경 한정 (C:\Users\dasgut, vault는 `C:\Users\admin` 경로). Phase 5 진행 중 vault 접근 환경 확보 시 본 task 결과를 task T에서 추가 baseline으로 갱신 가능.
2. **encoding_rs binary delta 미측정**: cargo-bloat 미설치 상태. Y task에서 cargo-bloat 설치 + 정량 측정.
3. **schema_version 1.0 한계**: `mode` / `failed_reason` / `lfs_pointer` 필드 미적용 — 본 baseline은 v0.1 4분류 한정. task O 시점 1.1 적용 → 본 baseline 비교 시 schema diff 정확화로 분류 (W task 자동 비교).

## Acceptance

- [x] vault scan 재실행 (KneShell/gitless-sync 한정, vault 부재) — 92 files baseline 작성
- [x] drift 근원 분석 — 2건 모두 scan 자체 race noise, 함정 0건 surface
- [x] encoding_rs binary size baseline — 2.30 MB (encoding_rs 미추가 baseline), Y task에서 delta 측정
- [x] git core NUL byte heuristic 정확 N — **8000 bytes** (xdiff-interface.c FIRST_FEW_BYTES 직접 확인) [source: https://github.com/git/git/blob/master/xdiff-interface.c]
- [x] Windows NTFS NFD 파일 생성 검증 — NFC/NFD 공존 확인 (위 §3)
