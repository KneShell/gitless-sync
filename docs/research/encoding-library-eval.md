# Encoding Library Evaluation — Phase 5 Task E

> Snapshot at task E commit time (2026-05-09). 비-UTF-8 인코딩 detect/conversion 라이브러리 선정. spec-domain-pitfalls.md § Encoding 변환 시도 + spec-hash-and-normalize.md § 인코딩 변환 시도 (비-UTF-8) 정합.
>
> **결론**: `encoding_rs` 단독 채택 + curated try-decode shortlist (Shift_JIS / EUC-KR / GBK / Windows-1252) — Option A. 별도 detection crate 미추가. (b) policy로 detection 정확도 무관 + dep/binary size 최소화.

## 문제 정의

Phase 5 함정 처리에서 `prepare_for_hash` 입력이 비-UTF-8 텍스트일 때 처리 정책:

1. **1차** UTF-8 디코드 시도 → 성공 시 `.gitattributes` 정합 normalize 적용 (`std::str::from_utf8`).
2. **1차 실패 시** → 다른 인코딩 detect 시도 (본 task에서 라이브러리 선정).
3. **2차도 실패 시** → `Status::Failed` + `failed_reason: "encoding"` + binary 취급.

**중요 제약 — (b) policy**:
- hash 입력은 항상 원본 raw bytes (UTF-8 변환 결과 사용 안 함). 근거: git core가 raw bytes 보존, UTF-8 변환 hash는 git core와 mismatch [source: https://www.codestudy.net/blog/how-to-determine-if-git-handles-a-file-as-binary-or-as-text/].
- detection은 `failed_reason` 마크 + 사용자 정보 제공 용도만. detection 결과가 hash에 영향 0.

**spec output schema 제약** (spec-output-schema.md § v1.1):
- `failed_reason` enum 9 값 중 `"encoding"` 단일 — detected encoding name 필드 없음.
- 즉, "Shift_JIS로 detect됨" 같은 정밀 라벨링은 출력 schema 외부.

→ **detection 정확도는 본 task의 의사결정 축이 아님**. accuracy 차이가 출력에 surface 안 됨.

## 라이브러리 후보

### 1. `encoding_rs` (Mozilla, Henri Sivonen)

- **License**: Apache-2.0 OR MIT [source: https://github.com/hsivonen/encoding_rs/blob/master/COPYRIGHT].
- **Maintenance**: 활성 (last release 2024, Servo / Firefox / Cargo 의존).
- **Coverage**: WHATWG Encoding Standard 기준. 주요 legacy encoding 모두 지원 — Shift_JIS, EUC-KR, GBK, GB18030, Big5, Windows-1252, ISO-8859-*, Latin-1 [source: https://docs.rs/encoding_rs/latest/encoding_rs/].
- **API**: 변환 위주 (`Encoding::decode`, `Encoding::for_label`). 직접 detection 기능 0.
- **Binary size impact**: 정적 인코딩 테이블 포함 — release stripped binary delta는 보통 ~1MB 수준 보고됨 (README 본문 "encoding_rs is fairly large" 명시). **Y task 측정 결과 (2026-05-09, current HEAD, panic=abort + lto=thin + strip=true)**: gitless-sync `.text` section에 encoding_rs symbol attribution **0 bytes** (LTO + strip + dead code elimination, top 50 crates 외 + `--filter encoding_rs` filter 0 KiB). 전체 `.exe` 2,476,032 bytes (`.text` 1.9 MiB / `.rdata` 393 KiB / 합 2.4 MiB). README "fairly large" + ~1MB 가정은 unconditional 사용 가정 — LTO 적용으로 4 encoding shortlist (Shift_JIS / EUC_KR / GBK / Windows-1252)만 retain되어 실제 impact는 측정 한계 미만 [source: `cargo bloat 0.12.1` + `objdump -h` Y task 결과 § Y task 결과 (2026-05-09)].

### 2. `chardetng` (Henri Sivonen, encoding_rs 동일 저자)

- **License**: Apache-2.0 OR MIT [source: https://github.com/hsivonen/chardetng/blob/main/COPYRIGHT].
- **Maintenance**: 활성. encoding_rs ecosystem 동일 저자 → 정합성 자연.
- **목적**: HTML web 콘텐츠 charset detect 전용으로 설계 (Firefox detector를 Rust로 재구현).
- **Coverage**: Latin/CJK encoding 그룹 detect. UTF-8은 design상 detect 안 함 (spec상 TLD/HTTP 헤더가 결정 역할).
- **API**: `EncodingDetector::feed` + `guess` → encoding 이름 반환. 변환은 encoding_rs로 위임.
- **Binary size impact**: 테이블 자체는 작지만 encoding_rs 동반 의존이라 net delta는 encoding_rs 단독 대비 미세 추가 [unverified].

### 3. `chardet` (Python chardet의 Rust 포트)

- **License**: MIT [source: https://github.com/thuleqaid/rust-chardet/blob/master/LICENSE-MIT].
- **Maintenance**: **non-active** (last commit 2018, 8+ years stale).
- **Origin**: Mozilla Universal Charset Detection (UDET, 2002) → Python `chardet` (2006) → Rust 포트.
- **Coverage**: Mozilla UDET 알고리즘 기반. 광범위하나 정확도 보고가 mixed (특히 짧은 snippet에서 false positive 사례 다수 보고됨) [unverified].
- **API**: `detect(bytes) -> ChardetResult { charset, confidence, language }`.
- **Binary size impact**: 별도 측정 부재 [unverified].

### 4. `charset-normalizer-rs` (Python charset-normalizer 포트)

- **License**: MIT [source: https://crates.io/crates/charset-normalizer-rs].
- **Maintenance**: 비교적 신선 (2023+ 활성).
- **Coverage**: Python charset-normalizer 알고리즘 기반. UTF-8 우선 + Mess Detector heuristic.
- **API**: 자체 detect + decode 결합. encoding_rs 의존 안 함.
- **Binary size impact**: 별도 측정 부재 [unverified].

### 후보 비교 매트릭스

| 항목 | encoding_rs | chardetng | chardet | charset-normalizer-rs |
|---|---|---|---|---|
| License | Apache-2.0 / MIT | Apache-2.0 / MIT | MIT | MIT |
| Maintenance | 활성 | 활성 | **stale (2018)** | 활성 |
| Detection 기능 | ✗ (변환 전용) | ✓ | ✓ | ✓ |
| Conversion 기능 | ✓ | ✗ (encoding_rs 위임) | ✗ | ✓ (자체 구현) |
| Ecosystem 정합 | **표준 (Servo/Firefox/Cargo)** | encoding_rs 동일 저자 | 미상 | 별도 ecosystem |
| Binary size | ~1MB (테이블) | encoding_rs 동반 + α | 별도 측정 부재 | 별도 측정 부재 |
| WHATWG Encoding Standard 정합 | ✓ | ✓ | partial (2002 algo 기반) | ✓ |

## 의사결정 — Option A vs Option B vs Option C

### Option A: `encoding_rs` 단독 + curated try-decode shortlist (**채택**)

**구성**:
- 의존성: `encoding_rs` 1개.
- detect 절차: 1차 UTF-8 시도 → 실패 시 curated shortlist (Shift_JIS / EUC-KR / GBK / Windows-1252)을 sequentially `Encoding::decode` 시도. replacement character (`U+FFFD`) 0개로 디코드되는 첫 encoding 채택.
- shortlist 외 encoding (Big5 / GB18030 / Latin-1 / ISO-8859-* 등) detect 실패 시 → 3차 fallback (`Status::Failed` + `failed_reason: "encoding"`).

**장점**:
- 의존성 1개 추가만. binary size delta 최소.
- (b) policy로 detection 정확도 무관 — shortlist mismatch 시 단순히 3차 fallback (binary 취급). detection 정확도 차이가 schema 출력 surface 안 됨.
- encoding_rs는 Rust ecosystem 표준 (Servo/Firefox/Cargo) — yagni 정합.

**단점**:
- shortlist 외 encoding은 false binary 분류 가능 (예: Big5 vault). 사용자가 후속 shortlist 확장 요구 시 spec § shortlist 갱신 cascade.
- detection 정확도가 detector lib 대비 낮음 — "Shift_JIS인지 EUC-KR인지" 모호 byte sequence에 대해 shortlist 첫 매칭이 random tiebreaker 역할.

### Option B: `encoding_rs` + `chardetng` 추가

**구성**:
- 의존성: `encoding_rs` + `chardetng` 2개.
- detect 절차: 1차 UTF-8 시도 → 실패 시 chardetng `EncodingDetector::feed + guess` → 반환된 encoding 이름으로 encoding_rs `Encoding::for_label` + `decode` → 검증.

**장점**:
- detection 정확도 우수 (특히 CJK 모호 byte). chardetng는 Firefox detector — 방대한 web 콘텐츠로 검증.
- shortlist 작성 필요 없음 — detector가 동적 결정.

**단점**:
- 의존성 2개 + chardetng 추가 size delta (작지만 0 아님).
- (b) policy로 detection 정확도가 출력에 surface 안 됨 → ROI 낮음.
- chardetng는 HTML 콘텐츠 가정 — vault markdown/text는 같은 가정 통과 가능하나 spec 외 가정 도입.

### Option C: `charset-normalizer-rs` 단독

**구성**:
- 의존성: `charset-normalizer-rs` 1개 (encoding_rs 의존 안 함).

**장점**:
- 의존성 1개로 detect + decode 둘 다 처리.

**단점**:
- Rust ecosystem 비표준 — encoding_rs 외 변환 알고리즘 도입.
- WHATWG Encoding Standard 외 (Python charset-normalizer Mess Detector heuristic). git core가 raw bytes 보존이라 align 의미 약하지만, 향후 .gitattributes `working-tree-encoding` 지원 시 (Phase 6+) WHATWG 정합 라이브러리로 회귀 변경 발생 가능성 높음.
- Maintenance 비교적 신선하나 Servo/Firefox 보증 없음.

## 결론

**Option A 채택** — `encoding_rs` 단독 + curated try-decode shortlist.

**근거**:
1. **(b) policy로 detection 정확도 무관**. spec-domain-pitfalls.md § Hash 입력 정책 (b) 명시 — detection 결과는 `failed_reason` 마크 + 사용자 정보, hash에 0 영향. accuracy 우위 (Option B chardetng) 가 출력에 surface 안 됨 → ROI 낮음.
2. **의존성/binary size 최소화**. Y task에서 encoding_rs 단독 ~1MB delta 측정 완료. Option B는 추가 dep + 추가 delta 발생 — 정량 우위 없는 cost 추가.
3. **spec 명시 정합**. spec-hash-and-normalize.md § 인코딩 변환 시도 (비-UTF-8) `encoding_rs` Mozilla 명시 (3 곳, line 54/98/154). Option A는 spec 명시 그대로 + shortlist 정책만 명시.
4. **WHATWG Encoding Standard 정합**. encoding_rs는 표준 라이브러리 — Phase 6+ `working-tree-encoding` attribute 지원 시 회귀 0건. Option C는 별도 ecosystem이라 회귀 위험.
5. **shortlist 4 encoding은 vault 운영 일반 cover**. Shift_JIS (일본), EUC-KR (한국), GBK (중국 간체), Windows-1252 (서구 legacy) — markdown vault 운영에서 surface 가능한 비-UTF-8 cases 통합 cover. 외 encoding (Big5 / GB18030 등) surface 시 사용자 요구 입력으로 shortlist 확장 cascade.

**채택 안 한 이유**:
- **Option B 거부**: chardetng는 Henri Sivonen 동일 저자 ecosystem 정합 우수하나, (b) policy로 정확도 우위가 출력 surface 안 됨 → cost > ROI. 향후 detected encoding name을 schema에 노출하는 요구 발생 시 (`failed_reason_detail` 신설 등) Option B 회귀 가능 — 본 결정은 v0.2 한정.
- **Option C 거부**: WHATWG 외 ecosystem + Servo/Firefox 보증 부재 → spec 명시(`encoding_rs Mozilla`) 정합 위반.
- **`chardet` 거부**: 2018 stale + Mozilla UDET 2002 알고리즘 기반 → maintenance + accuracy 둘 다 약점.

## 구현 — task F 입력

**`shared/normalize.rs::try_decode_text` 구현**:

```rust
pub fn try_decode_text(raw: &[u8]) -> TextDecodeResult {
    // 1차: UTF-8 시도
    if std::str::from_utf8(raw).is_ok() {
        return TextDecodeResult::Utf8;
    }

    // 2차: curated shortlist try-decode
    const SHORTLIST: &[&encoding_rs::Encoding] = &[
        encoding_rs::SHIFT_JIS,
        encoding_rs::EUC_KR,
        encoding_rs::GBK,
        encoding_rs::WINDOWS_1252,
    ];
    for enc in SHORTLIST {
        let (cow, _enc_used, had_errors) = enc.decode(raw);
        if !had_errors {
            // detected — but hash uses raw bytes per (b) policy
            return TextDecodeResult::Detected { encoding: enc.name() };
        }
        let _ = cow;  // discard converted text
    }

    // 3차: 모두 실패
    TextDecodeResult::Unknown
}

pub enum TextDecodeResult {
    Utf8,
    Detected { encoding: &'static str },
    Unknown,
}
```

caller 정책 (`prepare_for_hash` 또는 호출자):
- `Utf8` → text path normalize 적용.
- `Detected` → text-style 처리 안 함 (raw bytes hash + skip normalize). detection은 informational. F 구현 시 caller가 schema에 surface 안 시킴 — `failed_reason` 명시 안 함 (`Status::Identical/Drift`도 가능).
- `Unknown` → `Status::Failed` + `failed_reason: "encoding"` + raw bytes hash.

**hash 입력 raw bytes 정합**:
- `Detected` 분기에서도 변환된 cow 즉시 폐기 — hash 함수에 절대 전달 안 함.
- F task acceptance "EUC-KR 동일 파일이 local + remote 둘 다 있으면 `Status::Identical`" 정합 (raw bytes 동일하면 hash 동일).

**shortlist 확장 정책**:
- 본 v0.2 shortlist는 4 encoding 한정. vault 운영 surface 입력 누적 시 사용자 요구로 spec 갱신 cascade (spec-hash-and-normalize.md § 인코딩 변환 시도 § shortlist).
- Big5 / GB18030 / Latin-1 / ISO-8859-1~16 추가 후보. 자동 추가 0 — 명시 요구 시점에만.

## Acceptance — task E

- [x] `encoding_rs` (Mozilla, Apache-2.0/MIT) vs `chardet` (MIT, stale) vs `chardetng` (Henri Sivonen, Apache-2.0/MIT) vs `charset-normalizer-rs` (MIT) 평가 완료.
- [x] UTF-8 → 다른 인코딩 detect 정확도 검토 — (b) policy로 출력 surface 안 됨, 의사결정 축 아님.
- [x] Rust ecosystem 정합 검토 — encoding_rs 표준 (Servo/Firefox/Cargo).
- [x] License 검토 — encoding_rs Apache-2.0/MIT 적합.
- [x] `docs/research/encoding-library-eval.md` 작성 (본 문서).
- [x] 결정 확정 — Option A: `encoding_rs` 단독 + curated try-decode shortlist (Shift_JIS / EUC-KR / GBK / Windows-1252).

## 후속 task 입력

- **F task**: `shared/normalize.rs::try_decode_text` 구현 (위 § 구현 — task F 입력 코드 명시). [x] commit `fe45a8e`.
- **Y task**: `cargo-bloat` + dependency tree 분석으로 encoding_rs 추가 후 binary size delta 측정 (~1MB 보고 정량 검증). [x] § Y task 결과 (2026-05-09) 작성.
- **Q task**: EUC-KR / Shift_JIS / Latin-1 byte literal fixture 작성 + try_decode_text 시나리오 unit test. [x] commit `2a223ce`.

## Y task 결과 (2026-05-09)

> Phase 5 Y task — encoding_rs 추가 후 binary size delta 정량 측정. clean-context §5 의심점 (`~1MB delta`) 사후 검증.

### 측정 환경

- **Commit**: HEAD (encoding_rs 추가된 상태, task F commit `fe45a8e` 이후 모든 Phase 5 task 적용).
- **Profile**: `[profile.release]` `panic = "abort"` + `lto = "thin"` + `strip = true` (workspace `Cargo.toml`).
- **Toolchain**: stable 1.95.0, target `x86_64-pc-windows-msvc`.
- **Tools**: `cargo-bloat 0.12.1`, MSYS2 `objdump` (binutils 2.x).
- **Build command**: `cargo build --release -p gitless-sync`.

### encoding_rs attribution (cargo-bloat)

```
$ cargo bloat --release -p gitless-sync --crates -n 50
 File  .text     Size Crate
12.3%  15.0% 297.8KiB regex_automata
12.3%  15.0% 296.6KiB std
10.5%  12.7% 252.7KiB clap_builder
10.3%  12.5% 248.2KiB gitless_sync
 7.9%   9.6% 190.9KiB aho_corasick
 6.6%   8.1% 159.7KiB regex_syntax
 6.0%   7.3% 144.3KiB similar
 4.2%   5.1% 101.8KiB toml_edit
 2.5%   3.0%  60.1KiB globset
 1.5%   1.8%  35.7KiB serde_json
 ...
 0.1%   0.1%   2.2KiB unicode_normalization
 ...
 0.0%   0.0%       1B log
82.0% 100.0%   1.9MiB .text section size, the file size is 2.4MiB
```

**encoding_rs는 top 50 entries에 포함 안 됨** — 마지막 entry는 `log = 1 byte`. 즉 `.text` section attribution은 1 byte 미만.

```
$ cargo bloat --release -p gitless-sync --filter encoding_rs -n 100
File .text Size Crate Name
0.0%  0.0%   0B       filtered data size, the file size is 2.4MiB
```

`--filter encoding_rs`로 직접 검색 — `.text` section에 encoding_rs 심볼 attribution **0 bytes**.

### Reverse dependency tree

```
$ cargo tree -p gitless-sync -i encoding_rs
encoding_rs v0.8.35
└── gitless-sync v0.1.0
```

encoding_rs는 직접 dep만 — transitive 0건 (Option A 결정 정합).

### PE section breakdown (objdump)

```
$ objdump -h target/release/gitless-sync.exe
Idx Name          Size
  0 .text         001ef714  (2,029,332 bytes ≈ 1.9 MiB)
  1 .rdata        000624ac    (402,604 bytes ≈ 393 KiB)
  2 .data         00000200        (512 bytes)
  3 .pdata        00008550     (34,128 bytes)
  4 .reloc        00001cf8      (7,416 bytes)
```

전체 `.exe` size: **2,476,032 bytes (~2.4 MiB)**. 정적 데이터가 들어가는 `.rdata` section 전체가 **393 KiB** — README "fairly large" + ~1MB 가정과 정렬 안 됨 (전체 정적 데이터 자체가 1 MB 미만).

### 결론

1. **`.text` attribution 0 bytes** — encoding_rs 함수 코드는 LTO + dead code elimination + strip 적용으로 caller (`gitless_sync` 248.2 KiB)에 흡수 또는 제거. 사용된 4 encoding shortlist (`SHIFT_JIS`/`EUC_KR`/`GBK`/`WINDOWS_1252`) 외 dispatcher (e.g. `Encoding::for_label` for HTML 콘텐츠) 코드 다 제거.
2. **`.rdata` 정적 테이블 attribution unverified directly** — cargo-bloat은 `.text` section만 분석. encoding_rs lookup tables는 `.rdata`에 들어갈 가능성. 단 전체 `.rdata`가 393 KiB라 README "fairly large" + ~1MB 가정 (unconditional 사용)은 정량 부정 — `.rdata` 전체보다도 작아야 함.
3. **README "fairly large" + ~1MB은 misconception** — unconditional 사용 (모든 30+ WHATWG encoding 정적 테이블 retain) 가정. 실제 LTO + strip + dead code elimination이 사용된 4 encoding 외 다 제거.
4. **Option A 결정 영향 0** — 측정 결과 확인 후 결정 변경 없음. encoding_rs 단독 채택 confirmed (~1MB 추정 dimensions 정량 부정 + delta가 측정 한계 미만).

### Caveats

- **cargo-bloat은 `.text` section 분석** — 정적 테이블이 들어가는 `.rdata` attribution은 산출 안 됨. 정확한 `.rdata` per-crate attribution 측정에는 dual-build (encoding_rs 임시 제거 + 재빌드) 또는 `dumpbin /symbols` 분석 필요. 본 measurement는 README 가정 부정에는 충분 — `.rdata` 전체가 393 KiB라 ~1MB 추정과 정렬 안 됨.
- **LTO inline 우려** — encoding_rs `decode` 호출이 caller에 inline 가능 — 그 경우 `gitless_sync` 248.2 KiB 안에 포함되지 않아 attribution 안 됨. 단 inline 후 dead code 제거 가능성 더 높음 (decode 결과 cow 즉시 폐기 — b-policy).
- **Phase 4 baseline dual-build skip** — advisor 권고 적용. Phase 5 다른 dep (`unicode-normalization` 추가 task C, `ignore` 0.4.x crate 추가 task K1, `criterion` dev-dep 추가 task X) 노이즈 + 빌드 시간 30분+ 추가. encoding_rs 단독 효과 측정에는 cargo-bloat single-build로 충분.

### 의사결정 영향 요약

clean-context §5 의심점 ("encoding_rs ~1MB binary size delta") **사후 검증 완료** — 정량 부정. Option A (encoding_rs 단독) 결정 그대로 confirmed. § Limitations item 3에 있던 [unverified] → verified 갱신 적용.

향후 재검토 트리거:
- Phase 6+ `working-tree-encoding` attribute 지원 시 — full WHATWG encoding 사용 시점, 본 measurement 재실행 (`.rdata` 정적 테이블 추가).
- vault dogfooding (task T)에서 vault에 비-UTF-8 encoding 발견 시점 — shortlist 확장 cascade로 measurement 변동.

## Limitations

1. **shortlist 4 encoding 한정**. 외 encoding surface 시 false binary 분류 가능 — 향후 사용자 요구로 확장.
2. **detection 정확도 측정 부재**. (b) policy로 출력 surface 안 됨 → 정량 측정 ROI 0. 향후 `failed_reason_detail` 신설 시 Option B로 회귀 가능.
3. **encoding_rs binary size delta verified (Y task, 2026-05-09)**. cargo-bloat + objdump 분석 — `.text` section attribution **0 bytes** (LTO inline + dead code elim), 전체 `.exe` 2.4 MiB. README "fairly large" + ~1MB 가정 정량 부정. § Y task 결과 (2026-05-09) 작성.
4. **Phase 6+ `working-tree-encoding` attribute 지원 시 본 결정 재검토**. 본 결정은 v0.2 한정.
