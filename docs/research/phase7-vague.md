# Phase 7 vague — 사전 결정 셋

진입 시점: 2026-05-10
관련 문서: `docs/ralph/implementation-plan.md` § Phase 7, `docs/specs/spec-*.md` (Phase 7 신규)
검증: sub-claude clean-context 1회 (20 finding) → 사용자 결정 4건 + 자율 처리 9건 + 자율 fact check 1건

## Phase 7 목표

v0.3.0 release 직전 마지막 점검. 3 후보 한 phase 통합:

1. Trees API truncated repo (7MB or 100K entry 한도) sub-tree 재귀 fallback
2. 큰 파일 임계치 — `file_too_large` (100MB API 한도) + `memory_exceeded` (50MB tool 메모리)
3. vault scale 1000+ path dogfood — 합성 (.md 위주 1000+ 파일) + public repo cross-check

진행 순서: Trees → 큰 파일 → vault (독립부터, vault는 마지막 통합 dogfood).

## 사용자 결정 (vague + clean-context 결과)

### A. phase 구조 — 한 phase 통합 유지

- 사용자 dismiss: "나눈다고 비용 줄지 않음" — git bisect로 어느 tag/commit에서 회귀인지 추적 가능. tag 분할 무관.
- 순서 (Trees → 큰 파일 → vault) 유지 — 독립 spec/unit test 가능한 Trees/큰 파일 먼저, vault는 통합 dogfood라 마지막.

### B. 자율 chain hard cap (무한 loop 방지) — depth 3 + token 200k + wall-clock 6h 복합

- ralph + sub-claude 검증 chain은 max 3 depth (Phase 7 → 8 → 9 → 10).
- token 200k cap + wall-clock 6h cap 추가.
- 하나라도 초과 시 BLOCK + 다음 세션 wake-up 시 surface.
- 수렴 기준: "동일 finding 2회 연속 + 신규 0".

### C. 큰 파일 reason enum 분리

- `file_too_large`: 100MB API 한도 초과 (gh blob/raw download 거부 시점). `spec-hash-and-normalize.md` § 큰 파일 처리.
- `memory_exceeded`: 50MB tool 메모리 임계 초과 (Phase 4 cache 연결). `spec-hash-and-normalize.md` § 메모리 임계.
- schema v1.1 → v1.2 minor bump (failed_reason enum 9 → 11).

### D. spec semantics 변경 요구 finding 처리 — non-spec만 chain, spec은 queue

- sub-claude finding이 spec semantics 변경 요구 시 → changelog/research에 기록만 (chain 계속).
- 다음 세션 wake-up 시 사용자 surface (vague 대상 후보).
- ralph 자율 주행 + 도중 wake-up 0 stance 정합.

## 자율 처리 — sub-claude finding 9건 (spec 본문에 반영)

1. ralph 가동 직전 spec ↔ 코드 invariant 일치 점검 1회 (CI gate 추가).
2. Trees: 깊이 cap / 호출 budget / early-abort 셋 spec 명시.
3. sha 일관성: ref → commit sha → tree sha 1회 resolve, 모든 sub-tree 호출 동일 immutable tree sha 기반.
4. LFS pointer 분기: 100MB 미만 LFS pointer는 `file_too_large` 거짓 통과 — Phase 5 LFS detect-only spec 정합 활용.
5. 큰 파일 abort: Content-Length pre-flight + byte counter mid-stream abort.
6. 합성 vault: seed 고정 / UTF-8 NFC / LF / mtime explicit epoch / mtime 동률 차단.
7. NTFS case collision 사전 차단 (합성 generator).
8. noise (`.gitignore`/`.obsidian/`/symlink/BOM) explicit allow-deny.
9. public repo cross-check: HEAD floating 금지, commit sha 고정.

## 자율 fact check 1건 (WebFetch)

- gh CLI Contents API inline 1MB vs blob/raw 100MB 경로 → `spec-github-api.md` § blob fetch path 결정 반영.

## 자율 진행 회피 영역 (Phase 7 한정)

- spec semantics 변경: 본 vague에서 사전 본문 확정. ralph 가동 중 발생 시 → BLOCK.
- 비목표 침범 (read-only 깨기 / write 도구 추가): 영구 deny.
- architecture 큰 결정 (vertical slice 깨기 / Layer 정의 변경): 영구 deny.
- 50% 이상 재작성: 영구 deny.

## Excluded — sub-claude bisect 우려 (사용자 dismissed)

sub-claude는 "한 phase 통합 시 vault 단계에서 결함 터지면 bisect 비용 폭증" 우려 → mini-release 분할 권장. 사용자 dismiss: "tag 분할 무관, git bisect로 commit 수준 추적 가능". phase 통합 유지.
