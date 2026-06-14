# Mayyam Claude Instructions

Use this file as persistent project guidance for Claude Code or Claude Fable runs in this repository.

## Mission

Mayyam is an SRE, cloud, database, Kafka, Kubernetes, Linux, FinOps, and operations platform. Treat the product goal as a replacement for passive observability tools: it must observe resources, explain issues from evidence, score Well-Architected posture, interact with resources safely, triage deterministically, support bounded agentic investigation, and turn cost data into actionable savings.

## Autonomous Operating Mode

- Work end-to-end when the user gives an implementation, review, roadmap, validation, or commit request.
- Do not stop at a plan when there is enough information to act.
- Ask the user only for destructive or irreversible actions, a scope change, or information that cannot be discovered locally.
- Before ending a response, check whether the last paragraph is only a plan, promise, question, or next-step list. If it is, do the work instead.
- Keep edits scoped to the requested outcome and the established project shape.
- Prefer existing repo patterns over new abstractions.
- Never revert unrelated user changes.

## Roadmap One-Shot Execution

When the task is to execute the Mayyam product roadmap, do not ask the user which roadmap item to start with.

- Start from `docs/product-roadmap/README.md`, `implementation-sequencing.md`, `requirements-rigor.md`, every `release-plan.md`, and every `feature-backlog.csv`.
- Enumerate all roadmap folders and count backlog rows before selecting work.
- Process the backlog in deterministic batches. Do not attempt to load all rows into context at once.
- Prioritize P0, then P1, then P2. Within each priority, prefer M1 inventory and M2 observable foundations before M3, M4, and M5 work.
- Use a progress ledger or checkpoint so a later run can resume exactly.
- If sub-agents are available, use them for backlog triage, Rust backend, React UI, tests, and independent verification. If sub-agents are unavailable, run those passes sequentially.
- Commit each completed, verified batch when the task definition requires commits.
- Never claim the whole roadmap is complete unless every row has been processed and verified.

## Parallel Batch Execution

Speed matters, but parallelism must not corrupt the worktree.

- Prefer 2-4 parallel lanes when the runtime supports sub-agents or background agents.
- Use SQLite as the coordination source of truth. Every agent must atomically claim feature IDs before work begins.
- Parallelize only independent slices: different services, different files, or analysis/test work that will not edit the same modules.
- Do not let two agents edit the same Rust module, route, controller, React file, migration, or generated file at the same time.
- Keep one coordinator responsible for batch selection, conflict checks, final integration, validation, staging, commits, and checkpoint updates.
- Agents should return compact evidence: claimed feature IDs, files changed, tests run, failures, commit readiness, and exact next action.
- Run expensive validations in parallel only when they do not compete for the same build lock or mutate shared output. Otherwise serialize validation.
- Commits must be serialized. One committed, verified batch at a time.
- If parallel lanes conflict, pause the lower-priority lane, write an event to SQLite, and let the coordinator decide whether to rebase, merge manually, or requeue.
- If sub-agents are unavailable, simulate parallel roles sequentially but keep the same SQLite claim/checkpoint protocol.

## SQLite Checkpointing Protocol

For roadmap one-shot execution, use SQLite checkpointing. This is preferred over long prose checkpoints because the backlog has tens of thousands of feature rows and may be processed by multiple agents.

Checkpoint location:

- Database: `.claude/checkpoints/roadmap-run/checkpoint.sqlite`
- Human/model resume file: `.claude/checkpoints/roadmap-run/RESUME.md`

Initialize SQLite with:

```sql
PRAGMA journal_mode=WAL;
PRAGMA busy_timeout=5000;

CREATE TABLE IF NOT EXISTS runs (
  id TEXT PRIMARY KEY,
  roadmap_hash TEXT NOT NULL,
  started_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_commit TEXT,
  current_batch_id TEXT,
  next_action TEXT
);

CREATE TABLE IF NOT EXISTS feature_progress (
  feature_id TEXT PRIMARY KEY,
  module TEXT NOT NULL,
  service_or_domain TEXT,
  priority TEXT,
  release_phase TEXT,
  status TEXT NOT NULL,
  batch_id TEXT,
  agent TEXT,
  commit_sha TEXT,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS batches (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL,
  selection_rule TEXT,
  agent TEXT,
  started_at TEXT,
  completed_at TEXT,
  commit_sha TEXT
);

CREATE TABLE IF NOT EXISTS events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts TEXT NOT NULL,
  event_type TEXT NOT NULL,
  batch_id TEXT,
  feature_id TEXT,
  agent TEXT,
  command TEXT,
  status TEXT,
  details TEXT
);
```

Use these feature statuses:

- `pending`
- `claimed`
- `in_progress`
- `implemented`
- `tests_passed`
- `committed`
- `blocked`
- `skipped`

Rules:

- Hash roadmap inputs before selecting work: all `feature-backlog.csv`, all `release-plan.md`, and `scripts/generate-product-roadmap.js`.
- Store one row per feature ID in `feature_progress`; do not store the full CSV row body unless needed.
- Claim work atomically so agents do not duplicate work: update from `pending` to `claimed` only when the row is still pending.
- Append every important action to `events`: batch selection, claim, file edit, test command, failure, verifier result, commit, pause, resume, and blocker.
- Treat a git commit as the strongest checkpoint. After each verified batch commit, update `runs.last_commit`, `batches.commit_sha`, and all completed `feature_progress.commit_sha` values.
- Keep `RESUME.md` tiny: current run ID, roadmap hash, last commit, current batch, completed batch count, blocker if any, and exact next action.
- On resume, read `RESUME.md`, verify `checkpoint.sqlite`, verify the roadmap hash, verify `last_commit`, check `git status --short`, then continue from `runs.next_action`.

## Evidence Rules

- Ground claims in the current repo, command output, generated files, tests, or logs from the current run.
- Do not say work is complete until it is implemented and verified.
- If validation fails, report the exact command and the failing behavior.
- If the working tree is dirty, stage only files that belong to the requested change.

## Engineering Standards

- Use test-driven development for non-trivial behavior: write or update the smallest meaningful failing test first, implement the behavior, then refactor with tests passing.
- Use domain-driven design: keep domain rules in small, named Rust modules or pure functions; keep controllers/routes thin; keep persistence, provider clients, and UI concerns outside core domain logic.
- Keep files small and cohesive. Split large files when a new behavior creates a clear domain boundary, but avoid broad refactors unrelated to the task.
- Prefer explicit types, reason-coded errors, deterministic evaluators, and evidence objects over stringly typed control flow.
- Add unit tests for domain logic and API tests for route/controller behavior when backend behavior changes.
- Add React component or integration tests for critical UI workflows when frontend behavior changes.
- Do not mock Mayyam's actual backend logic to make tests pass. Test real domain/service code with fixtures, test databases, fake external inputs, or local adapters. Mock only true external boundaries such as AWS, cloud APIs, network services, LLM providers, or clocks, and keep those mocks behind clear interfaces.
- Do not hide defects with snapshots, brittle assertions, or skipped tests. Fix the implementation or narrow the test to the actual contract.

## Default Validation

Use the smallest validation set that matches the change:

- Backend Rust: prefer incremental validation. Do not run `cargo clean`, delete `target/`, or force a full clean rebuild unless explicitly requested.
- Backend domain logic: run targeted tests first with `cargo test <module_or_test_name>` from `backend/`.
- Backend library check: run `cargo check --lib` from `backend/` when the change is library/service/model logic and does not require binary wiring.
- Backend full incremental check: run `cargo check` from `backend/` after route, controller, binary, dependency, or cross-module changes. This may take time because of AWS SDK dependencies, so reuse the existing incremental build cache.
- Frontend React: run `npm run build` from `frontend/`.
- Roadmap generator: run `node --check scripts/generate-product-roadmap.js` and `node scripts/generate-product-roadmap.js`.
- Generated roadmap sanity: scan generated docs for `undefined`, `[object Object]`, `TODO`, and `NaN`.

Warnings may already exist. Treat command exit codes and new failures as the signal.

## Git Discipline

- Commit only when the user asks for a commit or the one-shot task explicitly includes committing in its definition of done.
- Before committing, run `git status --short` and review the intended diff.
- Use clear, specific commit messages.
- Leave the working tree clean after a commit when possible.

## Context Budget and Clear Discipline

For long roadmap execution runs, context pressure is expected. Do not keep dragging stale context after a completed batch.

- After every committed batch, update `checkpoint.sqlite` and `RESUME.md`, then prefer ending the current Claude session so the next run can start from a compact checkpoint.
- If the visible token count is high, roughly above 120k tokens, or Claude enters a long context-compaction/metamorphosing phase, stop doing new implementation work.
- Before stopping for context pressure, write a complete checkpoint: current batch, feature IDs, changed files, commands run, verification state, last commit, blocker if any, and exact next action.
- Do not use `/permissions` as a fix for token/context pressure. Permissions affect tool access, not context size.
- After the user runs `/clear`, resume by reading `CLAUDE.md`, `.claude/checkpoints/roadmap-run/RESUME.md`, and the SQLite checkpoint. Verify roadmap hash, last commit, and `git status --short`, then continue from `runs.next_action`.
- Do not re-enumerate the full roadmap after `/clear` unless the roadmap hash changed or the checkpoint is invalid.
- Keep progress updates short. Put durable state in SQLite and `RESUME.md`, not in chat.

## Long-Running and Token-Limit Behavior

Claude cannot restart itself after an API hourly token or rate limit. The external harness must catch the limit response and requeue the task after the reset window.

If a token/rate limit or context interruption happens, write a checkpoint before stopping:

- Objective.
- Completed work.
- Files changed.
- Commands run.
- Verification status.
- Current blocker.
- Exact next action.
- Resume instructions.

On resume, read the checkpoint and continue from the exact next action instead of restarting from scratch.

## Final Response

Lead with the outcome. Include changed files, validation commands, commits, and remaining blockers. Keep the answer concise and factual.
