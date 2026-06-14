# Codex One-Shot Roadmap Execution Prompt

Use this from the repo root. If Codex supports Goal mode, paste it after `/goal`; otherwise paste it as a normal prompt.

```text
Read and follow AGENTS.md first.

Objective: pick up the next executable Mayyam roadmap task batch and carry it through implementation, validation, checkpointing, and commit.

Work mode:
- Do not stop at planning if there is enough local context to act.
- Resume from an existing checkpoint if present:
  - Prefer .agents/checkpoints/roadmap-run/RESUME.md and checkpoint.sqlite.
  - If only .claude/checkpoints/roadmap-run exists, inspect it and continue from that checkpoint unless migration is required.
  - If both checkpoint locations exist, compare runs.last_commit, current_batch_id, next_action, and checkpoint freshness; continue from the freshest valid checkpoint and record the choice in events.
- If no valid checkpoint exists, initialize the SQLite checkpoint protocol from AGENTS.md.
- If the working tree is dirty, identify unrelated user/agent changes and do not revert them.

Task selection:
- Start from docs/product-roadmap/README.md, implementation-sequencing.md, requirements-rigor.md, every release-plan.md, and every feature-backlog.csv.
- Enumerate roadmap folders and count backlog rows before choosing work.
- Process the backlog in deterministic batches. Do not attempt to load all rows into context at once.
- Select a small deterministic batch:
  - P0 before P1 before P2.
  - Prefer M1 inventory and M2 observable foundations before M3/M4/M5.
  - Choose related rows that can be implemented and verified together without broad refactors.
- Atomically claim selected feature IDs in the checkpoint database before editing.

Batch loop:
- Continue through completed and committed batches until the roadmap is finished or a hard stop condition is reached. Do not stop merely because a batch was completed, a checkpoint was written, a commit succeeded, or a concise status update could be reported.
- After each verified commit:
  - Update checkpoint.sqlite and RESUME.md.
  - Re-read the active checkpoint.
  - Verify git status --short, runs.last_commit, current_batch_id, next_action, and the roadmap hash.
  - Select and atomically claim the next deterministic batch using the same P0 -> P1 -> P2 priority rules.
  - Implement, validate, checkpoint, commit, and repeat.
- Stop the loop only when a hard stop condition is hit:
  - no pending roadmap rows remain
  - validation fails and cannot be fixed within the current batch
  - unrelated worktree changes conflict with the next batch
  - a real blocker prevents progress
  - context, token, rate-limit, or timeout pressure makes another implementation batch unsafe
- Treat context, token, rate-limit, and timeout pressure as hard stop conditions only when there is concrete evidence that continuing another batch would likely lose work or prevent a checkpoint. Do not stop speculatively.
- Before stopping, write a complete checkpoint with the current batch, feature IDs, changed files, commands run, verification state, last commit, blocker if any, and exact next action.
- Codex cannot restart itself after API token, context, process, or network limits. For multi-hour unattended execution, run Codex from an external harness that relaunches it with this prompt until the checkpoint reports no pending roadmap rows; SQLite and RESUME.md are the handoff contract.

Execution:
- Use existing backend/frontend patterns.
- Use TDD for non-trivial behavior.
- Keep Rust domain logic small and explicit; keep routes/controllers thin.
- Keep React UI consistent with the current CoreUI/AG Grid style.
- Do not mock Mayyam's own logic just to pass tests.
- Never revert unrelated user changes.

Validation:
- Run the smallest meaningful validation for the changed surface.
- Backend examples: cargo test <target>, cargo check --lib, or cargo check from backend/.
- Frontend examples: npm run build, CI=true npm test -- --watchAll=false, or npm run test:e2e from frontend/.
- If roadmap docs are regenerated, run node --check scripts/generate-product-roadmap.js and node scripts/generate-product-roadmap.js.
- Record every important command and result in the checkpoint events table.

Checkpoint and commit:
- Update checkpoint.sqlite and RESUME.md after selection, implementation, validation, blockers, and completion.
- Use the statuses from AGENTS.md: pending, claimed, in_progress, implemented, tests_passed, committed, blocked, skipped.
- Treat a git commit as the strongest checkpoint.
- If validation passes, create one clear git commit for the completed batch.
- Stage only files belonging to this batch and its checkpoint updates.
- After committing, update:
  - runs.last_commit
  - runs.current_batch_id
  - runs.next_action
  - batches.commit_sha
  - completed feature_progress.commit_sha
  - RESUME.md
- Keep RESUME.md tiny: run ID, roadmap hash, last commit, current batch, completed batch count, blocker if any, exact next action, and latest verification evidence.

Git sandbox handling:
- Run read-only git commands such as git status, git diff, git log, and git show normally.
- Treat mutating git commands such as git add, git commit, git tag, git merge, git rebase, git cherry-pick, git stash, and git reset as .git writes that may require escalated permissions in restricted workspaces.
- If a required mutating git command fails with Permission denied, Operation not permitted, or unable to create .git/index.lock, rerun only the necessary git command with escalated permissions and a concise justification.
- Do not work around sandbox failures by copying the repository, manually editing .git, using sudo, or running destructive cleanup.
- Never run destructive git operations unless the user explicitly requested them.

If blocked:
- Do not spin.
- Record the blocker, evidence, failed command if any, and exact next action in RESUME.md and checkpoint.sqlite.
- Report the blocker concisely.

Final response:
- Lead with the outcome.
- Include selected feature IDs, files changed, validation commands, commit SHA if committed, and remaining blockers or next action.
```
