# Spec — Architecture Rules

> Phase 6 박제 (2026-05-08). 박제 expiration: Phase 진입마다 재검토 (CLAUDE.md § 박제 expiration 정책).

## Layer 정의

### Vertical slice (사용자 취향 박제)

명령어 단위 자체 모듈. `shared/`는 여러 명령어가 동일 로직 사용하는 진짜 공통.

```
crates/gitless-sync/src/
├── main.rs                # CLI 진입점, 명령어 디스패치
├── commands/
│   ├── scan/              # scan 슬라이스
│   ├── diff/              # diff 슬라이스
│   └── init/              # init 슬라이스
└── shared/                # 진짜 공통
```

### Cross-slice 직접 ref 금지

- `commands/scan/` ↔ `commands/diff/` ↔ `commands/init/` 간 import 금지.
- 공통 코드는 `shared/`로 이전.
- 컴파일러가 `pub(crate)` 가시성으로 강제.

### Slice 안 의존 그래프 acyclic 강제

- file A → B → A 같은 순환 의존 금지.
- 검증: `cargo xtask check-cycles` (xtask가 `cargo modules dependencies --lib --no-fns --no-types --no-traits --no-sysroot` DOT 출력을 파싱해 module-level uses 그래프에서 cycle 검출). cargo-modules `--acyclic`은 type-method edge를 cycle로 잘못 잡는 false positive가 있어 직접 사용하지 않음.

### Slice-internal directional discipline

각 slice 안 file은 다음 방향성 따름:

- **Orchestrator** (`mod.rs`): slice 진입점, domain/IO 호출.
- **Domain** (`compare.rs`, `output.rs`): 비즈니스 로직, 4분류 판정, JSON 직렬화. IO 모름.
- **IO** (`walker.rs`, `github.rs`, `graphql.rs`): 외부 부수효과 (filesystem, gh subprocess).

방향: `orchestrator → domain → IO`. domain은 IO를 import하지 않음.

강제 메커니즘: naming convention + `pub(crate)`/`pub(super)` 가시성. **manifest 박지 않음** (yagni — 18 files 프로젝트에 deviation 거의 없음, clean-context §4 격하).

### Module 폴더 단위 정책 (sub-module 분할)

Module 폴더 (예: `shared/gitattributes/`, `shared/error/`)는 **단일 책임 묶음** — vertical slice 박은 directional discipline 적용 X. 같은 도메인의 file 분할이라 mediator 강제는 oversize.

- **`mod.rs`**: re-export hub 또는 thin orchestrator. LOC 게이트 안 박힘 (300줄 자연 통과).
- **sub-module 간 sibling cross-ref 허용**: `use super::parser::Rule;` 같은 직접 참조. Rust 관용 정합.
- **acyclic 강제**: Phase 6 `cargo xtask check-cycles` 게이트가 sub-module 단위에도 자연 박힘 — cycle 0 보장.
- **directional discipline 적용 X**: orchestrator/domain/IO 분류는 vertical slice 단위 정책. module 폴더는 단일 책임이라 sub-module 간 같은 layer 박는 게 자연.

근거: `std`/`tokio`/`serde` 등 큰 crate도 sub-module 간 sibling cross-ref 자연 박음. mediator 강제하면 mod.rs 비대화 + Phase 6 LOC 300 게이트 위반 위험.

### Horizontal layer 영구 제외

CLI/Domain/IO 전체 분층은 채택 안 함. vertical slice 박제와 충돌 + 작은 CLI에 oversize.

## LOC 임계

### 임계값 — 300줄 (사용자 취향 박제)

- 모든 production 코드 file `.rs` ≤ 300 LOC.
- tests 포함 (same-file `#[cfg(test)] mod tests` 그대로 카운트).

### 면제 카테고리

- **doc comment heavy 모듈**: `///` API 문서가 LOC 자연 평장. 모듈 명시 면제.
- **mod.rs re-export only**: 자연 통과 (re-export만 박힌 mod.rs는 30~50줄 안). 별도 면제 정책 불필요.

### 구조적 분리 (면제 X)

- **error 정의 모듈**: 단일 거대 enum 대신 도메인별 sub-module로 분리 (`shared/error/network.rs`, `shared/error/config.rs` 등).
- **integration tests**: 단일 파일 대신 도메인별 file 분리 (`tests/scan_*.rs`, `tests/diff_*.rs`, `tests/common/mod.rs`). Rust ch11-03 best practice.

### 금지 패턴 — sibling test file

`production_module.rs` 옆에 `production_module_tests.rs` 같은 sibling test file **박지 마라**. Rust 관용 위반:

- unit test = same file `#[cfg(test)] mod tests` (private item 접근 + 컴파일 시점 분리)
- integration test = `tests/` 별도 crate (public API만 접근)
- sibling `_tests.rs`는 위 둘 다 어긴 패턴 — Rust ecosystem에 거의 안 박혀있음

LOC 300 게이트 통과 위해 test 분리 박지 말고, **production 자체를 module 폴더로 분할**:
- `foo.rs` (296 LOC) → `foo/{mod.rs, parser.rs, classify.rs, matching.rs}` 4 file
- 각 sub-module에 `#[cfg(test)] mod tests` 박음
- LOC 게이트 자연 통과 + Rust 관용 정합 + production 폴더 noise 0

위반 사례 (2026-05-09): `shared/gitattributes_tests.rs` + `shared/gitattributes_classify_tests.rs` (task K1.5 시점 박음). task Z에서 정리.

### Enforcement

- Phase 6 Step 2: F-I 4 task 분할 + Q error 분리 + P tests 분리 직후 baseline 위반 0건 도달 시 즉시 deny 전환.
- **enforcement 시점 deferred 금지** (clean-context §3-1 fix).

## Panic escape hatch 차단

### 단계적 도입 (warn → fix → deny)

| lint | 의미 | 안전한 대안 |
|---|---|---|
| `clippy::unwrap_used` | `.unwrap()` 호출 검출 | `?` + `anyhow::Context` |
| `clippy::expect_used` | `.expect("msg")` 호출 검출 | `Result` 변환 + 명시 error |
| `clippy::panic` | `panic!()` 매크로 호출 검출 | `unreachable!()` 또는 `Err(...)` |

### tests 면제

`#[cfg(test)] mod tests` 안 또는 `tests/*.rs` file에서 `#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]` 자연 면제. fixture 가독성 우선, panic = test failure가 정상 동작.

### Enforcement 단계

- **Phase 6 Step 1.5 (task R)**: workspace lint warn 박음. baseline 위반 카운트 측정 → `docs/research/phase6-baseline.md`.
- **task S**: 위반 1건씩 `?` + `anyhow::Context`로 대체.
- **task T**: baseline 위반 0건 도달 시 warn → deny 전환.

## 외부 도구

| 도구 | 목적 | 박힘 시점 |
|---|---|---|
| `cargo-modules` | 의존 그래프 + cycle 검출 | Phase 6 Step 3 |
| `cargo-public-api` | API 변경 추적 (분할 회귀 가드) | Phase 6 Step 4 |
| `cargo-machete` | unused dependency | Phase 6 Step 4 |
| `cargo-tarpaulin` | coverage ≥80% | v0.1 |
| `cargo-deny` | license/supply chain | v0.1 |
| `cargo-audit` | security | v0.1 |

`cargo-udeps` 제외 (machete와 중복 + nightly 필요, MSRV 1.95 stable과 충돌).

## Event 기반 layer 통신 — 영구 제외

채택 안 함. 도메인 사실 — CLI 1회 호출 → main.rs dispatch → 단일 명령어 실행 → 종료. cross-feature 런타임 통신 0. 사용자 의도(참조 방향성 보호)는 위 Layer 정의 (cross-slice 금지 + acyclic + directional discipline + 가시성)로 강제.

Phase 5+ (도메인 함정 진입) 또는 1000+ path scale에서 비동기 필요해질 시 재검토.

## 박제 expiration

모든 박제 항목 (검증·토론 대상 X 포함) Phase 진입마다 재검토. clean-context 외부 시각 §5-1 self-correcting 메커니즘. transitive constraint 누적 차단 — 박제가 다음 Phase 결정의 근거로 굳어 검증 임계를 올리는 패턴 막음.
