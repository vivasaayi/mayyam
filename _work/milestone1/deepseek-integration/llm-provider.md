I reverted the changes

Whan I add a provider and save it, then load it, I see diifferent values loaded.. Can you fix that? Also can you add integration test for that?

Every provider can have multuple models, currnetly the UI allows only one model.. We need to improve the UI, 

Lets seperate the LLM provider management from manageing the models provided by them.

We can show an AGGrid to add the LLM providers.. To add the LLM, lets show a modal. then to edit the LLM provider, lets redirect the user to a new screen, where we present the LLM provider details to edit, and provide a AGGrid to show the model details.. The user can add or edit models.. As well as provide opotions to manage any details for that model

The user may choose to enable or disable a model. So we need to provide explicit options and visually indicate.

Also provide a quick option to test the modal to make sure its integrated correctly - we can simple query the current weather events.

----
place the integration tests here: /Users/rajanpanneerselvam/work/mayyam-theta/backend/tests/integration/llm

Refer the existing tests (/Users/rajanpanneerselvam/work/mayyam-theta/backend/tests/integration) and follow the same pattern for auth & code reuse

The integration test will call the backend to add few providers, then edit the providers, retrived the edited providers and confirm they are saved correctly and then delete the providers?

Also for each privider, add few models, edit them and save and delete them

for each model, test the modal to validate it is working as expected

for each model, test the enable and disable..