# LLM-as-Caller Usability Evaluation (vault dogfood)

> 2026-05-10 record. v0.3.0 release 후 Claude Code 1회 호출자가 README → `--help`
> → `init` → `scan` → `diff` 워크플로를 0-shot으로 따라가며 LLM 관점 사용성을
> 평가. 측정 데이터 + 마찰점 7건 + 개선 후보를 사실로 박아둔다. 결정/우선순위
> 분해는 ralph plan 모드 (다른 PC) 에서 본 file을 input으로 진행.
>
> **주의**: 본 평가는 "사람용 사용성"이 아니라 "LLM caller가 spec 안 읽고 결과만
> 보고 다음 액션을 결정 가능한가" 기준. 마찰점 우선순위도 인간 UX가 아닌 LLM
> token budget / 의미 일관성 / chain-friendliness 기준.

## Measurement Setup

| Field | Value |
|---|---|
| Date | 2026-05-10T07:47Z |
| OS | Windows 11 Pro 10.0.26200 |
| Rust | stable 1.95.0 |
| `gh` CLI | authenticated as `KneShell` (token scopes: gist, read:org, repo, workflow) |
| Binary | `target/release/gitless-sync.exe` (rebuilt from `e9f2b75` after stale-binary friction — see § Friction #7) |
| Backend | default (`graphql`, ADR 0006) |
| Local | `C:\Users\admin\iCloudDrive\iCloud~md~obsidian` (iCloud-synced obsidian vault) |
| Remote | `KneShell/obsidian-vault` branch `main` |

## Raw Measurements

### scan summary (`--summary-only --pretty`)

```json
{
  "schema_version": "1.2",
  "scanned_at": "2026-05-10T07:47:15.661690700Z",
  "repo": "KneShell/obsidian-vault",
  "branch": "main",
  "local_root": "C:\\Users\\admin\\iCloudDrive\\iCloud~md~obsidian",
  "summary": {
    "identical": 281,
    "local_only_changed": 60,
    "remote_only_changed": 22,
    "drift": 0,
    "failed": 0
  }
}
```

`exit 0`, stderr empty, 11 lines. 363 files total.

11일 전 (v0.1, 2026-04-29) 결과 (`docs/research/phase5-vault-after.md` 동일 vault):
284 / 55 / 17 / 0 / 0 = 356 → 7 file 추가 + 양방향 변경 일부. **0 drift / 0 failed 유지** —
v0.2 코드의 함정 처리 (NFD/case/encoding/`.gitattributes`/BOM/LFS) regression 없음.

### Output size (full scan, minified)

| Mode | bytes | rough tokens (≈3.5 chars/token) |
|---|---:|---:|
| `--summary-only --pretty` | 339 | ~95 |
| 전체 (`files[]` 포함, minified) | **98,211** | **~28,000** |

200K context window 의 **12-15%**. 363 files vault에서 이 정도 — 10K+ files repo는
default 출력이 context budget killer. `--summary-only` / `--status drift,...`
filter가 LLM 호출의 1급 시민이어야 함을 입증.

### `diff` 케이스별 출력

같은 path `001_PARA/01 Project/OPIc AL/003-opic-practice-strategy.md` (scan 결과
`status: remote_only_changed`, `local_sha != remote_sha`) 에 `diff` 호출:

```
exit: 0
stdout bytes: 0
stderr bytes: 0
```

→ scan은 sha 다름이라 "drift type" 분류, diff 는 LF + BOM normalize 후 비교라
**의미 차이 없음** 판정. spec-hash-and-normalize.md 정합이지만 LLM caller에게는
"두 명령이 모순된 답" 인상 (§ Friction #1 참조).

`.obsidian/app.json` (status: `local_only_changed`, remote 미존재) 에 `diff` 호출:

```
exit: 0
stdout = (local 파일 전체 dump, 256 bytes)
stderr = "(local only)\n"
```

→ unified diff 가 아닌 "side marker on stderr + raw file dump on stdout" 형식.
README "single-file unified text diff" 약속과 불일치 (§ Friction #3).

### Error UX raw

```
$ gitless-sync scan ... --status drif
{"error_code":"CONFIG_ERROR","message":"Configuration error: invalid --status value: drif"}
exit 1
```

```
$ gitless-sync diff "nonexistent/path.md" ...
{"error_code":"CONFIG_ERROR","message":"Configuration error: path not found locally or remotely: nonexistent/path.md"}
exit 1
```

→ 한 줄 JSON, machine-parseable. 입력값 echo back. **단 valid 후보를 알려주지
않음** — `--status drif` 응답이 `[identical, local_only_changed, remote_only_changed, drift, failed]`
중 어느 것을 의미했는지 LLM 추측 필요.

### `-v` verbose stderr

```
info: scanning C:\Users\admin\iCloudDrive\iCloud~md~obsidian against KneShell/obsidian-vault@main
info: found 348 local files, 307 remote files
```

→ 사람용 free-form text. JSON 아님. (StaerR 는 일반적으로 사람용이 맞으니 트레이드오프
유효. **단** 348 local + 307 remote 와 summary 합 363 의 차이 = union 기준 vs raw
기준 dedup 차이를 LLM 이 spec 안 읽고는 매핑 못 함.)

## Strengths (AI-Native 인정)

| # | 항목 | Evidence |
|---|---|---|
| S1 | stdout = 결과 JSON, stderr = 사람용 로그/JSON 에러 분리 | summary scan 시 stderr empty, init 시 hint 만 stderr |
| S2 | `schema_version` 박혀있음 (`"1.2"`) | 미래 schema 변경 시 LLM 분기 가능 |
| S3 | echo-back 메타데이터 (`repo`/`branch`/`local_root`/`scanned_at`) | 결과 JSON만으로 호출 컨텍스트 복원 |
| S4 | drift 있어도 `exit 0` | drift = 데이터, 에러 아님 (spec-cli-interface.md 정합) |
| S5 | 에러도 한 줄 JSON | `{"error_code":"...","message":"..."}` machine-parseable |
| S6 | 토큰 압축 옵션 1급 시민 | `--summary-only` 단독 / `--status a,b` comma 합집합 |
| S7 | chain-friendly path | scan 결과 `path` (forward slash, space 포함) → quote만으로 `diff` 인자로 그대로 패스 |
| S8 | `init` = stdout-only pure function | redirect 힌트는 stderr (ADR 0004 정합) — LLM-safe side-effect 없는 호출 |

## Friction Points (AI-Native라 부르기 어려움)

각 finding 마다 Evidence + Why LLM-unfriendly + Improvement candidate (어디에 박을지 hint).

### F1 — `scan` 과 `diff` 가 다른 비교 기준

**Evidence**: `003-opic-practice-strategy.md` 가 `scan` 에서는
`remote_only_changed` (`local_sha != remote_sha`), 동일 path `diff` 는 stdout 0
bytes + stderr 0 bytes + exit 0. § Raw § `diff` 케이스 참조.

**Why LLM-unfriendly**: 자체정의 해시 (BOM/encoding 차이 포착) vs LF+BOM normalize
후 비교 — spec-hash-and-normalize.md 에는 박혀있지만 LLM caller 는 spec 안 읽음.
같은 도구 안에서 "한쪽 명령은 different, 다른 명령은 identical" 답이 나오면
도구 신뢰 자체가 흔들림. 0-shot 으로 "왜?" 답 못 함.

**Improvement candidate** (P0):
- (a) scan entry 에 `diff_meaningful: bool` hint 필드 추가 — sha differ but normalize-equal 이면 false. spec-output-schema.md `1.3` 후보.
- (b) `diff` stderr 에 "no semantic diff (sha differ due to encoding/normalization)" 한 줄 박기. spec-cli-interface.md `diff` 섹션.
- (a) 가 LLM-friendly 우위 — 호출 1회로 정보 다 받음. (b) 는 호출 2회 필요.

### F2 — status 이름이 self-explanatory 하지 않음

**Evidence**: `local_only_changed` 가
- (i) `.gitignore` (local 미존재, remote 만 존재) 케이스 — `local_sha` 필드 없음
- (ii) `.obsidian/app.json` (양쪽 존재, local 만 변경) 케이스 — `local_sha` 필드 있음

둘 다 같은 status 로 들어옴. § Raw 첫 50줄 참조.

**Why LLM-unfriendly**: `local_only_changed` 라는 이름이
- "로컬에만 변경됨, 원격은 그대로 (둘 다 존재)"
- "로컬에만 존재 (remote 미존재)"

둘 중 어떤지 spec-classification.md 안 읽으면 모름. 실제 의미 = "remote 기준 local
이 다름" 의 union. 이 모호성 때문에 LLM 이 결과만 보고 다음 액션 (예: "remote
에 없는 새 파일이니 push 후보" vs "remote 와 sync 안 됐으니 conflict 검토") 결정
못 함.

**Improvement candidate** (P0):
- entry 에 plain `presence` 필드 추가: `"local_only" | "both" | "remote_only"`. 4분류 status 는 그대로 (backward compat), presence 가 case 구분.
- spec-output-schema.md `1.3` 후보. 현 schema_version `1.2` → backward-compat lock test 패턴 (Phase 7.2 task P) 그대로 적용.
- 대안 (heavier): 4 → 6분류로 status 자체를 쪼개기 (`local_only_added` / `local_only_modified` / ...). presence 필드 추가 보다 비용 높고 호출자 분기 늘어남.

### F3 — `diff` 출력 형식이 케이스별로 분기

**Evidence**: § Raw § `diff` 케이스 두 가지.
- 양쪽 normalize-equal: stdout empty + stderr empty + exit 0
- 한쪽만 존재: stderr `(local only)` 마커 + stdout raw file dump
- 양쪽 normalize-diff: (vault drift 0 이라 본 평가에서 미시연) — README 약속대로 unified diff 형식 추정

**Why LLM-unfriendly**: README `### diff` 는 "single-file unified text diff" 라고만
약속. LLM 이 한 형식 기대했다 다른 형식 받으면 파싱 분기 필요. 또 stderr 에
side marker 가 가는 게 stdout 과 매핑 어려움 (특히 stderr/stdout 분리해서 받는
호출 패턴에선 "(local only)" 신호 손실).

**Improvement candidate** (P1):
- (a) `--json` 옵션 추가: `{"side": "local_only|remote_only|both", "unified": "...", "raw": "..."}` 형식. spec-cli-interface.md `diff` 섹션 + spec-output-schema.md 별도 sub-schema.
- (b) README `diff` 섹션에 3 케이스 명시 (cheaper, 임시 mitigation).
- (a) 가 본질 — `diff` 가 LLM caller 대응 1급 시민이 되려면 unified text 만으로는 부족.

### F4 — `--help` 옵션 description 거의 비어있음

**Evidence**:

```
Options:
      --summary-only       
      --status <STATUS>    
      --repo <REPO>        
      --branch <BRANCH>    
      --local <LOCAL>      [default: .]
      --ignore <IGNORE>    
      --keep-bom           
      --pretty             
      --backend <BACKEND>  [default: graphql] [possible values: rest, graphql]
```

`--summary-only`/`--status`/`--ignore`/`--keep-bom`/`--pretty` 전부 한 줄 설명 없음.

**Why LLM-unfriendly**: LLM 은 보통 `--help` 출력으로 인터페이스 1차 학습. README/spec
까지 안 읽으면 (예: ralph 자율 실행에서 fresh sub-claude) 의미 모름. clap doc-comment
한 줄씩 박으면 끝나는 일.

**Improvement candidate** (P1):
- `crates/gitless-sync/src/commands/scan/args.rs` 의 clap struct 각 field 위에 `///` doc comment 한 줄씩. `commands/init/mod.rs` 와 `commands/diff/args.rs` 도 동일.
- 작업량 작고 PR 1개로 끝남.

### F5 — `--status` 에 `possible_values` 미표시 + 에러에 valid 후보 미명시

**Evidence**:
- `--help` 출력: `--backend <BACKEND>  [default: graphql] [possible values: rest, graphql]` — 후보 박혀있음
- 같은 출력: `--status <STATUS>` — 후보 비어있음, free-form
- 잘못된 입력 에러: `"invalid --status value: drif"` — valid 후보 echo 안 됨

**Why LLM-unfriendly**: 같은 clap 패턴인데 `--backend` 와 `--status` 가 비대칭. `--status`
도 fixed enum (`identical`/`local_only_changed`/`remote_only_changed`/`drift`/`failed`)
인데 string 으로 받고 자체 validate. 에러 메시지에도 후보 없음 — LLM 이 첫 시도 실패시
다음 시도 추측 필요.

**Improvement candidate** (P1):
- `--status` 를 `Vec<StatusFilter>` enum 으로 바꾸고 clap `value_enum` derive (또는 `EnumValueParser` 수동). comma 합집합은 `value_delimiter = ','` 또는 `ArgAction::Append` + `--status drift --status local_only_changed` 두 형태 모두.
- 자동으로 `--help` 에 `[possible values: ...]` 나오고 에러 메시지에도 valid 후보 자동.
- spec 변경 없음 (이미 5 카테고리 spec 박혀있음). `commands/scan/args.rs` 만 수정.

### F6 — `--branch` 기본값 `--help` 표시 없음

**Evidence**: `--help` 출력 `--branch <BRANCH>` (no default 표기). README 는
"Branch defaults to main" 명시. 코드 어디선가 fallback 적용 중.

**Why LLM-unfriendly**: LLM 이 `--help` 만 신뢰하면 default 없는 줄 알고 매번
`--branch main` 명시. 또는 default 값을 모른 채 호출했다가 의도와 다른 branch
조회 가능.

**Improvement candidate** (P2):
- `commands/scan/args.rs` 등 `branch: Option<String>` 에 clap `default_value = "main"` 또는 `Files cascade` (config → CLI → default) 의 default leg 노출.
- 또는 README 의 "defaults to main" 약속을 `--help` 에도 박기 (clap `long_help` 활용).
- 기능 변화 없음, surface 명시화만.

### F7 — 자기 도구의 역설 (stale binary)

**Evidence**: 본 평가 첫 시도시 `target/release/gitless-sync.exe` 가 옛 빌드
(M3 token 제거 / Phase 2 init 추가 미반영) 상태. README `## Quick Start` 따라 `init`
호출 → `error: unrecognized subcommand 'init'`. 또 top-level `--help` 에 폐기된
`--token` 플래그 그대로. `cargo build --release` 23 초 후 해소.

**Why LLM-unfriendly** (도구 본질 결함은 아님): drift 검출 도구의 README 와 binary
가 자기 자신에게 "stale binary drift" 가 있는 상태. 사용자가 README 따라 `gitless-sync init ...`
직접 호출하면 첫 명령부터 실패. CI 가 README 코드블록을 실제 실행하는 sanity test 가
없음.

**Improvement candidate** (P3):
- (a) CI workflow 에 README 코드블록 추출 → 실제 실행 step 추가 (예: `markdown-link-check` 같은 패턴 응용 또는 `mdsh` 류). xtask `check-readme-examples` sub-command 자체 구현도 가능.
- (b) `release/` 산출물 자동화 (GitHub Releases 에 `gitless-sync-x86_64-pc-windows-msvc.exe` 자동 업로드, README 가 binary download 안내). 사용자가 "build from source" 안 하고도 latest binary 받음.
- 우선순위 낮음 — 본 평가 외 케이스에서는 사용자/ralph 가 매번 `cargo build --release` 박는 게 패턴이라 marginal.

## Open Decisions (사용자 결정 필요)

ralph plan 모드에서 다음 중 어느 개선을 v0.4 에 반영할지 결정 필요. spec/ADR 갱신
없이 작업 진입 불가.

| 결정 | 후보 | 영향 |
|---|---|---|
| F1 해소 방식 | (a) `diff_meaningful` field | spec-output-schema 1.3, scan output entry 추가 |
|   | (b) diff stderr hint | spec-cli-interface diff 섹션, code change minimal |
| F2 해소 방식 | (a) `presence` field 추가 | spec-output-schema 1.3, 4분류 status 유지 |
|   | (b) status 4 → 6분류 | spec-classification 재작성, breaking |
| F3 해소 방식 | (a) `diff --json` 옵션 | spec-output-schema sub-schema, 추가 surface |
|   | (b) README 명시만 | 임시 mitigation |
| F1+F2 → ADR | new ADR 0014 후보: "scan-diff metadata contract" | F1+F2 묶어 처리 가능 |

ralph plan 모드 input 으로 본 file 전체 + advisor() 거치면 어떤 묶음으로
implementation-plan.md 에 박을지 결정 가능.

## One-line Verdict

> **출력 형식·exit·에러 JSON 은 진짜 AI-friendly 한데, 명령 간 의미 일관성 (F1) 과
> surface self-documentation (F2/F4/F5) 두 축이 약해 LLM 이 spec 안 읽고 0-shot 으로
> 다음 액션 결정하긴 부족.** "사람용 unix CLI 에 JSON 옵션을 잘 박은 수준"
> → "LLM 이 결과만 보고 결정 가능한 수준" 으로 격상하려면 F1 + F2 가 가장 큰 레버.

## Acceptance

- [x] vault scan summary + delta-from-v0.1 박제 (363 / 4분류 / 0 drift)
- [x] 출력 사이즈 측정 (full minified 98,211 bytes ≈ 28k tokens)
- [x] `diff` 3 케이스 중 2 케이스 시연 (drift 0 이라 unified diff 케이스 미시연 — Limitations §1)
- [x] error UX 2 케이스 raw 박제 (잘못된 status / 없는 path)
- [x] `-v` verbose stderr 형식 박제
- [x] Strengths 8건 / Friction 7건 / Open Decisions 4건 분리 작성
- [x] 각 Friction 에 Evidence + Why LLM-unfriendly + Improvement candidate 분리

## Limitations

1. **drift > 0 케이스 미시연**: 본 vault 가 0 drift 상태라 `diff` 양쪽 normalize-diff
   케이스 (정상 unified diff 출력) 직접 측정 못 함. README 약속 형식 추정만.
   → 다른 PC 에서 drift surface 한 vault (예: dogfood self repo 의 인위적 drift
   fixture) 로 보충 측정 권장.
2. **단일 vault**: obsidian markdown 위주 → encoding/long-path/submodule/symlink 함정
   surface 0건. spec-domain-pitfalls.md 의 다른 함정 (P0~P3) 은 본 평가 범위 외.
3. **단일 caller (Claude Opus 4.7)**: 다른 LLM (예: Sonnet, Haiku, GPT-5) 에서도
   같은 마찰점 나오는지 미검증. 작은 모델일수록 spec 못 따라가니 Friction 영향
   더 클 가능성 (가설).

## References

- `README.md` — caller 가 실제 따라간 가이드.
- `docs/specs/spec-output-schema.md` — F1/F2/F3 개선 후보 spec 위치.
- `docs/specs/spec-cli-interface.md` — F3/F4/F5/F6 surface 정의.
- `docs/specs/spec-classification.md` — F2 status 의미.
- `docs/specs/spec-hash-and-normalize.md` — F1 비교 기준 차이의 근원.
- `docs/research/phase5-vault-after.md` — 11일 전 동일 vault 측정 (델타 비교).
- `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` — read-only 본성 + 호출자 책임 분리 (LLM-friendly 본 설계 의도).
- `docs/adr/0004-init-stdout-redirect.md` — S8 (init pure function) 결정 근거.
