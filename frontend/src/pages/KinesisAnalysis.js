// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


import React, { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { 
  Card, CardHeader, CardBody, Button, Alert, Spinner, Row, Col, Badge,
  Form, FormGroup, Label, Input, ButtonGroup, UncontrolledTooltip
} from "reactstrap";
import { fetchWithAuth, analyzeAwsResource } from "../services/api";
import PageHeader from "../components/layout/PageHeader";

const KinesisAnalysis = () => {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [kinesisStreams, setKinesisStreams] = useState([]);
  const [selectedStream, setSelectedStream] = useState(null);
  const [analysisResult, setAnalysisResult] = useState(null);
  const [selectedWorkflow, setSelectedWorkflow] = useState('performance');
  const [analysisMode, setAnalysisMode] = useState('auto'); // auto, llm, metrics
  const [bulkAnalysisResults, setBulkAnalysisResults] = useState([]);
  const [bulkAnalysisProgress, setBulkAnalysisProgress] = useState({ current: 0, total: 0 });
  const [isBulkAnalyzing, setIsBulkAnalyzing] = useState(false);

  const workflows = [
    {
      id: 'performance',
      name: 'Performance Analysis', 
      description: 'Analyze consumer lag, throughput, and throttling',
      icon: 'tachometer-alt',
      color: 'primary'
    },
    {
      id: 'cost',
      name: 'Cost Analysis',
      description: 'Calculate costs and optimization recommendations',
      icon: 'dollar-sign',
      color: 'success'
    },
    {
      id: 'unused',
      name: 'Unused Detection',
      description: 'Identify idle streams across time periods (LLM)',
      icon: 'pause-circle',
      color: 'warning',
      requiresLLM: true
    },
    {
      id: 'classification',
      name: 'Usage Classification',
      description: 'Score usage 1-10 with scaling recommendations (LLM)',
      icon: 'chart-bar',
      color: 'info',
      requiresLLM: true
    },
    {
      id: 'patterns',
      name: 'Usage Patterns',
      description: 'Detect patterns using LLM analysis (LLM)',
      icon: 'chart-line',
      color: 'secondary',
      requiresLLM: true
    },
    {
      id: 'scaling',
      name: 'Scaling Analysis',
      description: 'Data-driven shard scaling recommendations (LLM)',
      icon: 'expand-arrows-alt',
      color: 'dark',
      requiresLLM: true
    }
  ];

  // Fetch Kinesis streams on component mount
  useEffect(() => {
    fetchKinesisStreams();
  }, []);

  const fetchKinesisStreams = async () => {
    try {
      setLoading(true);
      const response = await fetchWithAuth('/api/aws/resources?resource_type=KinesisStream&page_size=20');
      if (response.ok) {
        const data = await response.json();
        setKinesisStreams(data.resources || []);
        if (data.resources && data.resources.length > 0) {
          setSelectedStream(data.resources[0]);
        }
      } else {
        throw new Error('Failed to fetch Kinesis streams');
      }
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  const runAnalysis = async () => {
    if (!selectedStream) {
      setError('Please select a Kinesis stream to analyze');
      return;
    }

    try {
      setLoading(true);
      setError(null);
      setAnalysisResult(null);

      console.log(`Running ${selectedWorkflow} analysis for stream: ${selectedStream.resource_id}`);
      
      const result = await analyzeAwsResource(
        selectedStream.arn,
        selectedWorkflow,
        'last_7_days'
      );
      
      console.log('Analysis result:', result);
      setAnalysisResult(result);
    } catch (err) {
      console.error('Analysis failed:', err);
      setError('Analysis failed: ' + (err.response?.data?.message || err.message));
    } finally {
      setLoading(false);
    }
  };

  const runBulkAnalysis = async () => {
    if (!kinesisStreams || kinesisStreams.length === 0) {
      setError('No Kinesis streams available for bulk analysis');
      return;
    }

    try {
      setIsBulkAnalyzing(true);
      setError(null);
      setBulkAnalysisResults([]);
      setBulkAnalysisProgress({ current: 0, total: kinesisStreams.length });

      console.log(`Starting bulk analysis for ${kinesisStreams.length} streams with workflow: ${selectedWorkflow}`);
      
      const results = [];
      
      for (let i = 0; i < kinesisStreams.length; i++) {
        const stream = kinesisStreams[i];
        setBulkAnalysisProgress({ current: i + 1, total: kinesisStreams.length });
        
        try {
          console.log(`Analyzing stream ${i + 1}/${kinesisStreams.length}: ${stream.resource_id}`);
          
          const result = await analyzeAwsResource(
            stream.arn,
            selectedWorkflow,
            'last_7_days'
          );
          
          results.push({
            stream: stream,
            result: result,
            status: 'success',
            timestamp: new Date()
          });
          
        } catch (err) {
          console.error(`Failed to analyze stream ${stream.resource_id}:`, err);
          results.push({
            stream: stream,
            error: err.response?.data?.message || err.message,
            status: 'error',
            timestamp: new Date()
          });
        }
        
        // Update results after each analysis
        setBulkAnalysisResults([...results]);
        
        // Small delay to avoid overwhelming the backend
        if (i < kinesisStreams.length - 1) {
          await new Promise(resolve => setTimeout(resolve, 500));
        }
      }
      
      console.log(`Bulk analysis completed. ${results.filter(r => r.status === 'success').length} successful, ${results.filter(r => r.status === 'error').length} failed`);
      
    } catch (err) {
      console.error('Bulk analysis failed:', err);
      setError('Bulk analysis failed: ' + err.message);
    } finally {
      setIsBulkAnalyzing(false);
    }
  };

  const downloadBulkResults = () => {
    if (bulkAnalysisResults.length === 0) return;
    
    const csvContent = [
      // CSV Header
      ['Stream Name', 'Region', 'Account', 'Status', 'Analysis Summary', 'Timestamp'].join(','),
      // CSV Rows
      ...bulkAnalysisResults.map(result => {
        const streamName = result.stream.name || result.stream.resource_id;
        const region = result.stream.region;
        const account = result.stream.account_id;
        const status = result.status;
        const summary = result.status === 'success' 
          ? (result.result.content || '').replace(/[\r\n]+/g, ' ').replace(/,/g, ';').substring(0, 200) + '...'
          : result.error || 'Unknown error';
        const timestamp = result.timestamp.toISOString();
        
        return [streamName, region, account, status, `"${summary}"`, timestamp].join(',');
      })
    ].join('\n');
    
    const blob = new Blob([csvContent], { type: 'text/csv' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `kinesis-bulk-analysis-${selectedWorkflow}-${new Date().toISOString().split('T')[0]}.csv`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    window.URL.revokeObjectURL(url);
  };

  const clearBulkResults = () => {
    setBulkAnalysisResults([]);
    setBulkAnalysisProgress({ current: 0, total: 0 });
  };

  const getAnalysisModeInfo = () => {
    switch (analysisMode) {
      case 'llm':
        return {
          title: 'LLM-Powered Analysis',
          description: 'Uses advanced AI for pattern recognition and intelligent recommendations',
          color: 'info',
          icon: 'brain'
        };
      case 'metrics':
        return {
          title: 'Metrics-Based Analysis',
          description: 'Direct CloudWatch metrics analysis for performance and cost insights',
          color: 'primary',
          icon: 'chart-line'
        };
      default:
        return {
          title: 'Auto-Select Analysis',
          description: 'Automatically chooses the best analysis method based on availability',
          color: 'success',
          icon: 'magic'
        };
    }
  };

  const renderAnalysisResult = () => {
    if (!analysisResult) return null;

    return (
      <Card className="mt-4">
        <CardHeader>
          <h5 className="mb-0">
            <i className="fas fa-chart-area me-2"></i>
            Analysis Results - {workflows.find(w => w.id === selectedWorkflow)?.name}
          </h5>
        </CardHeader>
        <CardBody>
          <div 
            className="analysis-content"
            dangerouslySetInnerHTML={{
              __html: analysisResult.content?.replace(/\n/g, '<br>') || 'No content available'
            }}
          />
          
          {analysisResult.related_questions && analysisResult.related_questions.length > 0 && (
            <div className="mt-4">
              <h6>Related Questions:</h6>
              <ul className="list-unstyled">
                {analysisResult.related_questions.map((question, index) => (
                  <li key={index} className="mb-2">
                    <Badge color="outline-secondary" className="me-2">Q{index + 1}</Badge>
                    {question}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {analysisResult.metadata && (
            <div className="mt-4 pt-3 border-top">
              <small className="text-muted">
                <i className="fas fa-info-circle me-1"></i>
                Analysis completed at {new Date(analysisResult.metadata.timestamp).toLocaleString()}
                {analysisResult.metadata.data_sources && (
                  <span> • Data sources: {analysisResult.metadata.data_sources.join(', ')}</span>
                )}
              </small>
            </div>
          )}
        </CardBody>
      </Card>
    );
  };

  const renderBulkAnalysisResults = () => {
    if (bulkAnalysisResults.length === 0 && !isBulkAnalyzing) return null;

    return (
      <Card className="mt-4">
        <CardHeader>
          <div className="d-flex justify-content-between align-items-center">
            <h5 className="mb-0">
              <i className="fas fa-list-alt me-2"></i>
              Bulk Analysis Results - {workflows.find(w => w.id === selectedWorkflow)?.name}
            </h5>
            <div>
              {bulkAnalysisResults.length > 0 && (
                <>
                  <Button
                    color="outline-primary"
                    size="sm"
                    onClick={downloadBulkResults}
                    className="me-2"
                  >
                    <i className="fas fa-download me-1"></i>
                    Download CSV
                  </Button>
                  <Button
                    color="outline-secondary"
                    size="sm"
                    onClick={clearBulkResults}
                  >
                    <i className="fas fa-trash me-1"></i>
                    Clear
                  </Button>
                </>
              )}
            </div>
          </div>
        </CardHeader>
        <CardBody>
          {isBulkAnalyzing && (
            <div className="mb-4">
              <div className="d-flex justify-content-between align-items-center mb-2">
                <span>Analyzing streams...</span>
                <span>{bulkAnalysisProgress.current} of {bulkAnalysisProgress.total}</span>
              </div>
              <div className="progress">
                <div 
                  className="progress-bar" 
                  role="progressbar" 
                  style={{ width: `${(bulkAnalysisProgress.current / bulkAnalysisProgress.total) * 100}%` }}
                  aria-valuenow={bulkAnalysisProgress.current}
                  aria-valuemin="0"
                  aria-valuemax={bulkAnalysisProgress.total}
                ></div>
              </div>
            </div>
          )}

          {bulkAnalysisResults.length > 0 && (
            <div>
              <div className="mb-3">
                <Badge color="success" className="me-2">
                  {bulkAnalysisResults.filter(r => r.status === 'success').length} Successful
                </Badge>
                <Badge color="danger">
                  {bulkAnalysisResults.filter(r => r.status === 'error').length} Failed
                </Badge>
              </div>

              <div className="table-responsive">
                <table className="table table-sm">
                  <thead>
                    <tr>
                      <th>Stream</th>
                      <th>Region</th>
                      <th>Account</th>
                      <th>Status</th>
                      <th>Summary</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {bulkAnalysisResults.map((result, index) => (
                      <tr key={index}>
                        <td>
                          <div className="d-flex align-items-center">
                            <i className="fas fa-stream text-primary me-2"></i>
                            <div>
                              <div className="fw-bold">{result.stream.name || result.stream.resource_id}</div>
                              <small className="text-muted">{result.stream.resource_id}</small>
                            </div>
                          </div>
                        </td>
                        <td>
                          <Badge color="outline-secondary" size="sm">{result.stream.region}</Badge>
                        </td>
                        <td>
                          <small>{result.stream.account_id}</small>
                        </td>
                        <td>
                          {result.status === 'success' ? (
                            <Badge color="success">
                              <i className="fas fa-check me-1"></i>
                              Success
                            </Badge>
                          ) : (
                            <Badge color="danger">
                              <i className="fas fa-times me-1"></i>
                              Failed
                            </Badge>
                          )}
                        </td>
                        <td>
                          {result.status === 'success' ? (
                            <div className="small">
                              {(result.result.content || '').substring(0, 100)}...
                            </div>
                          ) : (
                            <div className="small text-danger">
                              {result.error}
                            </div>
                          )}
                        </td>
                        <td>
                          {result.status === 'success' && (
                            <Button
                              color="outline-primary"
                              size="sm"
                              onClick={() => {
                                setSelectedStream(result.stream);
                                setAnalysisResult(result.result);
                              }}
                            >
                              <i className="fas fa-eye me-1"></i>
                              View
                            </Button>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {!isBulkAnalyzing && bulkAnalysisResults.length === 0 && (
            <div className="text-center text-muted">
              <i className="fas fa-info-circle me-2"></i>
              No bulk analysis results yet. Click "Analyze All Streams" to get started.
            </div>
          )}
        </CardBody>
      </Card>
    );
  };

  const modeInfo = getAnalysisModeInfo();

  return (
    <div>
      <PageHeader 
        title="Kinesis Stream Analysis" 
        icon="fa-stream"
        breadcrumbs={[
          { name: "Cloud", path: "/cloud" },
          { name: "Kinesis Analysis", active: true }
        ]}
      />
      
      <Row>
        <Col lg={4}>
          <Card>
            <CardHeader>
              <h5 className="mb-0">
                <i className="fas fa-stream me-2"></i>
                Select Stream
              </h5>
            </CardHeader>
            <CardBody>
              {loading && kinesisStreams.length === 0 && (
                <div className="text-center">
                  <Spinner size="sm" /> Loading streams...
                </div>
              )}
              
              {kinesisStreams.length === 0 && !loading && (
                <Alert color="warning">
                  <i className="fas fa-exclamation-triangle me-2"></i>
                  No Kinesis streams found. Make sure you have streams in your AWS accounts.
                </Alert>
              )}

              {kinesisStreams.length > 0 && (
                <Form>
                  <FormGroup>
                    <Label for="streamSelect">Kinesis Stream</Label>
                    <Input
                      type="select"
                      id="streamSelect"
                      value={selectedStream?.id || ''}
                      onChange={(e) => {
                        const stream = kinesisStreams.find(s => s.id === parseInt(e.target.value));
                        setSelectedStream(stream);
                      }}
                    >
                      {kinesisStreams.map(stream => (
                        <option key={stream.id} value={stream.id}>
                          {stream.name || stream.resource_id} ({stream.region})
                        </option>
                      ))}
                    </Input>
                  </FormGroup>
                </Form>
              )}

              {selectedStream && (
                <div className="mt-3 p-3 bg-light rounded">
                  <h6>Stream Details</h6>
                  <p className="mb-1"><strong>Name:</strong> {selectedStream.name || selectedStream.resource_id}</p>
                  <p className="mb-1"><strong>Region:</strong> {selectedStream.region}</p>
                  <p className="mb-1"><strong>Account:</strong> {selectedStream.account_id}</p>
                  <p className="mb-0"><strong>ARN:</strong> <code className="small">{selectedStream.arn}</code></p>
                </div>
              )}
            </CardBody>
          </Card>

          <Card className="mt-4">
            <CardHeader>
              <h5 className="mb-0">
                <i className={`fas fa-${modeInfo.icon} me-2`}></i>
                Analysis Mode
              </h5>
            </CardHeader>
            <CardBody>
              <ButtonGroup vertical className="w-100">
                <Button
                  color={analysisMode === 'auto' ? modeInfo.color : 'outline-secondary'}
                  onClick={() => setAnalysisMode('auto')}
                  id="mode-auto"
                >
                  <i className="fas fa-magic me-2"></i>
                  Auto-Select
                </Button>
                <UncontrolledTooltip target="mode-auto">
                  Automatically chooses LLM or metrics-based analysis based on availability
                </UncontrolledTooltip>

                <Button
                  color={analysisMode === 'llm' ? 'info' : 'outline-secondary'}
                  onClick={() => setAnalysisMode('llm')}
                  id="mode-llm"
                >
                  <i className="fas fa-brain me-2"></i>
                  LLM-Powered
                </Button>
                <UncontrolledTooltip target="mode-llm">
                  Uses AI for advanced pattern recognition and intelligent recommendations
                </UncontrolledTooltip>

                <Button
                  color={analysisMode === 'metrics' ? 'primary' : 'outline-secondary'}
                  onClick={() => setAnalysisMode('metrics')}
                  id="mode-metrics"
                >
                  <i className="fas fa-chart-line me-2"></i>
                  Metrics-Based
                </Button>
                <UncontrolledTooltip target="mode-metrics">
                  Direct CloudWatch metrics analysis for performance and cost insights
                </UncontrolledTooltip>
              </ButtonGroup>

              <div className="mt-3 p-2 rounded" style={{ backgroundColor: `rgba(var(--bs-${modeInfo.color}-rgb), 0.1)` }}>
                <small>
                  <strong>{modeInfo.title}:</strong><br />
                  {modeInfo.description}
                </small>
              </div>
            </CardBody>
          </Card>
        </Col>

        <Col lg={8}>
          <Card>
            <CardHeader>
              <h5 className="mb-0">
                <i className="fas fa-cogs me-2"></i>
                Analysis Workflows
              </h5>
            </CardHeader>
            <CardBody>
              <div className="row">
                {workflows.map(workflow => (
                  <div key={workflow.id} className="col-md-6 mb-3">
                    <div 
                      className={`card h-100 cursor-pointer ${selectedWorkflow === workflow.id ? 'border-primary' : ''}`}
                      style={{ cursor: 'pointer' }}
                      onClick={() => setSelectedWorkflow(workflow.id)}
                    >
                      <div className="card-body p-3">
                        <div className="d-flex align-items-center mb-2">
                          <i className={`fas fa-${workflow.icon} text-${workflow.color} me-2`}></i>
                          <h6 className="mb-0">{workflow.name}</h6>
                          {workflow.requiresLLM && (
                            <Badge color="info" size="sm" className="ms-auto">LLM</Badge>
                          )}
                        </div>
                        <p className="small text-muted mb-0">{workflow.description}</p>
                      </div>
                    </div>
                  </div>
                ))}
              </div>

              {error && (
                <Alert color="danger" className="mt-3">
                  <i className="fas fa-exclamation-triangle me-2"></i>
                  {error}
                </Alert>
              )}

              <div className="d-flex justify-content-between align-items-center mt-4">
                <div>
                  {selectedWorkflow && (
                    <Badge color="outline-primary">
                      Selected: {workflows.find(w => w.id === selectedWorkflow)?.name}
                    </Badge>
                  )}
                </div>
                <div>
                  <Button
                    color="primary"
                    size="lg"
                    onClick={runAnalysis}
                    disabled={!selectedStream || loading || isBulkAnalyzing}
                    className="me-2"
                  >
                    {loading ? (
                      <>
                        <Spinner size="sm" className="me-2" />
                        Analyzing...
                      </>
                    ) : (
                      <>
                        <i className="fas fa-play me-2"></i>
                        Run Single Analysis
                      </>
                    )}
                  </Button>
                  <Button
                    color="success"
                    size="lg"
                    onClick={runBulkAnalysis}
                    disabled={!kinesisStreams || kinesisStreams.length === 0 || loading || isBulkAnalyzing}
                  >
                    {isBulkAnalyzing ? (
                      <>
                        <Spinner size="sm" className="me-2" />
                        Analyzing All ({bulkAnalysisProgress.current}/{bulkAnalysisProgress.total})
                      </>
                    ) : (
                      <>
                        <i className="fas fa-play-circle me-2"></i>
                        Analyze All Streams ({kinesisStreams.length})
                      </>
                    )}
                  </Button>
                </div>
              </div>
            </CardBody>
          </Card>

          {renderAnalysisResult()}
          {renderBulkAnalysisResults()}
        </Col>
      </Row>
    </div>
  );
};

export default KinesisAnalysis;
