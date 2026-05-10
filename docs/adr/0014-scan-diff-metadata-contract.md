# ADR 0014: scan-diff metadata contract — `diff_meaningful` + `presence` field 추가 (4-state status 유지)

- **Status**: Accepted
- **Date**: 2026-05-10
- **Related**: `docs/research/llm-as-caller-usability-eval.md` § F1 + § F2 + § Open Decisions, `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` § read-only 영구 (caller-decides 본성), `docs/specs/spec-output-schema.md` § v1.2 → v1.3 변경 (신규 대상), `docs/specs/spec-classification.md` § 상태 정의 (변경 0 contract)

## Context

LLM-as-caller usability eval (2026-05-10, vault dogfood) 7 friction 중 P0 2건 (F1 + F2)이 동일한 본질 — "caller가 spec 안 읽고 결과 JSON만 보고 다음 액션 결정 가능한가" 부족 — 으로 묶인다. 둘 다 scan output schema field 추가로 해소 가능한 additive change.

### F1 — scan과 diff 비교 기준 불일치

- 동일 path가 scan에서 `remote_only_changed` (sha differ), 같은 path `diff` 호출 시 stdout/stderr 0 bytes + exit 0.
- 근원: 자체정의 SHA-1 (BOM/encoding 차이 포착) vs LF+BOM normalize 후 비교 — `spec-hash-and-normalize.md`에 박혀있지만 LLM caller는 spec 안 읽음.
- 도구 신뢰 자체 흔들림 — "한쪽 명령 different, 다른 명령 identical" 답을 0-shot으로 해석 불가.

### F2 — `local_only_changed` status 의미 모호

- 같은 status가 (i) "local만 존재, remote 미존재" + (ii) "양쪽 존재 + local만 변경" 둘 다 cover.
- caller가 결과만 보고 다음 액션 ("remote에 새 파일 push 후보" vs "양쪽 conflict 검토") 결정 불가 — `spec-classification.md` 안 읽으면 모름.

### Open Decisions 사전 확정

eval § Open Decisions 표는 (a)/(b) 후보를 제시. 사용자 결정 (Phase 8 plan 본문 명시):

| 결정 | 채택 | 근거 |
|---|---|---|
| F1 해소 | (a) `diff_meaningful: Option<bool>` field | 호출 1회로 정보 다 받음. (b) diff stderr hint는 호출 2회 필요. |
| F2 해소 | (a) `presence` field 추가 | 4분류 status 그대로 유지 + backward compat. (b) status 4→6 split는 breaking + 호출자 분기 늘어남. |
| 묶음 형태 | F1 + F2 → 본 ADR | 둘 다 scan-output schema additive change + 동일 본성 (caller-decides surface) — 한 결정 trail. F3 (`diff --json`)는 별도 surface (CLI flag + sub-schema)라 본 ADR scope 외. |

## Decision

scan output entry (`files[]`)에 2 field 추가. 4-state status (`spec-classification.md` § 상태 정의)는 그대로 유지 — presence가 case 구분, status는 push/pull/conflict 액션 분류 그대로.

### `diff_meaningful: Option<bool>` (F1)

caller에게 "이 entry가 diff 호출했을 때 의미 있는 결과 나오는지" hint. 4-case lock:

| 시나리오 | 값 | 근거 |
|---|---|---|
| Identical (sha same, presence=both) | `Some(false)` | normalize 전후 동일. diff 호출 stdout 0 bytes 확정. |
| sha differ + normalize-equal (presence=both) | `Some(false)` | F1 케이스 본체 — BOM/encoding 차이만 있는 sha drift. diff 호출 stdout 0 bytes. |
| sha differ + normalize-diff (presence=both) | `Some(true)` | 진짜 의미 차이. diff 호출 unified text 출력 expected. |
| LocalOnly / RemoteOnly / Failed | `None` | 비교 대상 한쪽 부재 또는 비교 불가 — diff_meaningful 정의 자체가 N/A. |

계산 근거: `spec-hash-and-normalize.md` § Normalize 규칙 재사용. compare 시점에 sha 비교 후 differ면 normalize-equal 검증 1회 추가.

### `presence: "local_only" | "both" | "remote_only"` (F2)

`#[serde(rename_all = "snake_case")]` enum. status와 직교 — status는 액션 분류, presence는 존재성 분류.

기각된 대안: 4-state status를 6-state로 split (`local_only_added` / `local_only_modified` 등). breaking change + 호출자 분기 늘어남 + 기존 status semantics (push/pull 후보 분류)와 presence (existence) 두 관심사를 한 enum에 묶는 형태라 직교성 깨짐.

### Backward compat

- schema_version v1.2 → v1.3 minor bump (additive only).
- v1.0/v1.1/v1.2 lock test 패턴 (Phase 7.2 task P) 그대로 적용 — 신규 field 부재 시에도 기존 caller 정상 parse.
- `presence` field 부재 case (LocalOnly/RemoteOnly entry)는 enum value로 명시되므로 caller 분기 명확.

## Consequences

- `spec-output-schema.md` § v1.2 → v1.3 변경 § 신규 + § Acceptance Criteria v1.3 § 신규 (task B/D/L scope, 본 ADR은 schema JSON 박지 않음 — spec이 authoritative).
- `spec-classification.md` § 상태 정의 변경 0 — 4-state 그대로 유지가 contract. presence는 별도 field로 case 구분 분리. **변경 없음 자체가 본 ADR의 결정**.
- `spec-hash-and-normalize.md` § Normalize 규칙 — diff_meaningful 계산이 본 § 재사용 (compare 시점 normalize-equal 검증 1회). spec 본문 변경 0.
- `CHANGELOG.md` v0.4.0 entry (task E) — Added: `diff_meaningful` + `presence` field, Changed: schema v1.2 → v1.3.
- 신규 코드 (Phase 8.2 task F~M scope): `compare.rs::FileEntry` 2 field + `Presence` enum + compare 함수 분기 + finalize plumbing + unit test 6 시나리오 + integration test (eval F1 evidence 케이스 합성 fixture) + SCHEMA_VERSION lock test 갱신.
- F3 (`diff --json`) + F4/F5/F6 (clap surface)는 별도 결정 trail — 본 ADR scope 외. spec/code 변경은 Phase 8.3/8.4에서 별도 처리.

## References

- `docs/research/llm-as-caller-usability-eval.md` § F1 + § F2 + § Open Decisions
- `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` § read-only 영구 (caller-decides 본성 — 본 field들이 존재하는 이유)
- `docs/specs/spec-output-schema.md` § v1.2 → v1.3 변경 (신규 § 후보)
- `docs/specs/spec-classification.md` § 상태 정의 (변경 0 contract)
- `docs/specs/spec-hash-and-normalize.md` § Normalize 규칙 (diff_meaningful 계산 근거)
