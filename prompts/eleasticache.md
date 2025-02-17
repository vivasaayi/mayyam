I want to expose these methods as API. The APIs shoule be created in this location with proper namespace src/main/java/org/poriyiyal/mayyam/userinterface/web/controllers/aws/elasticache. The APIs should use proper or enhanced error handling.

I also want to create react comonents to consume these APIs and render UI. If the response is a collection then render it as an AGGridTable. I should be able to navigate to these componenets. Where applicable build a master child view. 

My React routes are here: reactapp/src/routes.js

My coreUI react menu is here: /Users/rajanpanneerselvam/work/mayyam/reactapp/src/_nav.js


--- 
My goal is add a "Create Elasticache" button to this page. When clicking that button, a modal should be shown, which collects necessary data. I want to build it using the CoreUI library.

Refer "aws elasticache create-cache-cluster"cli command. I also want the same parameters.

To break it down further,

Task 1:
In this react component, I want an add "Create Elasticache" button. When I click this button, it should show a modal. That modal should cover the entire screen. The modal should have two buttons called "Create Elasticache" and "Cancel".

Task 2:
In that modal, I want to have a testarea, say yamlTextArea, where I can paste yaml content. I am expecting the user to paste properties required to create an elasticache cluster as a yaml file. 

Task 3:
To give users an immediate feedback, whenever an user enters/pastes yaml data, it should be parsed and converted to a JSON object. You can actually create one more text area, say jsonTextArea, to show this JSON. 

Task 4: 
You can actually place the yamlTextArea and jsonTextArea side by side, so that whenever the user pastes the yaml file, he can see the parsed JSON.

Task 5:
Below the above controls, I want to display editable form showing the above properties. 


Remember, this yaml will have details to create react clusters. So the pased JSON object is also having the elasticahe details. The form which you build should show Elasticache related data. 


Task 6:
The user may miss to specify properties in the yaml file. In that case, the form should say that field is missing.

Task 7:
Validate the data when user pastes the data. Then if the user clicks "Create Elasticache", call the redis api that we have created and create the cluster.

Task 8:
The user many not be aware of the yaml structure. So populate the yamlArea with a sample yaml data.

Ensure you include all the parameters for the "aws elasticache create-cache-cluster"


---- Created the code. After testing, I am adding below enhancements.

Another idea, when the user changes the form controls, can we add the capability to regenerate the yaml file and update both the yamlare and jsontextarea?