Context:
Instead of using a Chat interface to interact with LLMs, I want to build an UI and clickable experience to interact with the AI.

This is going to be an openionated analysis process. The idea is to make sure everyone in the org, use the same triaging flow, and get to the same results.

Let me explain a sample analysis workflow;

1. Simple workflow.
I have a page that lists RDS clusters. A single button is shown, "Analyze". When I click the analyze button, I am redirected to "RDS Analyis" page.

"RDS Analysis" page is nothing but an extension of the "Base Analysis" page, so that the experience is same for any resource.

In the "RDS Analysis" Page, I want to show a list of predefined workflows, like, "Analyze Memory Usage", "Analyze CPU Usage", "Analyze Disk", "Tell me why my cluster is slow" (which is configurable). 

When the user clicks on the desired flow, the related flow should be invoked, with the resource details. 

As part of the flow, I invoke a predefined process to gather data about the flow, then template it along with predefined LLM prompts, and post it to selected backend LLM. then get the results, and return back to the UI.  The response shuold also show related questions. These questions can be hard coded, or can be generated from LLMs reg the related topic

The response can be in the format of text, image, yaml, json, html etc. 

The UI should show the response, and then show related questions. 

When the user clicks the related question, invoke a flow (same as above), optionally asking user for the data.