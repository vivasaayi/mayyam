Context: 
I want to analyze cloudwatch metrics using LLMs.

Goals:
1. I should be able to specify a single AWS Resource (like EC2 instance, redis cache, dynamodb table) and analyze metrics that are related to that resource
2. I should be able to specify a resource type and analyze all resources (like all EC2 instances, all dynamodb tabkes) and analyze the metrics that are related to the them. 
3. I want the analysis to be interactive. Instead of user doing the chat, I want the user to just click buttons


Implementation Notes:
1. From the implementation wise I want to build an efficient and resusable code that will work for one resource or a set of resources or a bulk set of resources
2. In the UI, I should be able to analyze a specific resource. For example, from an Redis list page, i shold be able to select a single redis cluster and start analyzing
3. In the UI, I shouldbe able to analyze all resources for a specific type. For example, from an Redis list page, I should be able to select multiple redis clusters and start analyzing
4. In UI, I should be able to select and analyze resources of a specific type. For example, I should be able to tell, analyze DynamoDB tables, and the server pulls the list of dynamodb tables and anlyze them.


Tasks1:
Implement an API, that will accept a resource id, resource type, list of metics, and optional timestamp. Then the API code should generate the correct cloudwatch query. Then call the cloudwatch API to get the result. Then store the result in proper files. Then return teh results to teh front end so that it can be rendered


The API code Send the cloudwatch metrics to OpenAi with related questions and get the analysus result, and return to the user