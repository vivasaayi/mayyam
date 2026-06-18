# Mayyam Agent Instructions

Use this file as persistent project guidance for Codex runs in this repository. These instructions apply to the whole repo unless a nested `AGENTS.md` or `AGENTS.override.md` provides more specific guidance.

## Mission

Mayyam is an SRE, cloud, database, Kafka, Kubernetes, Linux, FinOps, and operations platform. Treat the product goal as a replacement for passive observability tools: it must observe resources, explain issues from evidence, score Well-Architected posture, interact with resources safely, triage deterministically, support bounded agentic investigation, and turn cost data into actionable savings.

## Repository Shape

- Backend: Rust with Actix Web in `backend/`.
- Frontend: React with CoreUI and AG Grid in `frontend/`.
- Docker and local bootstrap scripts live at the repository root and under `scripts/`.

## Autonomous Operating Mode

- Work end-to-end when the user gives an implementation, review, roadmap, validation, or commit request.
- Do not stop at a plan when there is enough information to act.
- Ask the user only for destructive or irreversible actions, a scope change, or information that cannot be discovered locally.
- Before ending a response, check whether the last paragraph is only a plan, promise, question, or next-step list. If it is, do the work instead.
- Keep edits scoped to the requested outcome and the established project shape.
- Prefer existing repo patterns over new abstractions.
- Never revert unrelated user changes.

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

Use the smallest validation set that matches the change.

- Backend Rust: prefer incremental validation. Do not run `cargo clean`, delete `target/`, or force a full clean rebuild unless explicitly requested.
- Backend domain logic: run targeted tests first with `cargo test <module_or_test_name>` from `backend/`.
- Backend library check: run `cargo check --lib` from `backend/` when the change is library/service/model logic and does not require binary wiring.
- Backend full incremental check: run `cargo check` from `backend/` after route, controller, binary, dependency, or cross-module changes. This may take time because of AWS SDK dependencies, so reuse the existing incremental build cache.
- Backend full tests: run `cargo test --workspace --all-targets` from `backend/` when risk or scope warrants it.
- Frontend React: run `npm run build` from `frontend/`.
- Frontend tests: run `CI=true npm test -- --watchAll=false` from `frontend/` for testable UI behavior.
- Frontend e2e: run `npm run test:e2e` from `frontend/` when changing browser workflows covered by Playwright.
- Full local tests: run `make test` from the repo root when both backend and frontend behavior changed.

Warnings may already exist. Treat command exit codes and new failures as the signal.

## Local Runtime

- Preferred local compose flow:
  - `bash scripts/bootstrap.sh local up`
  - `bash scripts/bootstrap.sh local test`
  - `bash scripts/bootstrap.sh local down`
- Distributable flow:
  - `cp .env.distributable.example .env.distributable`
  - `bash scripts/bootstrap.sh distributable up`
- Common access points:
  - Frontend: `http://localhost:3000`
  - Backend API: `http://localhost:8085` in local bootstrap mode
  - PostgreSQL: `localhost:5432`
  - MySQL: `localhost:3306`
  - Kafka: `localhost:9092`
  - LocalStack: `localhost:4566`
- Never commit `.env`, generated credentials, tokens, or local secrets.

## Frontend Guidance

- Follow existing CoreUI, React, AG Grid, routing, and state patterns.
- Build the actual usable workflow first, not a marketing or placeholder page.
- Keep operational screens dense, calm, and scannable.
- Use established component libraries and icons already present in the frontend before adding new dependencies.
- Make states explicit: loading, empty, success, partial failure, validation errors, and permission or connectivity errors where applicable.
- Verify responsive layouts for user-facing UI changes.

## Aruvi Task Tracker MCP

Use Aruvi Studio's tasktracker MCP as the only product planning, claim, evidence, task selection, and checkpoint mechanism for roadmap or backlog work.

- Start roadmap, backlog, migration, and "pick up next task" work from the MCP task tracker.
- Prefer the typed MCP tools when available: `agent_work_runs_health`, `aruvi_agent_work`, `agent_work_items_claim_next`, `agent_work_evidence_append`, `agent_work_evidence_list`, and catalog/product tools.
- If typed tools are unavailable but the HTTP MCP bridge is reachable, use the bridge and the running server's `tools/list` result rather than assuming tool names from memory.
- Use Aruvi catalog entities as the product-management source of truth: product areas, capabilities, feature nodes, and work items belong in Aruvi Studio.
- Use Aruvi agent-work entities as the delivery source of truth: runs, ready items, claims, conflict zones, batches, leases, evidence, status transitions, and commit links belong in the tasktracker MCP.
- When selecting work, inspect run health and ready items, then atomically claim through MCP.
- Do not select roadmap or backlog work outside MCP.
- Use MCP conflict zones and leases for coordination. Keep heartbeats current during long work, and release or requeue claims if you cannot continue.
- Append implementation, validation, review, blocker, and commit evidence through MCP as the work progresses. Evidence should include changed files, commands, exit codes, and concise product outcome.
- Update item status through MCP. Use `in_progress` for partial vertical slices, `implemented` only when code is complete but not fully verified/committed, `tests_passed` after required validation, `committed` after the implementation commit is linked, and `blocked` only for a real blocker.
- If the MCP task tracker is unavailable, do not pick, claim, continue, or checkpoint roadmap/backlog work. Treat MCP unavailability as a blocker for task selection and durable progress.

## Roadmap One-Shot Execution

When the task is to execute the Mayyam product roadmap, do not ask the user which roadmap item to start with.

- Start from Aruvi tasktracker MCP run health and ready items.
- If no active MCP run exists, report that MCP has no active run to claim from.
- Process large or cross-domain roadmap runs through the Roadmap MapReduce Execution Model below. Do not attempt to load all rows into context at once.
- Prioritize P0, then P1, then P2. Within each priority, prefer M1 inventory and M2 observable foundations before M3, M4, and M5 work.
- Use Aruvi tasktracker MCP as the progress ledger so a later run can resume exactly.
- If subagents are available, use them for backlog triage, Rust backend, React UI, tests, and independent verification. If subagents are unavailable, run those passes sequentially.
- Commit each completed, verified batch when the task definition requires commits.
- Never claim the whole roadmap is complete unless every row has been processed and verified.

## Roadmap MapReduce Execution Model

Use this model for large roadmap runs. The goal is to reason globally, execute independently, integrate centrally, and verify continuously.

### Map Phase: Roadmap Decomposition

Before implementation, the coordinator reads MCP run health, ready items, feature context, existing evidence, and current repository state, then maps the available work into candidate macro-batches. This phase is non-mutating unless an MCP run must be initialized or repaired.

Identify:

- Product capabilities and feature IDs.
- Dependency relationships and critical-path blockers.
- Shared contracts, domain foundations, and acceptance gates.
- Affected backend modules, routes, controllers, schemas, migrations, generated outputs, frontend screens, components, and tests.
- Candidate macro-batches and safe parallel lanes.
- Conflict zones that must be serialized.

The Map phase must not produce implementation code unless the roadmap graph and candidate macro-batches are already known or valid in MCP.

### Shuffle Phase: Claiming and Coordination

Group dependency-ready work into macro-batches and assign lanes through Aruvi tasktracker MCP.

Rules:

- Every lane must atomically claim feature IDs through MCP before implementation.
- Do not assign two lanes to the same Rust module, route, controller, migration, generated file, React screen, shared UI component, or shared contract at the same time.
- Prefer independent lanes: backend domain/service work, provider integrations, React UI, roadmap/test fixtures, validation, and independent verification.
- If work overlaps, serialize it through the coordinator.
- If a conflict appears, pause the lower-priority lane, record the event in MCP, and requeue, merge, or split the work.

### Worker Phase: Independent Implementation

Each worker lane implements only its claimed macro-batch.

Workers must return compact evidence:

- Claimed feature IDs and batch ID.
- Files changed.
- Contracts or APIs changed.
- Tests added or updated.
- Commands run and results.
- Failures encountered.
- Product outcome achieved.
- Commit readiness.
- Exact next action.

Workers must avoid broad speculative scaffolding. Code is valuable only when it supports a verified workflow, shared foundation, acceptance gate, test fixture, or critical-path dependency.

### Reduce Phase: Integration

The coordinator integrates worker outputs.

The coordinator must:

- Review worker evidence.
- Resolve integration conflicts.
- Remove duplicate or incompatible models.
- Ensure shared contracts are authoritative.
- Prune speculative code not required by the accepted workflow.
- Run required validation commands.
- Stage only files that belong to the verified batch.
- Commit one verified batch at a time.
- Append evidence, update statuses, and link commits in MCP.

Commits remain serialized even when implementation work is parallel.

### Verify Phase: Acceptance and Metrics

A MapReduce cycle is complete only when the integrated batch is verified.

Record:

- Accepted workflow or critical-path foundation completed.
- Validation commands and results.
- Files changed.
- Net lines added/deleted when available.
- Downstream items unblocked.
- Product progress score when available.
- Last commit SHA.
- Exact next action.

A cycle is not successful because it generated code. A cycle is successful only when it increases accepted Mayyam product capability.

Operating principle: Map the roadmap globally. Shuffle work safely through MCP. Implement in independent lanes. Reduce through one coordinator. Verify before commit. Checkpoint in MCP after every accepted product slice. Maximize accepted product capability per token, not code volume per hour.

## Roadmap Batch Loop Mode

Use this mode only when the user or one-shot prompt explicitly asks the agent to continue through multiple roadmap batches in a loop. Do not impose a discretionary batch-count limit.

- Continue selecting and executing deterministic batches until every roadmap row is processed and verified, or until a hard stop condition is reached.
- After each verified commit, update MCP evidence/status/commit links, then re-read MCP run health before selecting the next batch.
- Before starting each next batch, verify `git status --short`, MCP run status, active claim/lock state, last commit, current batch, and next action.
- Select, claim, implement, validate, commit, and checkpoint the next batch using the Roadmap MapReduce Execution Model and the same P0 -> P1 -> P2 priority rules.
- Do not stop merely because a batch completed, the next batch is larger, or the worktree is clean after a checkpoint.
- Stop the loop only when there are no pending rows, validation fails and cannot be fixed within the current batch, unrelated worktree changes create a conflict, a real blocker prevents progress, the user explicitly stops the run, or context/token/rate-limit/timeout pressure makes it impossible to continue safely.
- Treat context, token, rate-limit, and timeout pressure as hard stop conditions only when there is concrete evidence that continuing another batch would likely lose work or prevent a checkpoint. Do not stop speculatively.
- Before stopping, write durable MCP evidence/status with the current batch, feature IDs, changed files, commands run, verification state, last commit, blocker if any, and exact next action. If MCP is unavailable, stop and report that durable progress cannot be recorded.
- Codex cannot restart itself after API token, context, process, or network limits. If an external runtime reset is required, Aruvi tasktracker MCP is the handoff contract for the harness to relaunch Codex and continue from the exact next action.

## Parallel Batch Execution

Speed matters, but parallelism must not corrupt the worktree.

- Prefer 2-4 parallel lanes when the runtime supports subagents or background agents.
- Use Aruvi tasktracker MCP as the coordination source of truth. Every agent must atomically claim feature IDs before work begins.
- Parallelize only independent MapReduce lanes: different backend services, frontend surfaces, files, or analysis/test work that will not edit the same modules.
- Do not let two agents edit the same Rust module, route, controller, React file, migration, shared contract, or generated file at the same time.
- Keep one coordinator responsible for Map, Shuffle, Reduce, Verify, conflict checks, staging, commits, and checkpoint updates.
- Agents should return compact worker evidence: claimed feature IDs, batch ID, files changed, contracts or APIs changed, tests run, failures, product outcome, commit readiness, and exact next action.
- Run expensive validations in parallel only when they do not compete for the same build lock or mutate shared output. Otherwise serialize validation. The Rust `target/` build lock is shared within `backend/`.
- Commits must be serialized. One committed, verified batch at a time.
- If parallel lanes conflict, pause the lower-priority lane, write an MCP event/evidence record, and let the coordinator decide whether to rebase, merge manually, or requeue.
- If subagents are unavailable, simulate parallel roles sequentially but keep the same MCP claim/checkpoint protocol.

## Git Discipline

- Commit only when the user asks for a commit or the one-shot task explicitly includes committing in its definition of done.
- Before committing, run `git status --short` and review the intended diff.
- Use clear, specific commit messages.
- Leave the working tree clean after a commit when possible.

## Sandbox-Aware Git Handling

- Read-only git commands such as `git status`, `git diff`, `git log`, `git show`, and `git branch --show-current` should run normally in the sandbox.
- Mutating git commands such as `git add`, `git commit`, `git tag`, `git merge`, `git rebase`, `git cherry-pick`, `git stash`, and `git reset` write to `.git` and may require escalated permissions in restricted workspaces.
- If a required mutating git command fails with a sandbox-style error such as `Permission denied`, `Operation not permitted`, or `Unable to create .git/index.lock`, rerun only the necessary git command with escalated permissions and a concise justification.
- Do not work around git sandbox failures by copying the repository, manually editing `.git`, using `sudo`, or running destructive cleanup.
- Stage explicitly with `git add <path>...`, review `git status --short`, then commit with a clear message.

## Context Budget and Resume Discipline

For long roadmap execution runs, context pressure is expected. Checkpoint in Aruvi tasktracker MCP after each completed batch so an external runtime reset can resume exactly; do not voluntarily stop solely to get a clean context.

- After every committed batch, append MCP evidence, update item/batch/run status, link the commit, then select and claim the next deterministic batch unless a hard stop condition applies.
- Do not stop solely because the context is large, a batch boundary is clean, or a context compaction may occur. Stop starting new implementation work only when the remaining context is insufficient to complete, validate, and checkpoint the next batch safely, or the runtime is about to hit a hard token/rate/timeout limit.
- Before stopping for context pressure, write a complete MCP evidence/status handoff: current batch, feature IDs, changed files, commands run, verification state, last commit, blocker if any, and exact next action.
- Do not use permissions changes as a fix for token/context pressure. Permissions affect tool access, not context size.
- After a context reset, resume by reading this `AGENTS.md`, then MCP run health, active claims, latest evidence, and ready items. Verify last commit and `git status --short`, then continue from MCP `nextAction` or the active claimed item.
- After a context reset, use MCP only. If MCP context is invalid or unavailable, stop and report the blocker.
- Keep progress updates short. Put durable state in MCP evidence/status records, not in chat.

## Long-Running and Limit Behavior

Codex cannot restart itself after an API token, rate, context, process, or external timeout limit. The external harness must catch the limit response and requeue the task after the reset window.

If a token/rate limit, context interruption, or external timeout happens, write an MCP handoff before stopping:

- Objective.
- Completed work.
- Files changed.
- Commands run.
- Verification status.
- Current blocker.
- Exact next action.
- Resume instructions.

On resume, read MCP run health and latest evidence, then continue from the exact next action instead of restarting from scratch. If MCP is unavailable, stop and report the blocker.

## Final Response

Lead with the outcome. Include changed files, validation commands, commits, and remaining blockers. Keep the answer concise and factual.
