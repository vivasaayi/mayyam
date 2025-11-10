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


import React, { useEffect, useState } from "react";
import { Card, CardHeader, CardBody, Table, Badge, Spinner, Alert, Button, Row, Col, Form, FormGroup, Label, Input } from "reactstrap";
import { useNavigate } from "react-router-dom";
import { getSyncRuns } from "../services/api";

const SyncRunsDashboard = () => {
  const [runs, setRuns] = useState([]);
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [status, setStatus] = useState("");

  const load = async (statusFilter = "") => {
    try {
      setLoading(true);
      const data = await getSyncRuns(statusFilter || null);
      setRuns(data);
    } catch (e) {
      console.error("Failed to load sync runs", e);
      setError("Failed to load sync runs");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load(status);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status]);

  return (
    <div>
      {error && <Alert color="danger">{error}</Alert>}
      <Card>
        <CardHeader>
          <div className="d-flex justify-content-between align-items-center w-100">
            <h5 className="mb-0"><i className="fas fa-list me-2"></i>Sync Runs</h5>
          </div>
        </CardHeader>
        <CardBody>
          <Form className="mb-3" onSubmit={(e) => { e.preventDefault(); load(status); }}>
            <Row className="g-2 align-items-end">
              <Col md="3">
                <FormGroup>
                  <Label for="statusFilter">Status</Label>
                  <Input
                    id="statusFilter"
                    type="select"
                    value={status}
                    onChange={(e) => setStatus(e.target.value)}
                  >
                    <option value="">All</option>
                    <option value="created">Created</option>
                    <option value="running">Running</option>
                    <option value="completed">Completed</option>
                    <option value="failed">Failed</option>
                  </Input>
                </FormGroup>
              </Col>
              <Col md="2">
                <Button color="primary" type="submit" className="mt-2">Apply</Button>
              </Col>
            </Row>
          </Form>
          {loading ? (
            <div className="text-center p-4">
              <Spinner />
            </div>
          ) : (
            <Table responsive striped hover>
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Name</th>
                  <th>Region Scope</th>
                  <th>Regions</th>
                  <th>Status</th>
                  <th>Total</th>
                  <th>Success</th>
                  <th>Failure</th>
                  <th>Started</th>
                  <th>Completed</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {runs.map(run => (
                  <tr key={run.id}>
                    <td style={{maxWidth: 220}}>{run.id}</td>
                    <td>{run.name}</td>
                    <td>{run.region_scope || (run.metadata?.all_regions ? 'all' : (run.metadata?.regions?.length > 0 ? 'custom' : ''))}</td>
                    <td style={{maxWidth: 280, whiteSpace: 'nowrap', textOverflow: 'ellipsis', overflow: 'hidden'}}>
                      {Array.isArray(run.regions) && run.regions.length > 0
                        ? run.regions.join(', ')
                        : (Array.isArray(run.metadata?.regions) && run.metadata.regions.length > 0 ? run.metadata.regions.join(', ') : '-')}
                    </td>
                    <td>
                      <Badge color={run.status === 'completed' ? 'success' : run.status === 'failed' ? 'danger' : run.status === 'running' ? 'primary' : 'secondary'}>
                        {run.status}
                      </Badge>
                    </td>
                    <td>{run.total_resources}</td>
                    <td>{run.success_count}</td>
                    <td>{run.failure_count}</td>
                    <td>{run.started_at ? new Date(run.started_at).toLocaleString() : '-'}</td>
                    <td>{run.completed_at ? new Date(run.completed_at).toLocaleString() : '-'}</td>
                    <td>
                      <Button size="sm" color="link" onClick={() => navigate(`/cloud-resources?sync_id=${run.id}`)}>
                        View Resources
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </Table>
          )}
        </CardBody>
      </Card>
    </div>
  );
};

export default SyncRunsDashboard;
