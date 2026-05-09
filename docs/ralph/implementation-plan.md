# Implementation Plan

## Status
- Last updated: 2026-05-10 (Phase 6.1 종료 — UU/VV/WW 완료 + skeleton inline slim 440→246 LOC)
- Total tasks: 60
- Completed: 60 / 60

> **Slim 정책 (2026-05-10)**: completed task `결과 (날짜): ...` paragraph + acceptance/spec/검증/Files sub-bullet 제거. header만 retain. 자세한 task별 결과는 git history (`git log --grep="<task ID>"`) + commit message 본문 + CHANGELOG.md user-facing summary로 cover. active/pending task만 verbose retain.

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 의존성 순서로 처리.
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Phase 6 hard gate 모두 deny active 유지 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출). 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (project-ops.md). Phase 5 새 코드 cover 정책 — 신규 task의 acceptance에 unit test 포함.

## Tasks (Phase 5 — 도메인 함정 정리)

### Phase 5.1 — vault 운영 데이터 분석 + fact check (우선순위 입력)

- [x] **A. vault scan 재실행 + drift 근원 분석 + 검증 필요 fact check**

- [x] **B. 우선순위 결정 — vault 데이터 기반**

- [x] **A2. `.gitignore` 무시 정책 spec 명시**

### Phase 5.2 — path 정규화 함정 (NFD / case)

- [x] **C. NFD → NFC path 정규화**

- [x] **D. 대소문자 충돌 처리 정책 결정**

- [x] **D1. Windows NTFS case collision local-side detection**

### Phase 5.3 — encoding 변환 시도 (hash 입력 (b) 정책)

- [x] **E. 인코딩 라이브러리 조사 + 채택**

- [x] **F. 비-UTF-8 인코딩 변환 시도 구현 (hash 입력 (b))**

- [x] **F1. BOM 처리 정책 구현 (UTF-8 + UTF-16)**

### Phase 5.4 — submodule / symlink / LFS pointer / Windows long path

- [x] **G. submodule (`160000`) detect-only**

- [x] **H. symlink (`120000`) detect-only**

- [x] **G1. git LFS pointer detection (.gitattributes filter=lfs 신호 기반)**

- [x] **R1. Windows long path / 예약 파일명 detect-only**

### Phase 5.5 — 빈 파일 실파일 검증

- [x] **I. 빈 파일 실파일 fixture + integration test**

### Phase 5.6 — 실행 권한 detect

- [x] **J. mode bit (`100755` vs `100644`) detect-only**

### Phase 5.7 — `.gitattributes` 정확 hash 재현 (큰 변경)

- [x] **K1. `.gitattributes` 파서 구현 (working tree 한정)**

- [x] **K1.5. `.gitattributes` 지원 attribute 화이트리스트 (5 entry, advisor BLOCKING fix)**

- [x] **K2. conditional LF normalize + lifetime 계약 구현 (advisor flag 3 — 분기 helper 분리)**

- [x] **K3. binary attribute 정확 적용**

- [x] **K4. `.gitattributes` 우선순위 정합 검증**

- [x] **X. `.gitattributes` parser performance gate**

### Phase 5.8 — spec 갱신 cascade

- [x] **L. spec-hash-and-normalize.md `.gitattributes` 정합 검증**

- [x] **M. spec-classification.md path 정규화 정합**

- [x] **N. spec-error-contracts.md 함정별 reason 매핑**

- [x] **O. spec-output-schema.md mode bit + reason + LFS 필드 검증**

- [x] **L1. spec-config.md `.gitattributes` 위치 정책 검증**

### Phase 5.9 — 보강 fixture

- [x] **P. NFD raw bytes injection unit test fixture**

- [x] **P1. NFD NTFS 실파일 fixture (clean-context §5 fact check)**

- [x] **Q. 인코딩 변환 fixture**

- [x] **R. submodule/symlink/permission integration fixture**

- [x] **R2. error contract robustness fixture**

- [x] **R3. large vault scale fixture (Phase 5 perf 회귀 차단)**

- [x] **S. `.gitattributes` integration fixture**

- [x] **Y. encoding_rs binary size 사후 측정**

### Phase 5.10 — vault dogfooding + 회귀 검증

- [x] **T. vault dogfooding (Phase 5 후)**

- [x] **W. v0.1 baseline regression diff (정확화 vs 회귀 자동 분류)**

### Phase 5.11 — 최종 박제 + CI

- [x] **U. CI gate 갱신 (.github/workflows/ci.yml)**

- [x] **V. CLAUDE.md / roadmap.md 완료 박스**

- [x] **V1. CHANGELOG.md v0.2 작성**

### Phase 5.12 — Audit & cleanup (병렬 sub-agent, 모든 task 후 마지막 sweep)

- [x] **Z. 코드 스멜 audit + sibling test file 정리 (병렬 Explore sub-agent 6개)**

### Phase 5.13 — Plumbing follow-up + sibling cleanup (Phase 5 strict 완료)

> Z task audit + 사용자 지적으로 surface된 미완성 plumbing 3건 + 추가 sibling test 정리 2건.

- [x] **AA. `failed_reason` plumbing 3건 추가 (encoding / nfd_collision / gitattributes_unsupported)**
    - **Encoding**: `shared/decode.rs::try_decode_text` 결과가 `Failed` (UTF-8 + 2차 detect 모두 fail) → `Status::Failed` + `failed_reason: "encoding"`. `prepare_for_hash` 시그니처 변경 또는 caller가 `try_decode_text` 직접 호출 후 분기.
    - **NfdCollision**: `walker.rs` 또는 `pipeline.rs`에서 같은 NFC key를 가진 entry 2건 이상 detect (precomposeunicode false 환경 시뮬레이션) → 충돌 entry 모두 `Status::Failed` + `failed_reason: "nfd_collision"`.
    - **GitattributesUnsupported**: `prepare_for_hash`가 `AttributeMatch::Unsupported { attribute_name }` 받으면 새 반환 channel 또는 caller-side classify 호출로 `Status::Failed` + `failed_reason: "gitattributes_unsupported"`.
  - 추가 spec 갱신: `spec-classification.md` § Path 정규화 § edge case의 hedge marker 제거 (구현 완료 명시), `spec-error-contracts.md` § Per-file Pitfall Reasons 표의 enum-spec'd-but-unimplemented 3건 → 구현 완료 marker 변경, `spec-output-schema.md` § Acceptance § v1.1 신규 hedge marker 제거.
  - unit test: 3 reason 각각 detect → Status::Failed + failed_reason 매핑 round-trip. integration test: 각 시나리오 fixture (encoding fail UTF-16 BOM / NFC/NFD collision tempfile / .gitattributes Unsupported attribute) → JSON 출력 정합.
  - 검증: cargo fmt + clippy + check-line-limits + check-cycles + machete + test (385+ pass) + tarpaulin 80% 유지. Phase 6 hard gate 그대로.
  - spec: `docs/specs/spec-error-contracts.md` § Per-file Pitfall Reasons + `spec-output-schema.md` § v1.1 신규 + `spec-classification.md` § Path 정규화 + `spec-domain-pitfalls.md` § Encoding/NFD/.gitattributes 화이트리스트 + `spec-hash-and-normalize.md` § BOM 호출 지점.

- [x] **CC. shared/github/trees module 폴더 분할 + sibling test 제거**

- [x] **DD. commands/scan/pipeline module 폴더 분할 + 4 sibling test 제거**

### Phase 5.13.1 — clean-context audit follow-up

> 4 sub-claude (overall + AA + CC + DD) clean-context 검증으로 surface된 follow-up 7건. med 3건 (EE/FF/GG) + low 4건 (HH/II/JJ/KK).

- [x] **EE. encoding Failed entry의 `is_binary` schema 처리 명시 (med)**

- [x] **FF. encoding cascade 우선순위 spec 명시 (med)**

- [x] **GG. pipeline LFS negative case 회귀 가드 추가 (med)**

- [x] **HH. `hash_pass.rs` `PreState` multi-line 복원 (low)**

- [x] **II. `hash_local.rs` `TextDecodeResult::Unknown` arm YAGNI 제거 (low)**

- [x] **JJ. `classify.rs::make_remote` clone trade-off 결정 (low)**

- [x] **KK. `try_hash_local` encoding failure early return (low)**

- [x] **LL. tmp/ 정리 + .gitignore 추가 (cleanup)**

- [x] **MM. 외부 worktree 제거 (cleanup)**

### Phase 5.14 — md 자료 audit (verbose / security / privacy)

> 사용자 발견 (2026-05-09): CLAUDE.md "### 메모리 환경" section의 사용자 정보 노출 (vault path `C:\Users\admin\iCloudDrive\iCloud~md~obsidian` + admin username + 컨텍스트 종류 `프로필·재무·자기성찰`) — privacy critical. 추가로 CLAUDE.md verbose + ralph spec 중복 + CHANGELOG/ADR/research/specs verbose + 정보성 md 보안/privacy 점검. 검증은 clean-context skill 권장 (사용자 명시).

- [x] **NN. CLAUDE.md "### 메모리 환경" section 제거 (privacy critical)**

- [x] **OO. CLAUDE.md slim — Current State + 사용자 취향 결정 + Ralph Workflow + File Locations**

- [x] **PP. CHANGELOG.md slim — v0.2 entry verbose paragraph 정리**

- [x] **QQ. ADR (0001~0009) audit + slim**

- [x] **RR. docs/research/* audit (privacy + verbose)**

- [x] **SS. docs/specs/* audit (privacy + verbose)**

- [x] **TT. docs/ralph/* audit (verbose + 중복)**

### Phase 6.1 — v0.2.x cleanup (박제 expiration 재검토 + CI 안정성)

> Phase 5.14 종료 후 v0.2.0 release 직전 마무리. 사용자 우려 (clean-context §5-3 명시, roadmap § Open Questions) 처리.

- [x] **UU. Phase 6 박제 expiration 재검토 — cognitive_complexity vs LOC 300 proxy 중복**

- [x] **VV. CI 안정성 1차 검증 — GitHub Actions Windows runner tarpaulin LLVM**

- [~] **WW. CI runner Linux 전환 (Windows → Linux)**
  - acceptance: 사용자 명시 (2026-05-10) — CLAUDE.md "Windows 1차"는 실행 환경 한정 (사용자 도구 사용 환경), CI 환경과 분리 합리적. `.github/workflows/ci.yml` `runs-on: windows-latest` → `runs-on: ubuntu-latest` 전환. tarpaulin engine `--engine llvm` retain (cross-platform 정합, project-ops.md 명시 그대로 — Linux LLVM 백엔드도 정상 지원). 비용 (private 전환 시 Windows 2배) + cold-start (Windows ~3~5min vs Linux ~30s) 둘 다 Linux 우위.
  - 검증: 본 세션 직접 Edit + commit + push origin main → `gh run watch <run-id>` Linux CI run 1회 성공 확인 + tarpaulin coverage 80%+ 통과. 사례 surface (Linux-specific 함정) 시 G-018 신규 entry 작성. README badge skip (VV result `private repo 404 unauthenticated` 정합).
  - Files: `.github/workflows/ci.yml`.

## 의존 순서

```
A → B (vault 데이터 → 우선순위)
A → A2 (.gitignore 정책 — vault 분석 결과 정합)
B → {C, D, E, F1, G, H, I, J, K1}  (우선순위 후 함정별 처리 시작)
C → D1 (NTFS case local-side detection은 NFC 정규화 후)
E → F → Y (인코딩 라이브러리 결정 후 변환 + binary size 측정)
G → H → J → R (mode field 공유 + integration fixture)
G/H → G1 (LFS pointer는 blobs IO 영역 — github 모듈 갱신)
G/H/J → R1 (Windows long path는 walker 영역)
K1 → {K1.5, K2, K3, K4} (.gitattributes 파서 후 정책)
K2 → F (conditional normalize 후 인코딩 변환 hash 입력 정합)
{K1, K2, K3, K4, K1.5} → X (perf gate은 K 후 측정)
{C, D, D1} → M (path 정규화 → spec)
{G, H, G1, R1} → N (함정별 reason → spec)
{J, G1, G, H} → O (mode bit + reason + LFS → spec)
{K1, K2, K3, K4} → L (.gitattributes → spec)
{K1, L1} → spec-config 정합
{C, D, D1, F, F1, G, H, G1, R1, I, J, K1~K4, K1.5} → {P, P1, Q, R, R2, R3, S, Y} (함정 처리 후 fixture)
모든 함정 task + L/M/N/O/L1 완료 → T (vault dogfooding)
T → W (regression diff 자동 분류)
T/W → U (CI gate)
U → V → V1 (완료 박스 + CHANGELOG)
V1 → Z (모든 task 완료 후 audit + cleanup sweep)
Z → AA (audit 발견 + 사용자 지적 plumbing 3건)
Z → CC (sibling test trees 정리)
{AA, Z} → DD (pipeline module 분할은 AA의 enum 변경 수용 + Z sibling 정책 정합)
DD → {EE, FF, GG, HH, II, JJ, KK} (clean-context audit follow-up — DD 분할 결과 위에서 cleanup + spec 명시)
KK → {LL, MM} (cleanup — tmp/ 정리 + 외부 worktree 제거)
{LL, MM} → NN (Phase 5.13.1 종료 후 Phase 5.14 진입)
NN → OO (CLAUDE.md privacy 제거 후 slim)
OO → {PP, QQ, RR, SS, TT} (CLAUDE.md slim 후 다른 md 병렬 audit)
TT → {UU, VV} (Phase 5.14 종료 후 Phase 6.1 v0.2.x cleanup 진입)
{UU, VV} → WW (Phase 6.1 안에 묶음 — VV retrospective 후 사용자 명시 Linux 전환)
```

ralph build mode 진행 권장 순서:
1. A (vault 분석 + fact check)
2. B → A2 (우선순위 + .gitignore 정책)
3. C → D → D1 (path 정규화 + NTFS local-side)
4. E → F → Y → F1 (encoding 변환 + binary size + BOM)
5. G → H → J → G1 → R1 (mode bit + LFS + Windows long path)
6. I (빈 파일 실파일 fixture)
7. K1 → K1.5 → K2 → K3 → K4 → X (.gitattributes 5 sub-task + perf gate)
8. L → M → N → O → L1 (spec 갱신 cascade)
9. P → P1 → Q → R → R2 → R3 → S (보강 fixture)
10. T → W (vault dogfooding + regression diff)
11. U → V → V1 (CI + 완료 박스 + CHANGELOG)
12. Z (audit + cleanup, 모든 task 후 마지막 sweep — 병렬 sub-agent)
13. AA → CC → DD (Phase 5.13 plumbing follow-up + sibling cleanup)
14. EE → FF → GG → HH → II → JJ → KK (Phase 5.13.1 clean-context audit follow-up)
15. LL → MM (cleanup — tmp/ + 외부 worktree)
16. NN → OO → {PP, QQ, RR, SS, TT} (Phase 5.14 md 자료 audit — privacy + verbose)
17. UU → VV → WW (Phase 6.1 v0.2.x cleanup — 박제 expiration 재검토 + CI 안정성 + Linux runner 전환)
