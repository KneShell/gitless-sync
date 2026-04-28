# Planning Mode Prompt

## 0. Orientation
Read in this order, fully:
0a. `CLAUDE.md` — project overview, constraints, architecture, critical rules.
0b. All `docs/specs/*.md` — feature requirements + acceptance criteria.
0c. `docs/ralph/guardrails.md` — known failure patterns to avoid.
0d. `docs/ralph/project-ops.md` — build/test/validate commands.
0e. If `docs/ralph/implementation-plan.md` exists, study it. Identify completed (`[x]`) vs uncompleted (`[ ]`) tasks.

## 1. Gap Analysis
- Read `crates/gitless-sync/src/**/*.rs`. Compare current code state against `docs/specs/*.md` acceptance criteria.
- For each spec, list which acceptance criteria are met (compile + test passes) vs unmet (still `todo!()`, missing tests, missing struct fields, etc).
- Cross-reference with `implementation-plan.md` (if exists) to detect drift between plan and reality.

## 2. Plan Generation
Output / update `docs/ralph/implementation-plan.md` in this structure:

```markdown
# Implementation Plan: gitless-sync v0.1

## Status
- Last updated: {ISO-8601 UTC}
- Total tasks: {N}
- Completed: {M} / {N}

## Tasks
### T01. {Task title}
- **Spec reference**: `docs/specs/spec-{topic}.md` § {section}
- **Files**: {list of files to add/modify}
- **Depends on**: {T-IDs or "none"}
- **Acceptance criteria** (all must pass):
  - `[AUTO]` {machine-checkable criterion 1}
  - `[AUTO]` {criterion 2}
  - `[HUMAN]` {if any human verification needed}
- **Status**: `[ ]` not started / `[~]` in progress / `[x]` done / `[!]` blocked

### T02. ...
```

## 3. Task Design Rules
- **One iteration = one task.** Granularity: implementable + testable + commitable in a single ralph build iteration (~15-30 min).
- **Acceptance criteria are machine-checkable** (cargo test passes, specific test name passes, clippy clean, file contains specific symbol). Avoid "works correctly" or "looks good".
- **Dependencies in topological order.** A task that depends on T03 must come after T03.
- **`[HUMAN]` tag** for tasks that require non-automated verification (e.g., visual inspection, manual API call against real GitHub). `[HUMAN]` tasks are skipped by build mode and remain `[ ]` until a human marks them `[x]`.
- **No micro-tasks.** "Add a const" is not a task. "Implement IgnoreMatcher::new + 4 unit tests" is.

## 4. Exit
- **Do NOT write code in any `crates/**/*.rs` file.** Planning mode only edits `docs/ralph/implementation-plan.md`.
- When `implementation-plan.md` covers all unmet acceptance criteria from gap analysis, output `<promise>COMPLETE</promise>` on its own line and exit.
- If gap analysis reveals nothing to plan (all acceptance criteria already met), output `<promise>COMPLETE</promise>` immediately.

## Constraints
- File modification: only `docs/ralph/implementation-plan.md`.
- No `cargo build`, no `cargo test`, no commits in plan mode.
- If a spec is ambiguous, do NOT guess — add a `[BLOCKED]` task that says "clarify with human: {specific question}".
