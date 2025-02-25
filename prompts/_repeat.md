There is no CFormGroup, CInput, CLabel in coreui.

For AGGrid, dont import the CSS. Deprecated in latest version.

My React routes are here: reactapp/src/routes.js

My coreUI react menu is here: /Users/rajanpanneerselvam/work/mayyam/reactapp/src/_nav.js

Below is the working example for tabs
<CTabs activeItemKey="profile">
      <CTabList variant="tabs">
        <CTab itemKey="home">Home</CTab>
        <CTab itemKey="profile">Profile</CTab>
        <CTab itemKey="contact">Contact</CTab>
        <CTab disabled itemKey="disabled">
          Disabled
        </CTab>
      </CTabList>
      <CTabContent>
        <CTabPanel className="p-3" itemKey="home">
          Home tab content
        </CTabPanel>
        <CTabPanel className="p-3" itemKey="profile">
          Profile tab content
        </CTabPanel>
        <CTabPanel className="p-3" itemKey="contact">
          Contact tab content
        </CTabPanel>
        <CTabPanel className="p-3" itemKey="disabled">
          Disabled tab content
        </CTabPanel>
      </CTabContent>
    </CTabs>


----
For all teh components in /Users/rajanpanneerselvam/work/mayyam/reactapp/src,

Can you ensure the AGGRids has,

1. Colum Filtering
2. Sorting
3. Choose Columns
4. Pagination

For AG Grid, the correct implementation is,
Remove the CSS
import 'ag-grid-community/styles/ag-grid.css';
import 'ag-grid-community/styles/ag-theme-alpine.css';

import { ClientSideRowModelModule, DateFilterModule, ModuleRegistry, NumberFilterModule, TextFilterModule, ValidationModule, } from "ag-grid-community";

ModuleRegistry.registerModules([ ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule ]);

Pass the modules to AGGrid