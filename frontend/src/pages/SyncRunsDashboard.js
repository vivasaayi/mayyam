import React, { useEffect, useState } from "react";
import { Card, CardHeader, CardBody, Table, Badge, Spinner, Alert } from "reactstrap";
import { getSyncRuns } from "../services/api";

const SyncRunsDashboard = () => {
  const [runs, setRuns] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  useEffect(() => {
    const load = async () => {
      try {
        setLoading(true);
        const data = await getSyncRuns();
        setRuns(data);
      } catch (e) {
        console.error("Failed to load sync runs", e);
        setError("Failed to load sync runs");
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  return (
    <div>
      {error && <Alert color="danger">{error}</Alert>}
      <Card>
        <CardHeader>
          <h5 className="mb-0"><i className="fas fa-list me-2"></i>Sync Runs</h5>
        </CardHeader>
        <CardBody>
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
                  <th>Status</th>
                  <th>Total</th>
                  <th>Success</th>
                  <th>Failure</th>
                  <th>Started</th>
                  <th>Completed</th>
                </tr>
              </thead>
              <tbody>
                {runs.map(run => (
                  <tr key={run.id}>
                    <td style={{maxWidth: 220}}>{run.id}</td>
                    <td>{run.name}</td>
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
