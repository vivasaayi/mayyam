I want to fetch metrics from cloudwatch and analyze them using ChatGPT or DeepSeek or LocalLLm.

General use cases:
1. Fetch list of AWS resources
2. Collect necessary data from cloudwatch for each resource
3. Construct necessary prompts to LLM to analyze the data
4. Present the analysys results to the user (a simple html file)

Kinesis:
1. Find unused kinesis streams
   - Definition: The LLM will analyze and label if a stream is unused (no data throughput: IncomingBytes or OutgoingBytes) in each of the following periods: last 6 hours, 1 day, 3 days, 7 days, 2 weeks, 1 month, or 2 months.
2. Classify streams based on usage. On a scale of 1-10
   - Criteria: Based on average throughput vs. provisioned shards. 1 = very low usage (e.g., <10% of shard capacity), 10 = high usage (e.g., >90% of shard capacity).
3. Detect usage patterns
   - Examples: Seasonal spikes (e.g., higher throughput during business hours), steady low usage (consistent low activity).
4. Get suggestions for scaling down the provisioned shards

SQS:
1. Find unused SQS Queues
   - Definition: The LLM will analyze and label if a queue is unused (no messages processed: NumberOfMessagesSent = 0) in each of the following periods: last 6 hours, 1 day, 3 days, 7 days, 2 weeks, 1 month, or 2 months.
2. Classify Queues based on usage. On a scale of 1-10
   - Criteria: Based on average message count and processing rate. 1 = very low usage (e.g., <5 messages/day), 10 = high usage (e.g., >1000 messages/minute).
3. Detect usage patterns
   - Examples: Burst patterns (sudden spikes in messages), consistent high load (steady processing throughout the day).

RDS:
1. Find unused RDS instances
   - Definition: The LLM will analyze and label if an instance is unused (no active connections: DatabaseConnections = 0 or very low CPU utilization: CPUUtilization < 5%) in each of the following periods: last 6 hours, 1 day, 3 days, 7 days, 2 weeks, 1 month, or 2 months.
2. Classify instances based on usage. On a scale of 1-10
   - Criteria: Based on average CPU utilization and database connections. 1 = very low usage (e.g., CPU < 10% and connections < 5), 10 = high usage (e.g., CPU > 80% and connections > 100).
3. Detect usage patterns.
   - Examples: Read-heavy patterns (ReadIOPS > 1000), write-heavy patterns (WriteIOPS > 500), idle periods (CPUUtilization < 10% and DatabaseConnections < 5), peak usage (sudden spikes in CPU or connections during specific hours).
4. Get suggestions for scaling down or optimizing the instance.
