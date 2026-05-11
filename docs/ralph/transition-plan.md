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

- [x] D: `docs/specs/spec-architecture.md` rewrite. § Vertical slice / § LOC 임계 / § 박제 expiration 헤더에서 '사용자 취향' / '박제' 단어 제거. verbose 간결화 같이 (154 → 130 line).
- [x] E: `CLAUDE.md` (project) rewrite. § Current State 통째 제거 + § 사용자 취향 결정 (박제) 통째 제거 + § 비목표 roadmap.md reference 제거 + § 검증된 함정 '페르소나가 단언해도' → '외부 자료에 적혀있을 수 있으나' 일반화.
- [x] F: docs/research/* (14건) + docs/roadmap.md `git rm` (commit `dd8922e`, 15 file 2907 line 삭제).
- [x] G: spec/ralph/ADR file에서 박제 trail / 페르소나 mention 정리. spec-{domain-pitfalls, classification, error-contracts}.md / ralph/{guardrails, project-ops, prompt-build}.md / adr/{0001, 0003, 0004, 0006, 0010}.md / CHANGELOG.md.
- [x] H: task D/E rewrite에 verbose 간결화 같이 처리. ADR 0010도 통째 rewrite (84 → 60 line).
- [x] I: `README.md` 검수 — public-facing 호환 통과. 변경 0건.
- [x] J: hard gate (fmt/clippy/test/xtask check-line-limits/check-cycles) PASS 확인 + Phase B commit. xtask `check-panic-limits` sub-command는 미존재 — panic 검출은 clippy lint이 cover.

### C. ralph 워크스페이스 정리 (Task #30 일부)

- [x] K: `docs/ralph/implementation-plan.md` skeleton화. § Status 비움 + § Completed Phases (Phase 5/6/7/8 entries) 통째 제거 + § Active Phase 통째 제거 + § Notes for Build Mode + § Constraints 보존 (박제 expiration 한 줄 제거). 다음 phase 진입 시 § Status에 진행 정보 + § Tasks 추가.
- [x] L: `docs/ralph/prompt-plan.md` `git rm` (vague 결정: plan 모드 폐지). plan 모드 history는 vault reference task Q에서 한 단락 보존 예정.

### D. Rust 템플릿 vault export (Task #29)

- [x] M: `D:\11.vault\001_PARA\03 Resources\Rust\` 폴더 생성.
- [x] N: `001-rust-template.md` 작성. H2 8개 (Vertical Slice / Hard Gate Set / xtask 패턴 / ADR Format / Cargo Workspace + MSRV / CI Runner / Test 분리 정책 / Cargo Command Cheat Sheet). snippet 0 + 외부 reference 0 (vault standalone).
- [x] O: skip (Source Project link 사용자 결정으로 제외 — vault standalone 정합).

### E. ralph reference vault 갱신 (Task #30 본진)

- [ ] P: `D:\11.vault\001_PARA\03 Resources\Automation\002-ralph-technique-reference.md` 기존 본문 read. 어느 부분이 outdated인지 list업.
- [ ] Q: 간소화 rewrite. 변경점: (a) plan 모드 폐지 (또는 'Optional — LLM과 직접 plan 작업 시 skip' 한 단락), (b) build 모드 중심 재구성, (c) 본 프로젝트 운영 경험 반영 — Opus 4.7 + xhigh 권장 / explicit `--model` + `--effort` 인자 (default fallback 의존 X) / completion signal (`<promise>COMPLETE</promise>`) / chain depth cap (3) / hard gate full pipeline 통합 / sub-claude clean-context audit 통한 외부 시각 보강.

### F. Public readiness + self-cleanup (Task #31)

- [x] R: 전체 grep audit + 보안 sweep PASS. 잔존 위험 hit 0건. cargo audit (150 crate, 0 vuln) + cargo deny check (clean) + osv-scanner (No issues found) + gitleaks (484 commits, no leaks).
- [x] S: `.gitignore` 검증 PASS. target/ + .env + secrets + tmp/ 정합.
- [~] T: `README.md` public-facing 호환 PASS. **`LICENSE` 파일 미존재** — 사용자에게 권장 (MIT / Apache-2.0 dual 일반적, 사용자 결정 필요).
- [x] T+: `.github/dependabot.yml` + `.github/workflows/codeql.yml` 신규 — public 전환 후 자동 활성. Dependabot (cargo + github-actions weekly) + CodeQL (rust, weekly + push/PR).
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
