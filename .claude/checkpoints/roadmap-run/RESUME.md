# Roadmap Run Resume

- Run ID: run-001
- Roadmap hash: ab4059db94762a3e
- Last batch commit: 223d2ff (batch-145: Kafka offset inventory pillar reports)
- Current batch: none
- Current batch rows: none
- Current batch status: none
- Completed feature rows: 564 committed
- Current blocker: none
- Changed files in current batch: none
- Latest verification: batch-145 passed `CARGO_INCREMENTAL=0 cargo test --lib offset_inventory --message-format short`, `CARGO_INCREMENTAL=0 cargo test -q --features integration-tests --test integration_tests kafka_cluster_inventory_pillar_reports_contract`, `cargo fmt -- --check`, `git diff --check`, and `CARGO_INCREMENTAL=0 cargo check --lib --message-format short`.
- Exact next action: select-batch-146.
- Verification before continuing: `runs.last_commit=223d2ff`, `runs.current_batch_id` is null, `runs.next_action=select-batch-146`, and batch-145 rows are committed.
- Known pre-existing issue: `cargo test --test unit_tests` has failures in `aws_account_service_test`; do not chase unless scoped.
