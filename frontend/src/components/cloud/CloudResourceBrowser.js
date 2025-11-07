import React, { useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { Row, Col, Form, FormGroup, Label, Input, Button, Table, Badge, Spinner, Alert, Modal, ModalHeader, ModalBody, ModalFooter } from 'reactstrap';
import ReactJson from 'react-json-view';
import ReactMarkdown from 'react-markdown';
import api, { analyzeAwsResource } from '../../services/api';

function useQuery() {
  const { search } = useLocation();
  return useMemo(() => new URLSearchParams(search), [search]);
}

const CloudResourceBrowser = () => {
  const query = useQuery();
  const navigate = useNavigate();
  const [provider, setProvider] = useState(query.get('provider') || 'aws');
  const [resourceType, setResourceType] = useState(query.get('resource_type') || '');
  const [syncId, setSyncId] = useState(query.get('sync_id') || '');
  const [region, setRegion] = useState(query.get('region') || '');
  const [accountId, setAccountId] = useState(query.get('account_id') || '');
  const [name, setName] = useState(query.get('name') || '');
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(10);
  const [data, setData] = useState({ resources: [], total: 0, total_pages: 0 });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [selected, setSelected] = useState(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [jsonOpen, setJsonOpen] = useState(false);
  const [analyzeOpen, setAnalyzeOpen] = useState(false);
  const [analysisWorkflow, setAnalysisWorkflow] = useState('cost');
  const [analysisLoading, setAnalysisLoading] = useState(false);
  const [analysisError, setAnalysisError] = useState(null);
  const [analysisResult, setAnalysisResult] = useState(null);

  const load = async (opts = {}) => {
    setLoading(true); setError(null);
    try {
      const params = {
        provider,
        resource_type: resourceType || undefined,
        sync_id: syncId || undefined,
        region: region || undefined,
        account_id: accountId || undefined,
        name: name || undefined,
        page,
        page_size: pageSize,
      };
      const res = await api.get('/api/cloud/resources', { params });
      setData(res.data);
    } catch (e) {
      setError(e.message || 'Failed to load resources');
    } finally { setLoading(false); }
  };

  useEffect(() => { load(); /* eslint-disable-next-line */ }, [provider, resourceType, syncId, region, accountId, name, page, pageSize]);

  const applyFilters = (e) => {
    e.preventDefault();
    const params = new URLSearchParams();
    if (provider) params.set('provider', provider);
    if (resourceType) params.set('resource_type', resourceType);
    if (syncId) params.set('sync_id', syncId);
    if (region) params.set('region', region);
    if (accountId) params.set('account_id', accountId);
    if (name) params.set('name', name);
    navigate({ search: params.toString() });
    setPage(0);
    load();
  };

  const clearFilters = () => {
    setProvider('aws'); setResourceType(''); setSyncId(''); setRegion(''); setAccountId(''); setName('');
    setPage(0);
    navigate({ search: '' });
    load();
  };

  const openDetails = (r) => { setSelected(r); setDetailsOpen(true); };
  const openJson = (r) => { setSelected(r); setJsonOpen(true); };
  const openAnalyze = (r) => { setSelected(r); setAnalyzeOpen(true); setAnalysisResult(null); setAnalysisError(null); };
  const closeAll = () => { setDetailsOpen(false); setJsonOpen(false); setAnalyzeOpen(false); };

  const runAnalysis = async () => {
    if (!selected) return;
    setAnalysisLoading(true); setAnalysisError(null); setAnalysisResult(null);
    try {
      if (selected.provider === 'aws') {
        const resourceId = selected.arn_or_uri || selected.resource_id;
        const res = await analyzeAwsResource(resourceId, analysisWorkflow);
        setAnalysisResult(res);
      } else {
        setAnalysisError('Analysis for non-AWS providers is not available yet.');
      }
    } catch (e) {
      setAnalysisError(e.message || 'Analysis failed');
    } finally {
      setAnalysisLoading(false);
    }
  };

  return (
    <div>
      {error && <Alert color="danger">{error}</Alert>}
      <Form onSubmit={applyFilters} className="mb-3">
        <Row className="g-2 align-items-end">
          <Col md="2">
            <FormGroup>
              <Label>Provider</Label>
              <Input type="select" value={provider} onChange={(e) => setProvider(e.target.value)}>
                <option value="aws">AWS</option>
                <option value="azure" disabled>Azure (soon)</option>
                <option value="gcp" disabled>GCP (soon)</option>
              </Input>
            </FormGroup>
          </Col>
          <Col md="2">
            <FormGroup>
              <Label>Type</Label>
              <Input placeholder="e.g., S3Bucket" value={resourceType} onChange={(e) => setResourceType(e.target.value)} />
            </FormGroup>
          </Col>
          <Col md="2">
            <FormGroup>
              <Label>Sync ID</Label>
              <Input placeholder="sync id" value={syncId} onChange={(e) => setSyncId(e.target.value)} />
            </FormGroup>
          </Col>
          <Col md="2">
            <FormGroup>
              <Label>Region</Label>
              <Input placeholder="region" value={region} onChange={(e) => setRegion(e.target.value)} />
            </FormGroup>
          </Col>
          <Col md="2">
            <FormGroup>
              <Label>Account/Project</Label>
              <Input placeholder="account id" value={accountId} onChange={(e) => setAccountId(e.target.value)} />
            </FormGroup>
          </Col>
          <Col md="2">
            <FormGroup>
              <Label>Name</Label>
              <Input placeholder="search by name" value={name} onChange={(e) => setName(e.target.value)} />
            </FormGroup>
          </Col>
          <Col md="12" className="d-flex gap-2">
            <Button color="primary" type="submit">Apply Filters</Button>
            <Button color="secondary" type="button" onClick={clearFilters}>Clear</Button>
          </Col>
        </Row>
      </Form>

      {loading ? (
        <div className="text-center p-4"><Spinner /></div>
      ) : (
        <div className="table-responsive">
          <Table striped hover>
            <thead>
              <tr>
                <th>Provider</th>
                <th>Type</th>
                <th>Name/ID</th>
                <th>Account</th>
                <th>Region</th>
                <th>ARN/URI</th>
                <th>Sync ID</th>
                <th>Last Updated</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {data.resources.map(r => (
                <tr key={r.id}>
                  <td><Badge color="dark">{r.provider}</Badge></td>
                  <td>{r.resource_type}</td>
                  <td>
                    <Button color="link" className="p-0" onClick={() => openDetails(r)}>
                      {r.name || r.resource_id}
                    </Button>
                  </td>
                  <td>{r.account_id}</td>
                  <td>{r.region}</td>
                  <td style={{maxWidth: 260}}>{r.arn_or_uri || '-'}</td>
                  <td style={{maxWidth: 220}}>{r.sync_id}</td>
                  <td>{new Date(r.updated_at).toLocaleString()}</td>
                  <td className="d-flex gap-2">
                    <Button size="sm" color="secondary" onClick={() => openDetails(r)}>Details</Button>
                    <Button size="sm" color="info" onClick={() => openJson(r)}>JSON</Button>
                    <Button size="sm" color="primary" onClick={() => openAnalyze(r)}>Analyze</Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </Table>
          <div className="d-flex justify-content-between align-items-center">
            <div>Showing {data.resources.length} of {data.total}</div>
            <div className="d-flex gap-2">
              <Button disabled={page===0} onClick={() => setPage(p => Math.max(0, p-1))}>Prev</Button>
              <Button disabled={page+1>=data.total_pages} onClick={() => setPage(p => p+1)}>Next</Button>
            </div>
          </div>
        </div>
      )}

      {/* Details Modal */}
      <Modal isOpen={detailsOpen} toggle={() => setDetailsOpen(v => !v)} size="lg">
        <ModalHeader toggle={() => setDetailsOpen(v => !v)}>Resource Details</ModalHeader>
        <ModalBody>
          {selected && (
            <div>
              <Row className="mb-2">
                <Col md="6"><strong>Provider:</strong> {selected.provider}</Col>
                <Col md="6"><strong>Type:</strong> {selected.resource_type}</Col>
              </Row>
              <Row className="mb-2">
                <Col md="6"><strong>Name:</strong> {selected.name || '-'}</Col>
                <Col md="6"><strong>Resource ID:</strong> {selected.resource_id}</Col>
              </Row>
              <Row className="mb-2">
                <Col md="6"><strong>Account:</strong> {selected.account_id}</Col>
                <Col md="6"><strong>Region:</strong> {selected.region}</Col>
              </Row>
              <Row className="mb-2">
                <Col md="12"><strong>ARN/URI:</strong> {selected.arn_or_uri || '-'}</Col>
              </Row>
              <Row className="mb-2">
                <Col md="6"><strong>Sync ID:</strong> {selected.sync_id}</Col>
                <Col md="6"><strong>Updated:</strong> {new Date(selected.updated_at).toLocaleString()}</Col>
              </Row>
              <hr />
              <h6 className="mt-3">Tags</h6>
              <div className="border rounded p-2">
                <ReactJson src={selected.tags || {}} name={false} collapsed={1} displayDataTypes={false} enableClipboard={true} />
              </div>
            </div>
          )}
        </ModalBody>
        <ModalFooter>
          <Button color="secondary" onClick={closeAll}>Close</Button>
        </ModalFooter>
      </Modal>

      {/* JSON Modal */}
      <Modal isOpen={jsonOpen} toggle={() => setJsonOpen(v => !v)} size="xl">
        <ModalHeader toggle={() => setJsonOpen(v => !v)}>Resource JSON</ModalHeader>
        <ModalBody>
          {selected && (
            <div className="border rounded p-2">
              <ReactJson src={selected.resource_data || {}} name={false} collapsed={2} displayDataTypes={false} enableClipboard={true} />
            </div>
          )}
        </ModalBody>
        <ModalFooter>
          <Button color="secondary" onClick={() => setJsonOpen(false)}>Close</Button>
        </ModalFooter>
      </Modal>

      {/* Analyze Modal */}
      <Modal isOpen={analyzeOpen} toggle={() => setAnalyzeOpen(v => !v)} size="lg">
        <ModalHeader toggle={() => setAnalyzeOpen(v => !v)}>Analyze Resource</ModalHeader>
        <ModalBody>
          {selected && (
            <>
              <p>
                <strong>{selected.provider.toUpperCase()}</strong> {selected.resource_type} — {selected.name || selected.resource_id}
              </p>
              <Form onSubmit={(e) => { e.preventDefault(); runAnalysis(); }}>
                <Row className="g-2 align-items-end">
                  <Col md="6">
                    <FormGroup>
                      <Label>Workflow</Label>
                      <Input type="select" value={analysisWorkflow} onChange={(e) => setAnalysisWorkflow(e.target.value)}>
                        <option value="cost">Cost</option>
                        <option value="security">Security</option>
                        <option value="performance">Performance</option>
                        <option value="resiliency">Resiliency</option>
                      </Input>
                    </FormGroup>
                  </Col>
                  <Col md="3">
                    <Button color="primary" type="submit" disabled={analysisLoading}>
                      {analysisLoading ? 'Analyzing…' : 'Run Analysis'}
                    </Button>
                  </Col>
                </Row>
              </Form>
              {analysisError && <Alert color="danger" className="mt-3">{analysisError}</Alert>}
              {analysisLoading && (
                <div className="text-center p-3"><Spinner /></div>
              )}
              {analysisResult && (
                <div className="mt-3">
                  {analysisResult.summary && (
                    <>
                      <h6>Summary</h6>
                      <ReactMarkdown>{analysisResult.summary}</ReactMarkdown>
                    </>
                  )}
                  {analysisResult.details && (
                    <>
                      <h6 className="mt-3">Details</h6>
                      <ReactMarkdown>{analysisResult.details}</ReactMarkdown>
                    </>
                  )}
                  {analysisResult.recommendations && analysisResult.recommendations.length > 0 && (
                    <>
                      <h6 className="mt-3">Recommendations</h6>
                      <ul>
                        {analysisResult.recommendations.map((r, i) => <li key={i}>{r}</li>)}
                      </ul>
                    </>
                  )}
                  {analysisResult && !analysisResult.summary && !analysisResult.details && (
                    <div className="border rounded p-2">
                      <ReactJson src={analysisResult} name={false} collapsed={1} displayDataTypes={false} enableClipboard={true} />
                    </div>
                  )}
                </div>
              )}
            </>
          )}
        </ModalBody>
        <ModalFooter>
          <Button color="secondary" onClick={() => setAnalyzeOpen(false)}>Close</Button>
        </ModalFooter>
      </Modal>
    </div>
  );
}

export default CloudResourceBrowser;
