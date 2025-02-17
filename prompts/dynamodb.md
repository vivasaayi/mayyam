Based on the above conversations, can you do the same for dynamodb?

Remeber to include all the details.

1. My React App Lives here: reactapp
2. My current dynamodb service is here: /Users/rajanpanneerselvam/work/mayyam/src/main/java/org/poriyiyal/mayyam/cloud/aws/controlplane. Please make sure you are not removing exisiting code, just updat them,
3. Controller should be here: /Users/rajanpanneerselvam/work/mayyam/src/main/java/org/poriyiyal/mayyam/userinterface/web/controllers/aws/dynamodb
4. Ensure beans are annotated and autowired

For create dynamodb modal, refer the input parameters from here: aws dynamodb create-table 


----------------
Goal: I want to find out which tables dont have global replication

I want to add a "Get Tables without Replication" button. When I click that button, a new page should open in a new browser tab.

The new page should have two tabs.

One tab, called "Regional Tables" will load an AGGRid with a list of DynamoDB table that dont have Global REplication. 

The other tab, called "Global Tables", will load an AGGrid with a list of tables that have a secondary region.

I also want the capability to export these two tables independany as a csv file with button clicks.

My current dynamodb service is here: /Users/rajanpanneerselvam/work/mayyam/src/main/java/org/poriyiyal/mayyam/cloud/aws/controlplane. Please make sure you are not removing exisiting code, just updat them,
Controller is here: /Users/rajanpanneerselvam/work/mayyam/src/main/java/org/poriyiyal/mayyam/userinterface/web/controllers/aws/dynamodb
Ensure beans are annotated and autowired

Write all the backend code.