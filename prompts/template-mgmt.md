vivasaayi
If you explore further, I should also be able to define the data sources using UI..

Ex,
resource DynamoDB
source: cloudwatch
metric name: XXXX
metric purpose/usage: XYV

resource Kubernernetes Prod
source: Dyantarce/Pod
metric name: XXXX
metric purpose/usage: XYV

I should be able to swap the LLMs.. I prefer local llms.. so we also need a way to manage the LLM settins..

We should also manage the promps externally, so we need a proper prompt editor..

GitHub Copilot
Summarized conversation history

Looking at your requirements, I can see you want to transform your current template-based system into a comprehensive LLM-powered analytics platform with configurable data sources, swappable LLMs, and external prompt management. Let me explore the current architecture and then create a roadmap for implementing these features.

Summarized conversation history

I'll help you transform the current template-based system into a comprehensive LLM-powered analytics platform with configurable data sources, swappable LLMs, and external prompt management. Let me start by exploring the current architecture and then implement the necessary components.