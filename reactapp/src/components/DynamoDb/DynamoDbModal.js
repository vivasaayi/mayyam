import React, { useState, useEffect } from 'react';
import { CModal, CModalHeader, CModalTitle, CModalBody, CModalFooter, CButton, CForm, CFormLabel, CFormInput, CFormTextarea, CSpinner } from '@coreui/react';
import yaml from 'js-yaml';

const DynamoDbModal = ({ show, handleClose, handleCreate }) => {
  const [tableName, setTableName] = useState('');
  const [attributeDefinitions, setAttributeDefinitions] = useState([]);
  const [keySchema, setKeySchema] = useState([]);
  const [provisionedThroughput, setProvisionedThroughput] = useState({ readCapacityUnits: 5, writeCapacityUnits: 5 });
  const [yamlText, setYamlText] = useState('');
  const [jsonText, setJsonText] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    // Populate yamlText with a sample YAML data
    const sampleYaml = `
tableName: example-table
attributeDefinitions:
  - AttributeName: id
    AttributeType: S
keySchema:
  - AttributeName: id
    KeyType: HASH
provisionedThroughput:
  readCapacityUnits: 5
  writeCapacityUnits: 5
`;
    setYamlText(sampleYaml);
    setJsonText(JSON.stringify(yaml.load(sampleYaml), null, 2));
  }, []);

  const handleYamlChange = (e) => {
    const yamlValue = e.target.value;
    setYamlText(yamlValue);
    try {
      const jsonValue = yaml.load(yamlValue);
      setJsonText(JSON.stringify(jsonValue, null, 2));
      setTableName(jsonValue.tableName || '');
      setAttributeDefinitions(jsonValue.attributeDefinitions || []);
      setKeySchema(jsonValue.keySchema || []);
      setProvisionedThroughput(jsonValue.provisionedThroughput || { readCapacityUnits: 5, writeCapacityUnits: 5 });
    } catch (error) {
      setJsonText('Invalid YAML');
    }
  };

  const handleJsonChange = (e) => {
    const jsonValue = e.target.value;
    setJsonText(jsonValue);
    try {
      const yamlValue = yaml.dump(JSON.parse(jsonValue));
      setYamlText(yamlValue);
      const parsedJson = JSON.parse(jsonValue);
      setTableName(parsedJson.tableName || '');
      setAttributeDefinitions(parsedJson.attributeDefinitions || []);
      setKeySchema(parsedJson.keySchema || []);
      setProvisionedThroughput(parsedJson.provisionedThroughput || { readCapacityUnits: 5, writeCapacityUnits: 5 });
    } catch (error) {
      setYamlText('Invalid JSON');
    }
  };

  const handleSubmit = async (e) => {
    e.preventDefault();
    setLoading(true);
    await handleCreate(tableName, attributeDefinitions, keySchema, provisionedThroughput);
    setLoading(false);
  };

  return (
    <CModal visible={show} onClose={handleClose} size="lg">
      <CModalHeader closeButton>
        <CModalTitle>Create DynamoDB Table</CModalTitle>
      </CModalHeader>
      <CModalBody>
        <CForm onSubmit={handleSubmit}>
          <div className="mb-3">
            <CFormLabel htmlFor="yamlTextArea">YAML</CFormLabel>
            <CFormTextarea
              id="yamlTextArea"
              rows="10"
              value={yamlText}
              onChange={handleYamlChange}
            />
          </div>
          <div className="mb-3">
            <CFormLabel htmlFor="jsonTextArea">JSON</CFormLabel>
            <CFormTextarea
              id="jsonTextArea"
              rows="10"
              value={jsonText}
              onChange={handleJsonChange}
            />
          </div>
          <div className="mb-3">
            <CFormLabel htmlFor="tableName">Table Name</CFormLabel>
            <CFormInput
              type="text"
              id="tableName"
              value={tableName}
              onChange={(e) => setTableName(e.target.value)}
              required
            />
          </div>
          <div className="mb-3">
            <CFormLabel htmlFor="attributeDefinitions">Attribute Definitions (JSON)</CFormLabel>
            <CFormInput
              type="text"
              id="attributeDefinitions"
              value={JSON.stringify(attributeDefinitions)}
              onChange={(e) => setAttributeDefinitions(JSON.parse(e.target.value))}
              required
            />
          </div>
          <div className="mb-3">
            <CFormLabel htmlFor="keySchema">Key Schema (JSON)</CFormLabel>
            <CFormInput
              type="text"
              id="keySchema"
              value={JSON.stringify(keySchema)}
              onChange={(e) => setKeySchema(JSON.parse(e.target.value))}
              required
            />
          </div>
          <div className="mb-3">
            <CFormLabel htmlFor="provisionedThroughput">Provisioned Throughput (JSON)</CFormLabel>
            <CFormInput
              type="text"
              id="provisionedThroughput"
              value={JSON.stringify(provisionedThroughput)}
              onChange={(e) => setProvisionedThroughput(JSON.parse(e.target.value))}
              required
            />
          </div>
        </CForm>
      </CModalBody>
      <CModalFooter>
        {loading ? <CSpinner color="primary" /> : (
          <>
            <CButton color="primary" onClick={handleSubmit}>Create DynamoDB Table</CButton>{' '}
            <CButton color="secondary" onClick={handleClose}>Cancel</CButton>
          </>
        )}
      </CModalFooter>
    </CModal>
  );
};

export default DynamoDbModal;
