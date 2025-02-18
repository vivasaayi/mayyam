

Goal: 
1. I can select a list of global clusters and trigger failover
2. I can track the progress of failover
3. I want the AGGrid to display complext details


The table structure may look like below, but decide based on below requirements

Global Cluster      | Primary Region                   | No Secondary Regions | Secondary Region Names    | Actions 
Global Cluster Name | Primary region of the cluster    | 2                    | us-west-2, ap-southeast-2 | Failover Button, Failback Button


Task 1:
I want to build a react page where I can see the status of all global rds clusters for a region. You can use the RegionDropDown to show the list of regions.

Task 2:
Whenever I change the region, fetch the list of global clusters for that region and show it as an AGGrid.

Task 3: 
For Every Global RDS Cluster, display the number of secondary regions, and the region names.

For Every Global RDS Cluster, the table row should show how many instances are in primary and how manu are in the secondary.

Task 4:
For Evert row show two buttons, "Failover" and "Failback"

Task 5:
When failover button is clicked, it should failver to the secondary region

Task 6:
When failback button is clicked, it should failback to the secondary region

Task 7: There should be a visual indication for that row that the failvoer is in progress.

Task 8: Poll the server till teh failover is complet. Till that time show the progress bar.

Task 9: Even if I refresh tha page, detect that the cluster is failing over and display the status.

Task 10: The failover and failback events should be logged into a database, so that I can revisit the events later. The events should have details like the cluster name, failvoer start time, failover end time, duration, success ot failure.


Ensure you capture all the details. Do not remove exiting code unnecessariliy. 

My ReactApp is in Reactapp folder.

RDS controller is here: src/main/java/org/poriyiyal/mayyam/userinterface/web/controllers/aws/rds
-------------------