import React, { useEffect, useState } from "react";
import { useLocation } from "react-router-dom";
import { Card, CardHeader, CardBody, Row, Col, Form, FormGroup, Label, Input, Button } from "reactstrap";
import AwsResourceBrowser from "../components/cloud/AwsResourceBrowser";

function useQuery() {
  const { search } = useLocation();
  return React.useMemo(() => new URLSearchParams(search), [search]);
}

const CloudResources = () => {
  const query = useQuery();
  const syncIdFromQuery = query.get("sync_id");
  const [syncId, setSyncId] = useState(syncIdFromQuery || "");

  // Pass syncId down via context-ish pattern using location state, AwsResourceBrowser reads from URL
  useEffect(() => {
    if (syncIdFromQuery) setSyncId(syncIdFromQuery);
  }, [syncIdFromQuery]);

  return (
    <div className="animated fadeIn">
      <Card>
        <CardHeader>
          <div className="d-flex justify-content-between align-items-center w-100">
            <h5 className="mb-0"><i className="fa fa-cloud me-2"></i>Cloud Resources</h5>
          </div>
        </CardHeader>
        <CardBody>
          <Row>
            <Col>
              {/* We reuse AwsResourceBrowser which supports query params; ensure it forwards sync_id */}
              <AwsResourceBrowser />
            </Col>
          </Row>
        </CardBody>
      </Card>
    </div>
  );
};

export default CloudResources;
