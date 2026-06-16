# Roadmap Run Resume

- Run ID: run-001
- Roadmap hash: ab4059db94762a3e
- Last batch commit: 038cf88 (batch-184 Kafka managed Kafka services and batch-185 Postgres pg_stat_statements inventory pillar reports)
- Current batch: none
- Current batch rows: none
- Current batch status: none
- Completed feature rows: 684 committed
- Current blocker: none
- Changed files in current batch: none
- Latest verification: batch-184 and batch-185 passed `CARGO_INCREMENTAL=0 cargo test --lib managed_service_inventory --message-format short`, `CARGO_INCREMENTAL=0 cargo test --lib pg_stat_statements_inventory --message-format short`, `CARGO_INCREMENTAL=0 cargo test --test integration_tests kafka_cluster_inventory_pillar_reports_contract --features integration-tests --message-format short`, `CARGO_INCREMENTAL=0 cargo test --test integration_tests postgres_pg_stat_statements_inventory_pillar_reports_contract --features integration-tests --message-format short`, `cargo fmt -- --check`, `git diff --check`, and `CARGO_INCREMENTAL=0 cargo check --lib --message-format short`.
- Exact next action: select-batch-186.
- Verification before continuing: `runs.last_commit=038cf88`, `runs.current_batch_id` is null, `runs.next_action=select-batch-186`, and batch-184/batch-185 rows are committed.
- Known pre-existing issue: `cargo test --test unit_tests` has failures in `aws_account_service_test`; do not chase unless scoped.
