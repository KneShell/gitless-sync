# Roadmap (Backlog)

> 이 파일은 ralph가 자동 로드하지 않는다. 사람이 v0.1 완료 후 다음 phase 진입할 때 참조.

## Phase 2 — 편의 명령어
- `gitless-sync init` — `gitless-sync.toml` 설정 파일 생성 도우미.
  - 입력: `--repo`, `--branch` 등 인자 또는 prompt
  - 출력: 현재 디렉토리에 `gitless-sync.toml` 작성. 기존 파일 있으면 `--force` 없이 실패.
  - 실패 모드: 파일 권한, 기존 파일 충돌.
- `status` 명령어는 만들지 않는다. `scan --summary-only`로 대체.

## Phase 3 — Write 도구 분리
- 별도 바이너리 `gitless-push` (workspace 안에 추가).
- scan 결과(JSON)를 stdin 또는 파일로 받아 GitHub API로 실제 push 수행.
- gitless-sync는 read-only 불변 유지 (이 원칙은 v0.1부터 박힘).
- AI가 scan → 사용자 승인 → push로 명시적 단계 분리.
- workspace 안에서 `crates/gitless-sync/src/shared/`를 같이 사용 (또는 `crates/shared/`로 분리 검토).

## Phase 4 — 성능 최적화
- 로컬 SHA mtime 기반 캐시. 큰 vault에서 매번 전체 해시 계산 비용이 문제일 때만 도입.
- v0.1 성능 측정 결과를 보고 도입 여부 결정 (premature optimization 방지).
- Trees API sub-tree 재귀 fallback (truncated repo 지원, G-002 해소).

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

## v0.1 시점 미결 (Open Questions)

> Phase 1 진행 중 답을 찾아 해소되면 이 섹션에서 제거 + guardrails나 spec으로 옮긴다.

- **GitHub 토큰 최소 권한 범위.** Fine-grained PAT로 `Contents: Read`만으로 Trees + Commits API 모두 작동하는지 검증 필요. (검증 방법: 실제 PAT 발급 + 통합 테스트 1회.)
- **큰 파일 임계치.** 예: 10MB 이상 파일의 해시 메모리 사용량. Phase 4 캐시와 연결.
- **CI 플랫폼.** GitHub Actions Windows 러너에서 tarpaulin LLVM 백엔드 안정성 1차 검증 필요.

## 정책 메모 (v0.1 시점 결정)

- v0.1 비목표는 `CLAUDE.md` Critical Rules 참조. 위 Phase 2~5는 **언젠가 할 것**, 비목표는 **v0.1에는 안 한다**의 차이.
- LFS 추적 파일은 명시적 비목표 (Phase 5에도 포함 안 함). LFS 지원이 필요하면 별도 도구.
- 인터랙티브 UI는 영구 비목표. read-only CLI 본성에 어긋남.
- GitHub 외 호스팅(GitLab, Bitbucket)은 영구 비목표. fork 환영.
