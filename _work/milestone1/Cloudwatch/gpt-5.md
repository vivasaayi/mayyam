## Detailed Analysis and Action Plan for AWS Kinesis Integration Tests

In this section, we will outline the deviations observed during the end-to-end AWS Kinesis integration tests and provide a step-by-step remediation plan.

### Deviations Observed
1. **Data Latency**: There were instances where data latency exceeded acceptable thresholds.
2. **Data Loss**: Some records were not processed, leading to data loss.
3. **Throughput Issues**: The throughput was lower than expected during peak loads.

### Step-by-Step Remediation Plan
1. **Monitor Data Latency**: Implement CloudWatch alarms to monitor data latency and set thresholds for alerts.
2. **Implement Retry Logic**: Ensure that the application has retry logic for failed records to prevent data loss.
3. **Optimize Kinesis Streams**: Review and optimize the shard configuration to handle increased throughput.
4. **Load Testing**: Conduct load testing to simulate peak conditions and identify bottlenecks.

By following this action plan, we can address the deviations and improve the reliability of the AWS Kinesis integration tests.

### Repo Findings (file pointers)
- Real CloudWatch/LLM Kinesis analyzer: `backend/src/services/analytics/cloudwatch_analytics/kinesis_analyzer.rs`
- Metrics-based Kinesis analyzer (fallback): `backend/src/services/analytics/aws_analytics/resources/kinesis.rs`
- Router that selects analyzer: `backend/src/services/analytics/aws_analytics/aws_analytics.rs`
- Frontend UI for bulk analysis: `frontend/src/pages/KinesisAnalysis.js`
- Integration test scaffolding and scripts:
	- Test runner script: `run_real_aws_tests.sh` and `run_secure_aws_tests.sh`
	- Unit & API tests: `backend/src/tests/kinesis_unit_tests.rs`, `backend/src/tests/kinesis_api_tests.rs`
	- Demo DB insert script: `add_sample_kinesis_streams.sh`

### Detected Deviations (concrete examples)
1. Tests that reference mocked AWS behavior or mock clusters in planning docs and code (search 'mock' across `_work` and `tests` directories).
2. Some helpers and test runners exist that intend to run against real AWS (`run_real_aws_tests.sh`) but the unit/api tests still include code paths that stub or short-circuit AWS interactions.
3. Demo scripts insert rows directly into `aws_resources` instead of actually creating the Kinesis stream via AWS SDK or AWS CLI (see `add_sample_kinesis_streams.sh`).
4. Test auth flow: API tests should exercise `POST /api/auth/login` and use returned JWTs; current tests sometimes bypass auth or assume valid tokens (verify `backend/src/tests/*` for login usage).

### Remediation Checklist (detailed next steps)
1. Make a dedicated integration test feature flag (if not already) and gate real-AWS tests behind it (`--features integration-tests-real-aws`).
2. Modify integration tests to create/delete Kinesis streams using the official AWS SDK for Rust (rusoto/aws-sdk-rust) or by invoking the backend API endpoints which use configured AWS credentials. Prefer calling the backend API endpoints to validate end-to-end HTTP + auth.
3. Ensure tests read AWS credentials from environment variables or use named profiles (support `AWS_PROFILE`, `AWS_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`). Provide a `run_real_aws_tests.sh` wrapper that validates credentials before running.
4. Update `backend/src/tests/kinesis_api_tests.rs` to perform a real sign-in flow (`POST /api/auth/login`) with admin/admin123, capture the token, and use it for subsequent Create/Describe/Delete API calls.
5. Replace direct DB inserts in test/demo scripts with API calls that create resources (so the app's resource provisioning path is exercised).
6. Add cleanup and retry logic: tests must delete created Kinesis streams and handle eventual consistency (use backoff/retries for describe calls).
7. Add assertions that check the AWS Console/Describe API responses for correct attributes (shard count, stream status = ACTIVE after creation, etc.).
8. Optionally, add a tagging convention for test-created resources so they're easy to find and tear down (e.g., tag TestRun=<UUID>).

### Quick Next Steps I can take now (pick one)
1. Update `backend/src/tests/kinesis_api_tests.rs` to use the real login flow and call the public API for create/describe/delete, adding retries and cleanup. Then run a targeted test.
2. Implement a small helper `tests/integration/aws_helpers.rs` that wraps AWS SDK calls and cleanup utilities; then wire tests to use it.
3. Update `add_sample_kinesis_streams.sh` to actually call `aws kinesis create-stream` (or call the backend API) instead of inserting into DB, and make it safe (idempotent and tagged).

If you tell me which to do first, I'll implement it and run the test locally.
