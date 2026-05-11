# Transition Plan (Public 전환 + 외부 추출)

본 plan은 워크스페이스를 public 전환하기 위한 1회성 정리 트랙이다. 모든 task 종료 시점에 본 파일 자체를 삭제한다 (self-cleanup, task V).

## Status
- 진입 (2026-05-11)
- Tasks: 22 (A~V)
- Completed: 0 / 22

## Notes for Build Mode
- ralph build mode 진입 시 첫 미완료 task (`[ ]`)부터 처리. 의존 명시 없으면 알파벳 순서로 진행.
- 각 task 시작 시 `[~]` + commit, 완료 시 `[x]` + 본 작업 commit (`prompt-build.md` 룰).
- 코드 변경 동반 task는 hard gate full pipeline (fmt + clippy + xtask check-line-limits/check-cycles/check-panic-limits + test + tarpaulin 80%) PASS 후 commit.
- vault export task (M~Q)는 워크스페이스 외부 작업이므로 본 repo `git add` 대상 0건.

## Vague 결정 (2026-05-11)
- Deprecated spec 처리: **완전 삭제** (commit history가 backup 역할).
- Rust 템플릿 vault 구조: **단일 md hub** (`Rust/template.md`), 토픽별 H2 섹션 한 문단 + 권장값/snippet.
- Workspace ralph 잔존: **`docs/ralph/` 본문 유지**, `implementation-plan.md`만 skeleton화 (다음 phase 진입용 빈 틀).
- Public 전환 시점: **task V 종료 후 한 번에**, 사용자가 직접 `gh repo edit --visibility public` 실행.
- Self-cleanup: vault 이주 없이 본 파일 그대로 삭제.

## Tasks

### A. 기준 수립 (audit)

- [x] A: deprecated 판정 list 작성. 결과는 § Audit Result 참조.
- [x] B: 사용자 취향·박제 grep audit. 결과는 § Audit Result 참조.
- [x] C: 개인 path / private 식별자 grep audit. 결과는 § Audit Result 참조.

### B. 본진 정리 (Task #26 + #27 + #28 융합)

- [ ] D: `docs/specs/spec-architecture.md` § '사용자 취향 (박제)' 제거. 향후 contributor 가이드로 가치 있는 항목은 중립 톤 rewrite (vertical slice 정의 / module 폴더 정책 / panic 검출 룰 / sibling test 금지 등은 keep, 결정자 명시는 제거).
- [ ] E: `CLAUDE.md` (project) § '사용자 취향 결정' 제거 또는 중립 rewrite. § '검증된 함정' 같은 contributor 가치 항목은 keep.
- [ ] F: A 단계에서 `delete` 라벨 받은 spec/research/ralph 파일 `git rm`.
- [ ] G: A 단계에서 `strip` 라벨 받은 file에서 phase trail / 박제 trail 제거. phase trail은 `CHANGELOG.md`로 일원화.
- [ ] H: `keep` 라벨 받은 spec verbose 영역 간결화. 목표: 외부 contributor가 1독으로 핵심 요지 흡수 가능한 length.
- [ ] I: `README.md` public-facing 검수. 구성: 한 문장 소개 / 설치 (cargo install or release binary) / 빠른 사용 예시 / 아키텍처 1단락 / 링크 (CHANGELOG, ADR, LICENSE).
- [ ] J: hard gate full pipeline PASS 확인 + 본진 정리 commit (`docs: prep public — strip personal-stance, prune deprecated specs, simplify` 류).

### C. ralph 워크스페이스 정리 (Task #30 일부)

- [ ] K: `docs/ralph/implementation-plan.md` skeleton화. 보존 영역: Status (진입/Tasks/Completed line만 비움) + Notes for Build Mode + Constraints. Phase entry (Phase 5/6/7/8 Completed Phases) 전체 제거. 다음 phase 진입 시 비어있는 Phase entry에 새 task 추가.
- [ ] L: `docs/ralph/prompt-plan.md` 처리 결정. 옵션: (a) 유지 + heading에 'Optional — 사용자가 LLM과 직접 plan 작업 시 skip' 한 줄 추가, (b) `git rm`. plan 모드 폐지가 vague 결정이므로 (b) 우선. 다만 vault reference (task Q)에 plan 모드 history는 한 단락 보존.

### D. Rust 템플릿 vault export (Task #29)

- [ ] M: `D:\11.vault\001_PARA\03 Resources\Rust\` 폴더 생성.
- [ ] N: `Rust/template.md` 신규 작성. H2 섹션 6개:
    1. Vertical Slice (`commands/<name>/` + `shared/`, cross-slice ref 0건, slice-internal directional discipline)
    2. Hard Gate Set (clippy 60/15/5 + LOC 300 + cycle 0 + panic 검출 + tarpaulin 80%)
    3. xtask 패턴 (check-line-limits / check-cycles / check-panic-limits / check-readme-examples)
    4. ADR 형식 (1번 file 시작, status / context / decision / consequences)
    5. Cargo Workspace + MSRV (`rust-toolchain.toml` pin + `Cargo.lock` commit + `panic = "abort"`)
    6. CI Runner 선택 (Linux runner 이유 + actions/checkout v6 + Node.js deprecation 회피)
  각 섹션: 한 문단 설명 + 권장 설정값 + 본 repo file 경로 reference (예: `crates/gitless-sync/src/commands/scan/`).
- [ ] O: `Rust/template.md` 끝에 'Source Project' 한 줄 — 본 프로젝트 (gitless-sync) URL + 라이선스 명시. 외부 사용자가 실제 file 직접 참조하도록 가이드.

### E. ralph reference vault 갱신 (Task #30 본진)

- [ ] P: `D:\11.vault\001_PARA\03 Resources\Automation\002-ralph-technique-reference.md` 기존 본문 read. 어느 부분이 outdated인지 list업.
- [ ] Q: 간소화 rewrite. 변경점: (a) plan 모드 폐지 (또는 'Optional — LLM과 직접 plan 작업 시 skip' 한 단락), (b) build 모드 중심 재구성, (c) 본 프로젝트 운영 경험 반영 — Opus 4.7 + xhigh 권장 / explicit `--model` + `--effort` 인자 (default fallback 의존 X) / completion signal (`<promise>COMPLETE</promise>`) / chain depth cap (3) / hard gate full pipeline 통합 / sub-claude clean-context audit 통한 외부 시각 보강.

### F. Public readiness + self-cleanup (Task #31)

- [ ] R: 전체 grep audit 재시행. task B + C에서 잡은 hit 패턴 다시 grep — 잔존 0건 확인.
- [ ] S: `.gitignore` 검증. vault path / 개인 path 류 우연 누출 차단.
- [ ] T: `README.md` 최종 검수 + `LICENSE` 파일 존재 확인. LICENSE 없으면 사용자에게 권장 (MIT / Apache-2.0 dual 일반적).
- [ ] U: 본 plan 파일 (`docs/ralph/transition-plan.md`) `git rm` + commit (`chore: drop transition-plan post-cleanup`).
- [ ] V: 사용자에게 `gh repo edit KneShell/gitless-sync --visibility public` 명령어 surface. 실행은 사용자가 직접 (destructive + 외부 영향 surface, 자율 회피).

## Constraints (Transition Track 적용)
- **Read-only 영구** (ADR 0001) — 도구 본성 변경 금지. 정리 작업이 코드 동작 변경을 동반하면 안 됨.
- **Hard gate** — 코드/spec 변경 commit 직전 fmt + clippy + xtask + test + tarpaulin 80% PASS.
- **Vault export 격리** — task M~Q 결과물은 워크스페이스 외부. 본 repo `git add` 대상 0건.
- **Self-cleanup** — task U가 마지막 step. 본 파일 삭제 후 commit, 그 다음 task V.
- **자율 회피 영역** — public visibility 변경 (`gh repo edit --visibility public`)은 사용자 직접 실행. 외부 영향 + 되돌릴 때 비용 큼.

## Audit Result (Phase A 결과, 2026-05-11)

### A. 라벨링 (file pattern 단위)

| Pattern | Label | Reason |
|---|---|---|
| `docs/specs/*.md` (9건) | keep + strip | contributor 가이드 가치. phase 갱신 blockquote + 박제 marker 정리. |
| `docs/research/*.md` (14건) | **delete** | phase별 측정 artifact. 결과는 ADR + CHANGELOG에 반영됨. private path 핫스팟. |
| `docs/adr/*.md` (14건) | keep + strip | 결정 trail로 가치. 박제 단어 일반화 + tribunal/페르소나 mention 중립화. |
| `docs/ralph/prompt-plan.md` | **delete** (task L) | plan 모드 폐지 (vague 결정). |
| `docs/ralph/{prompt-build,project-ops,guardrails}.md` | keep + strip | ralph 운영 가이드. 페르소나/박제 trail 정리. |
| `docs/ralph/implementation-plan.md` | skeleton (task K) | Phase entry 비우고 빈 틀 유지. |
| `docs/ralph/transition-plan.md` | self-cleanup (task U) | 본 파일. |
| `docs/roadmap.md` | **delete** | phase 진행 trail이 본문 대부분. CHANGELOG와 중복 + 박제/stance hit 다수. |
| `CLAUDE.md` (project) | keep + strip | § '사용자 취향 결정 (박제)' 제거. § '검증된 함정' 유지. |
| `CHANGELOG.md` | keep + strip | line 151 박제 trail 한 줄 일반화. release entry는 그대로. |
| `README.md` | keep | public-facing 검수 통과. task I에서 최종 확인만. |

### B. 사용자 취향·박제 hit 핫스팟 (50+ 건)

- `CLAUDE.md:44-45` — § 사용자 취향 결정 (박제) — task E.
- `docs/specs/spec-architecture.md:7,61,152-154` — § Vertical slice (사용자 취향 박제) + § LOC 임계 + § 박제 expiration — task D.
- `docs/roadmap.md` 다수 — delete로 한 번에 해소 (task F).
- `docs/ralph/implementation-plan.md:39` — skeleton화로 자연 해소 (task K).
- `docs/specs/spec-{domain-pitfalls, classification, error-contracts}.md` — 박제 trail 1~2건씩 — task G.
- `docs/adr/0001,0003,0004,0006,0010` — 박제/페르소나 trail — task G.
- `docs/ralph/{guardrails, project-ops}.md` — 박제/페르소나 mention — task G.

### C. 개인 path / private 식별자 hit 핫스팟 (15+ 건)

- `docs/research/llm-as-caller-usability-eval.md:22,35,110,304,308` — `C:\Users\admin\iCloudDrive\...` + 다른 PC/LLM trail — delete로 일괄 해소 (task F).
- `docs/research/phase8-regression.md:207-208` — dasgut/admin user vault — delete로 해소 (task F).
- `CHANGELOG.md:105` — 이미 일반화됨 (Phase 5.14 audit entry). 추가 정리 불필요.
- 도메인 컨텍스트 (iCloud 언급) **keep**: `CLAUDE.md:4`, `README.md:3`, `docs/specs/spec-classification.md:54`, `docs/adr/0009:38`, `docs/ralph/guardrails.md:18` — 도구 동기 설명에 필수.
