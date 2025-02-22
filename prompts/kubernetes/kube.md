Goals: I want to implement a complex kubernetes dashboard

Task 1:
Create a react component in my reactapp. Add a link and ensure I can navigate to that page

Task 2: In the react component, I want these tabs, named "Deployments", "CronJobs", "Daemon Sets", "Stateful Sets", "PVCs", "PVs", "Storage Classes"

Task 3: 
Under the "Deployments" Tab, I want to add a AgGrid table, which lists all the kubernetes deployments. 

Task 4:
Under the "CronJobs" Tab, I want to add a AgGrid table, which lists all the kubernetes cron jobs. 

Task 5:
Under the "Daemon Sets" Tab, I want to add a AgGrid table, which lists all the kubernetes Daemon Sets. 

Task 6:
Under the "Stateful Sets" Tab, I want to add a AgGrid table, which lists all the kubernetes Stateful Sets. 

Task 7:
Under the "PVCs" Tab, I want to add a AgGrid table, which lists all the kubernetes PVCs. 

Task 7:
Under the "PVs" Tab, I want to add a AgGrid table, which lists all the kubernetes PVs.

Task 8:
Under the "Storage Classes" Tab, I want to add a AgGrid table, which lists all the kubernetes Storage Classes.

Task 9:
For all the above tasks, You need to add a method to my existing Kubernetes Controller, then add necessary logic to the existing Kubernetes Service.

Task 10:
For all the above AGGrids, Include ncessary fields like name, number of expected replicas, number of pods running, number of pods pending, number of pods not started. Also add another custom colum with a button, saying, "View Pod Details". 


Goal 2:
Task 1:
I also want to implement a new react page, "Kubernetes Pods". Add ncessary routes and link to the nav.

Task 2:
In the "Kubernetes Pods" page, I want to show an AgGrid, that loads all the pods from the kubernetes service. I should be able to to filter the pods, by name, status. If the pod is crashing, or pending, I want to see the status reflected. 

Goal 3:
Task 1:
I also want to implement a new react page, "Kubernetes Pod Details". Add ncessary routes and link to the nav.

Task 2: 
In that Kubernetes Pod Details poge, I want to load all the details about the pod. Implement two tabs, "Parsed Pod Details", "Raw JSON". The pod id and the namespace args will be passed as an URL query param. Also show a text box for the pod name, textbox for the namespace. Link the URL query params to these text box. The very first page load can be done automatic based on the query params. Also add a button so thet the users can change the pod name and namespace and load the data again


Task 3: 
Under the Parsed Pod Details tab, I want to parse the pod details and show them in proper UI Componenets. If the pod has multiple containers, I want each pod to be shown in a tab with following details. "Env Vars" from that pod should be parsed and shown in an AGGrid. "Attached Volumes" should be parsed and shown in an AGGrid. 


My React routes are here: reactapp/src/routes.js

My coreUI react menu is here: /Users/rajanpanneerselvam/work/mayyam/reactapp/src/_nav.js