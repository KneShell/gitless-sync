# Roadmap (Backlog)

> 이 파일은 ralph가 자동 로드하지 않는다. 사람이 v0.1 완료 후 다음 phase 진입할 때 참조.

## Phase 2 — 편의 명령어 (COMPLETED, 2026-05-07)

> **2026-05-07 완료.** `docs/adr/0004-init-stdout-redirect.md` 참조.
>
> - `gitless-sync init` — repo/branch/ignore 인자에서 `gitless-sync.toml` 본문을 stdout TOML로 emit. 도구 파일 작성 0, 사용자 shell redirect로 영구 파일 생성 (ADR 0004 read-only 영구 정합).
> - `--repo` 미명시 시 `GitlessError::Config("repo not specified")`, exit 1. 외부 호출 0이라 추가 실패 모드 없음.
> - 자세한 정의: `docs/specs/spec-cli-interface.md` § init subcommand.
> - `status` 명령어는 만들지 않는다 (영구 결정). `scan --summary-only`로 대체.

## Phase 3 — Write 도구 분리 (CANCELLED, ADR 0001)

> **2026-04-30 폐기.** `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` 참조.
>
> push 작업은 Claude Code(또는 사람)가 `gh` 명령으로 직접 처리한다. 도구가 잘 하는 일(drift 정량 보고)과 LLM이 잘 하는 일(자연어 → push 명령)을 굳이 합치지 않는다. `gitless-sync`는 영구 read-only.
>
> 이전 안(historical, 참고용):
> - 별도 바이너리 `gitless-push` (workspace 안에 추가).
> - scan 결과(JSON)를 stdin 또는 파일로 받아 GitHub API로 실제 push 수행.
> - AI가 scan → 사용자 승인 → push로 명시적 단계 분리.

## Phase 4 — 성능 최적화 (COMPLETED, 2026-05-07)

> **2026-05-07 완료.** ADR 0005/0006/0007/0008 박힘 + 0009 obsolete cascade. 측정 raw data는 commit history + `docs/ralph/implementation-plan.md` git log + `CLAUDE.md` § Current State 박스.
>
> - GraphQL batching 도입 — `--backend graphql` default 전환 (ADR 0006), REST는 `--backend rest` explicit fallback. GraphQL backend는 rayon 미사용 (ADR 0005, alias batching 자체가 병렬). batch 200 confirmed (ADR 0007).
> - mtime cache 도입 → 측정 → 제거. P6c speedup ≈ 1.0x noise floor → ADR 0008 제거 결정 + ADR 0009 obsolete cascade.
> - P6b 13 path scale: REST 2484ms vs GraphQL cluster 1437ms = 1.73x speedup (typical). 1000 path scale 추정 ~38x.
> - 188 tests pass, tarpaulin 90.09%, P9 dogfooding cross-backend 정합성 통과.
> - 사람 개입 0건 (15 task ralph 자율).

### 향후 검토 (v0.3+)
- **1000+ path scale에서 mtime cache 재도입 검토** — 50 path scale에선 hash 비중 작아 cache 효과 noise floor 안. vault scale (수백~수천 files)에서 hash 비중 증가 시 speedup 가능성 (ADR 0008 § Future work).
- **Trees API sub-tree 재귀 fallback** (truncated repo 지원, G-002 해소).

## Phase 5 — 도메인 함정 정리

> "언젠가는 터질 폭탄"이므로 비목표가 아닌 명시적 후속 단계로 박는다.

- macOS HFS+/APFS의 NFD 정규화 vs GitHub의 NFC 보존 (한글·악센트 파일명 깨짐).
- 대소문자 충돌 (Windows에서 `README.md` vs `Readme.md`가 동일 path key).
- 비-UTF-8 텍스트 인코딩 (EUC-KR 등) — v0.1은 바이너리 취급으로 영구 drift 발생 (G-006).
- submodule (Trees mode `160000`) entry 처리.
- 심볼릭 링크 (Trees mode `120000`).
- 빈 파일 (`SHA-1("blob 0\0") = e69de29...`) 실파일 통합 검증 — v0.1에서 unit test로는 통과했으나 실파일 케이스 검증 필요.
- 실행 권한 (Trees mode `100755` vs `100644`).
- `.gitattributes` 파싱 → git 표준 blob SHA 정확 재현 (선택적, 큰 변경).

> **우선순위 결정 미정** (ex-ADR 0001 Open Question #3). Phase 4 완료 후 운영 데이터(어떤 함정이 실제 사용 중 자주 발생하는지)와 사용자 요청 빈도로 순서 정함.

## v0.1 시점 미결 (Open Questions)

> Phase 1 진행 중 답을 찾아 해소되면 이 섹션에서 제거 + guardrails나 spec으로 옮긴다.

- ~~**GitHub 토큰 최소 권한 범위.**~~ **OBSOLETE (ADR 0001, 2026-04-30).** gh subprocess 채택으로 인증 책임이 도구 외부(gh CLI)로 이동. PAT 권한 가이드는 `gh auth login` 한 줄로 충분하므로 도구가 별도 검증할 필요 없음. 1차 smoke test (2026-04-29 vault 356 파일, OAuth token via `gh auth token`, 284 identical / 55 local_only_changed / 17 remote_only_changed / 0 drift)로 도구 동작 자체는 입증됨.
- **큰 파일 임계치.** 예: 10MB 이상 파일의 해시 메모리 사용량. Phase 4 캐시와 연결.
- **CI 플랫폼.** GitHub Actions Windows 러너에서 tarpaulin LLVM 백엔드 안정성 1차 검증 필요.

## 정책 메모 (v0.1 시점 결정)

- v0.1 비목표는 `CLAUDE.md` Critical Rules 참조. 위 Phase 2~5는 **언젠가 할 것**, 비목표는 **v0.1에는 안 한다**의 차이.
- LFS 추적 파일은 명시적 비목표 (Phase 5에도 포함 안 함). LFS 지원이 필요하면 별도 도구.
- 인터랙티브 UI는 영구 비목표. read-only CLI 본성에 어긋남.
- GitHub 외 호스팅(GitLab, Bitbucket)은 영구 비목표. fork 환영.
