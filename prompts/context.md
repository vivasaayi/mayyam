Goal of this product:
I want to build a toolbox for SRE/DevOps/Performance Engineers/DBAs, with many different utilities. It can be run as a server or a cli.

There are many third party providers, but most of them are hosted on their own, and have serious limitations. I want to break that barrier and provide a single tool, and make the engineer to sit on the front seat. It is very very very openionated. I want to cover the complex 20% of the problem not the easy 80%.

I want to combine various cloud pillars, like, operational excellance, security, reliability, performance efficiency, cost optimization, sustainability, into ONE tool.

I want to include AI Capabilities at its core.

Core Capabilities:
1. First class integration with AI capabilities
2. I should be able to connect to different database types, like Postgres, MySQL, Redis, OpenSearch etc. Also at a given time I may be connecting to multiple databases/custers. I can perform a range of operations. I want to scan for issues, triage them, optimze queries, optmize cost etc.
3. I need to be able to connect to different kafka clusters, and perform cluster operations, toptic operations, consume and publish messages.
4. I should be able to interact with multiple cloud providers, like Aws and Azure, and leverage their control plane, data plane, and scrape data from their observability stack. I want the capability to connect to multiple accounts with differnt creds also consideting safety (need a robust implementation). This includes, creating, updating, scanning, triaging, optimizing resources.
5. I want to interact with multiple kubernetes clusters, and perform various operations.
6. I want to perform Chaos Experiments - there are many tools, but I want to provide an integrated and complete experience

Implementation - very high level
I want to implement it in Rust & React.

About the rust backend:
1. The backed is a rust server written in actix, also having the capability to be executed as a cli. 
2. I want to make sure the routes, controllers, services, and reposirotries, dals, network and other utilities are seperated and placed in appropriate directory strucure
3. I want to use seaorm, and build APIs on top of it. At the same time, I also want to provide the seagraph capabilities 
4. I wan to package Rust + react in a container.
5. Proper, standard, structured error handling across the board to make identifying errors easier
6. I want to have SAML integration, Token Based authenticaiton, Username and password authentication. I want to be able to manage users, permissions, so that I can ensure who is accessing what and not impacting the prod accidentally. Though this feature can be implemnted at a later phase/behind a feature flag so that an engineer can use this tool independently, I want to make this as a choice in the initial phase.


About the Front end:
Front end is a react app, written in javascript, using CoreUI as the UI Framework, and AGGrid community edition as the table renderer. Place the components in proper folders. 

1. I prefer double quotes
2. I dont prefer typescript
3. I want the Controller -> Services -> Repository pattern everywhere. 

--------------------------------
Feature: Kuberntes Dashboard

I want to be able to browse the Multiple Kuberntes Clusters.. Need the capability to select the available clusters

Given a cluster, collect all the kubernetes resources, like, pods, services, deployments, cronjobs, storage classes, PVs, PVCs and all other resources. 

Then in the UI, render reach resouce in a grid and using  AG Grid.

--------------------------------
AWS Cost Collector:
I shoule be able to collect AWS Cost, given a day, and mumber of AWS accoutns fetch the cost from AWS accounts, 

Then render in the UI React component

--------------------------------

EC2 and Redis Discovery:

List all EC2 instances and Redis clusters using AWS SDK.

Return results in structured JSON with instance name, ID, tags, region.

On-Demand Analysis Flow:

For any selected EC2 or Redis instance, fetch CloudWatch metrics for:

CPUUtilization, NetworkIn, NetworkOut, DiskReadOps, etc.

Fetch CloudWatch log group entries (e.g. system logs).

Export metrics to:

Local folder with path format: /exports/{service}/{instance_id}/YYYY/MM/DD/*.json

(Optional) Upload to S3 bucket with same folder structure.

Post all metric+log data to a configurable OpenAPI endpoint like /analyze/metrics.

Scheduled/Batch Analysis Flow:

Periodically scan CloudWatch metrics for all instances and auto-analyze.

Include config-based throttling and retry logic.

Configuration:

All options (S3 upload toggle, export folder, endpoint URL, metric time range) should be driven by a config file (TOML/YAML/JSON).

CLI & OpenAPI:

Expose a CLI command like analyze instance <instance-id> and analyze redis <cluster-id>

Expose OpenAPI endpoints to trigger the same flows