# Building Mode Prompt

## 0. Orientation
Read in this order, fully:
0a. `CLAUDE.md` — project overview, constraints, architecture, critical rules.
0b. `docs/ralph/guardrails.md` — known failure patterns. Apply preemptively.
0c. `docs/ralph/project-ops.md` — exact validation commands.
0d. `docs/ralph/implementation-plan.md` — task list with statuses.
0e. The relevant `docs/specs/spec-*.md` referenced by the task you're about to do.

## 1. Task Selection
- Find the **first uncompleted task** (`[ ]`) in `implementation-plan.md` whose dependencies are all `[x]` and which is not tagged `[HUMAN]`.
- If no such task exists (only `[HUMAN]` or `[!]` blocked remain), output `<promise>COMPLETE</promise>` and exit.
- Mark the selected task `[~]` (in progress) before starting. Commit this status change.

## 2. Implementation
- **Spec-only task** (Files가 `docs/specs/*.md` 또는 `docs/ralph/*.md`만): 본 § 2의 코드 작성 룰 적용 제외 — spec/문서 본문 갱신만 수행. Coverage 게이트는 G-012 spec-only 케이스 적용으로 baseline 유지로 자동 통과.
- Code lives in `crates/gitless-sync/src/`. Do not create files outside this tree unless the task says so.
- Follow the architecture in `CLAUDE.md`: vertical slice (`commands/<name>/`) vs `shared/`. Don't put command-specific logic in `shared/`.
- Replace `todo!()` with real implementations. If a function signature needs to change, update the corresponding spec acceptance criterion in `implementation-plan.md` first.
- Add unit tests in the same file (`#[cfg(test)] mod tests`). Coverage gate is 80% (`project-ops.md`).
- GitHub API mocking uses `GhClient` trait + `MockGhClient` inject (M0~M2). M2 완료 후 `mockito` import 추가 금지. M2 진행 중 룰은 `guardrails.md` G-009 참조.

## 3. Validation (Backpressure)
Run in order. Do NOT proceed to step 4 until all pass.
1. `cargo fmt --check` — if fails, run `cargo fmt` and re-check.
2. `cargo clippy --all-targets -- -D warnings` — fix all warnings.
3. `cargo test --workspace` — all tests must pass.
4. `cargo tarpaulin --engine llvm --workspace --out Stdout` — coverage ≥ 80%. (G-012 적용 — spec-only task / `todo!()` 잔존 task는 baseline 유지로 자동 통과.)

If any step fails after a reasonable fix attempt:
- Update `docs/ralph/guardrails.md` with a new `G-NNN` entry describing the failure pattern.
- Mark the task `[!]` in `implementation-plan.md` with a one-line reason.
- Commit progress so far + guardrail update + plan update.
- Output `<promise>BLOCKED</promise>` and exit. (The loop will continue but skip this task next iteration.)

## 4. Post-Implementation
- Update `docs/ralph/implementation-plan.md`: mark task `[x]`, increment "Completed" counter.
- If you discovered a new failure pattern *after the fact* (rare), add it to `guardrails.md`.
- Commit: stage only the files this task touched + `git commit -m "feat: {task title}"` (or `fix:` / `test:` / `refactor:` as appropriate). Do NOT use `git add -A` indiscriminately.
- Verify commit succeeded with `git log -1 --oneline`.

## 5. Exit
- **ONE task per iteration.** After post-implementation step is fully done, exit.
- If uncompleted tasks remain (any `[ ]` whose deps are met), just exit silently — the loop starts next iteration.
- If ALL tasks are `[x]` (excluding `[HUMAN]` / `[!]` which require human action), output `<promise>COMPLETE</promise>` on its own line and exit.

## Constraints
- **Complete implementations only.** No `todo!()` left in your task's scope. No commented-out code.
- **Commit before exit.** No exit with uncommitted changes.
- **Do NOT modify other tasks' files.** Stay within the `Files` listed in your task.
- **Do NOT skip validation steps.** Even if "obviously fine".
- **Do NOT touch `docs/specs/*.md` or `CLAUDE.md`** unless the task explicitly says so.
