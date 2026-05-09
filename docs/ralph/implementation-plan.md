# Implementation Plan

## Status
- Last updated: 2026-05-10 (Phase 6.1 종료, v0.2.0 release 직전)
- Total tasks: 60
- Completed: 60 / 60

## Notes for Build Mode
- 이 plan은 사람이 직접 작성한 초안. ralph plan 모드는 스킵.
- ralph build mode는 첫 미완료 task (`[ ]`)부터 처리. 의존 순서가 본 plan에 명시 안 됐으면 acceptance + spec 본문에 잠재 의존 명시 (e.g., "X task 결과 위에서 진행").
- 각 task의 acceptance criteria는 spec 파일과 정확히 매핑. spec 변경 없이 plan만 수정하지 말 것.
- task 시작 시 `[~]`로 변경 + commit, 완료 시 `[x]`로 변경 + 본 작업 commit (`prompt-build.md` 룰).
- Phase 6 hard gate 모두 deny active 유지 (clippy 60/15/5 + LOC 300 + cycle/cross-slice 0 + panic 검출). 위반 시 task `[!]` BLOCKED.
- tarpaulin 80% 게이트 유지 (project-ops.md). 신규 task의 acceptance에 unit test 포함.

> **Slim 정책 (2026-05-10)**: completed phase는 1~2 sentence 요약만 retain. 자세한 task별 결과는 git history (`git log --grep="<task ID>"`) + commit message 본문 + CHANGELOG.md user-facing summary로 cover. active/pending phase만 verbose retain. 의존 순서 graph 제거 — completed phase는 이미 종결, 신규 phase는 phase 본문 안에 의존 명시.

## Completed Phases (v0.2.x)

### Phase 5 — 도메인 함정 정리 (본진 38 task: A~Y, 2026-05-09)
8 핵심 함정 (NFD/NFC + case + encoding + submodule + symlink + 빈 파일 + 실행 권한 + .gitattributes) + 추가 함정 4건 (UTF-8/16 BOM / LFS pointer / Windows long path / .gitignore) detect 또는 정확 hash 재현. NFC normalize + case_collision 3 시나리오 + encoding_rs 4-encoding sniff (raw bytes hash, b-policy) + .gitattributes 화이트리스트 5 entry + Schema v1.0→1.1 (mode/failed_reason/lfs_pointer 필드). vault dogfood 117 files / 0 drift / 0 failed + v0.1 baseline regression diff REGRESSION 0건.

### Phase 5.13 — Plumbing follow-up + sibling cleanup (3 task: AA/CC/DD, 2026-05-09)
failed_reason 3건 (encoding/nfd_collision/gitattributes_unsupported) caller plumbing 완성 + shared/github/trees + commands/scan/pipeline module 폴더 분할 + 5 sibling test file 제거.

### Phase 5.13.1 — clean-context audit follow-up (9 task: EE~MM, 2026-05-09)
encoding Failed entry is_binary plumbing + cascade priority lock + LFS regression guard test + PreState multi-line 복원 + Unknown arm YAGNI 제거 + make_remote ownership move 복원 + try_hash_local early return + tmp/ 정리 + .gitignore 추가 + 외부 worktree 제거.

### Phase 5.14 — md 자료 audit (7 task: NN~TT, 2026-05-09~10)
CLAUDE.md "### 메모리 환경" section 제거 (privacy critical, vault path/admin username 노출 제거) + CLAUDE.md slim 142→45 LOC + CHANGELOG.md slim 159→76 LOC + ADR (0001~0009) audit + 측정 raw data를 docs/research/phase4-measurements.md로 이전 + research/specs/ralph privacy 일반화 (vault path → `<project root>` placeholder) + guardrails G-011/G-016 fold + G-017 stale ref flag.

### Phase 6.1 — v0.2.x cleanup (3 task: UU/VV/WW, 2026-05-10)
ADR 0010 (cognitive_complexity 15 + LOC 300 orthogonal proxy 둘 다 유지 결정 — 함수 단위 분기 복잡도 vs file 단위 인지부하) + CI 안정성 1차 검증 (8 successful Windows runs retro, G-007 LLVM 가설 반증) + CI runner Linux 전환 (`windows-latest` → `ubuntu-latest`, 비용/cold-start 우위, G-018 cross-platform cfg gate 신규 발견).

## Active Phase

(없음 — Phase 6.1 종료, v0.2.0 release tag 직전)

## Pending Phases (v0.3+)

### Phase 7 — vault scale + Trees sub-tree + 큰 파일 임계치 (통합)

3 후보 통합. **진입 전 vague + clean-context 외부 시각 보강 필수** (Phase 5.13.1/5.14 동일 패턴). task 정의는 진입 시점에 작성.

- **vault scale 1000+ path dogfood** — Phase 5 후속, mtime cache 재도입 트리거 검토 (ADR 0008 § Future work). 결과 ADR 0010+1 (cache 재도입 OR keep-drop confirmed) + research/phase7-vault-scale.md.
- **Trees API sub-tree 재귀 fallback** — G-002 해소 (truncated repo 7MB or 100K entry 한도). spec-github-api.md § truncation handling 신규 + ADR 신규.
- **큰 파일 임계치 (10MB+)** — 메모리 사용량 + Phase 4 cache 연결. 임계치 결정 (skip / streaming hash / chunked read) + ADR 신규 + spec-hash-and-normalize.md § 큰 파일 처리 신규.

3 후보 interconnected — vault scale 측정에서 sub-tree 재귀 트리거 + 큰 파일 surface 가능성 (1000+ path vault 자연 발생). 한 phase 묶음 + ralph 1회 launch + sub-claude clean-context 검증 + AUTO-FIX + push.

## Constraints (모든 phase 적용)

- **Read-only 영구** (ADR 0001) — 도구는 파일/원격 수정 안 함.
- **Vertical slice** (`commands/<name>/` + `shared/` 진짜 공통만) + cross-slice ref 0건 + slice 안 acyclic + slice-internal directional discipline (orchestrator → domain → IO).
- **Windows 1차** (실행 환경) — CI 환경은 Linux runner (Phase 6.1 WW).
- **MSRV 1.95.0** stable + `#![forbid(unsafe_code)]` + `panic = "abort"` (release).
- **박제 expiration** — Phase 진입마다 재검토 (CLAUDE.md § 박제 정책).
- **자율 진행 회피 영역** (사용자 vague 답변, Phase 5.13.1/5.14 패턴) — spec semantics 변경 / 비목표 침범 / architecture 큰 결정 / 50% 이상 재작성. 진입 전 vague + clean-context 외부 시각 보강 필수.
