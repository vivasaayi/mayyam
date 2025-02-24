import React, { useEffect, useState } from 'react';
import { CButton, CFormSelect } from '@coreui/react';
import { AgGridReact } from 'ag-grid-react';
import { ClientSideRowModelModule, DateFilterModule, ModuleRegistry, NumberFilterModule, TextFilterModule, ValidationModule } from 'ag-grid-community';
import axios from 'axios';

ModuleRegistry.registerModules([ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule]);

const SearchDomainTab = () => {
  const [namespaces, setNamespaces] = useState([]);
  const [selectedNamespace, setSelectedNamespace] = useState('');
  const [searchDomainData, setSearchDomainData] = useState([]);
  const [error, setError] = useState(null);

  useEffect(() => {
    fetchNamespaces();
  }, []);

  useEffect(() => {
    if (selectedNamespace) {
      fetchSearchDomain();
    }
  }, [selectedNamespace]);

  const fetchNamespaces = async () => {
    try {
      const response = await axios.get('/api/kubernetes/namespaces');
      setNamespaces(response.data);
      setSelectedNamespace(response.data[0]);
    } catch (error) {
      console.error('Error fetching namespaces:', error);
      setError('Error fetching namespaces');
    }
  };

  const fetchSearchDomain = async () => {
    try {
      const response = await axios.get(`/api/kubernetes/checkSearchDomain?namespace=${selectedNamespace}&searchDomain=default.svc.cluster.local`);
      setSearchDomainData(response.data);
      setError(null);
    } catch (error) {
      console.error('Error fetching search domain:', error);
      setError('Error fetching search domain');
    }
  };

  return (
    <div>
      <div className="form-group">
        <label htmlFor="namespace">Namespace</label>
        <CFormSelect id="namespace" value={selectedNamespace} onChange={(e) => setSelectedNamespace(e.target.value)}>
          {namespaces.map((namespace) => (
            <option key={namespace} value={namespace}>
              {namespace}
            </option>
          ))}
        </CFormSelect>
      </div>
      <CButton color="primary" onClick={fetchSearchDomain}>Load</CButton>
      {error && <div className="alert alert-danger">{error}</div>}
      <div className="ag-theme-balham" style={{ height: 400, width: '100%', marginTop: '10px' }}>
        <AgGridReact
          rowData={searchDomainData}
          columnDefs={[
            { headerName: 'Pod Name', field: 'podName' },
            { headerName: 'Current Search Domain', field: 'currentSearchDomain' },
          ]}
          defaultColDef={{ flex: 1, minWidth: 100, sortable: true, filter: true, resizable: true }}
          pagination={true}
          paginationPageSize={10}
          sideBar={{ toolPanels: ['columns', 'filters'] }}
          modules={[ClientSideRowModelModule, TextFilterModule, NumberFilterModule, DateFilterModule, ValidationModule]}
        />
      </div>
    </div>
  );
};

export default SearchDomainTab;
