import React, { useState, useEffect } from 'react';
import { CModal, CModalHeader, CModalTitle, CModalBody, CModalFooter, CButton, CForm, CFormLabel, CFormInput, CFormTextarea, CSpinner } from '@coreui/react';
import yaml from 'js-yaml';

const KinesisModal = ({ show, handleClose, handleCreate }) => {
  const [streamName, setStreamName] = useState('');
  const [shardCount, setShardCount] = useState(1);
  const [yamlText, setYamlText] = useState('');
  const [jsonText, setJsonText] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    // Populate yamlText with a sample YAML data
    const sampleYaml = `
streamName: example-stream
shardCount: 1
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
      setStreamName(jsonValue.streamName || '');
      setShardCount(jsonValue.shardCount || 1);
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
      setStreamName(parsedJson.streamName || '');
      setShardCount(parsedJson.shardCount || 1);
    } catch (error) {
      setYamlText('Invalid JSON');
    }
  };

  const handleStreamNameChange = (e) => {
    setStreamName(e.target.value);
    const updatedJson = { ...JSON.parse(jsonText), streamName: e.target.value };
    setJsonText(JSON.stringify(updatedJson, null, 2));
    setYamlText(yaml.dump(updatedJson));
  };

  const handleShardCountChange = (e) => {
    setShardCount(e.target.value);
    const updatedJson = { ...JSON.parse(jsonText), shardCount: e.target.value };
    setJsonText(JSON.stringify(updatedJson, null, 2));
    setYamlText(yaml.dump(updatedJson));
  };

  const handleSubmit = async (e) => {
    e.preventDefault();
    setLoading(true);
    await handleCreate(streamName, shardCount);
    setLoading(false);
  };

  return (
    <CModal visible={show} onClose={handleClose} size="lg">
      <CModalHeader closeButton>
        <CModalTitle>Create Kinesis Stream</CModalTitle>
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
            <CFormLabel htmlFor="streamName">Stream Name</CFormLabel>
            <CFormInput
              type="text"
              id="streamName"
              value={streamName}
              onChange={handleStreamNameChange}
              required
            />
          </div>
          <div className="mb-3">
            <CFormLabel htmlFor="shardCount">Shard Count</CFormLabel>
            <CFormInput
              type="number"
              id="shardCount"
              value={shardCount}
              onChange={handleShardCountChange}
              required
              min="1"
            />
          </div>
        </CForm>
      </CModalBody>
      <CModalFooter>
        {loading ? <CSpinner color="primary" /> : (
          <>
            <CButton color="primary" onClick={handleSubmit}>Create Kinesis Stream</CButton>{' '}
            <CButton color="secondary" onClick={handleClose}>Cancel</CButton>
          </>
        )}
      </CModalFooter>
    </CModal>
  );
};

export default KinesisModal;
