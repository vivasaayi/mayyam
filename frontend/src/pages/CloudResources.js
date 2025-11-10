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
import { useLocation } from "react-router-dom";
import { Card, CardHeader, CardBody, Row, Col } from "reactstrap";
import CloudResourceBrowser from "../components/cloud/CloudResourceBrowser";

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
              {/* Unified browser hitting /api/cloud/resources with provider/type/sync_id filters */}
              <CloudResourceBrowser />
            </Col>
          </Row>
        </CardBody>
      </Card>
    </div>
  );
};

export default CloudResources;
