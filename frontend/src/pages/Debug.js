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


import React from 'react';
import { Container, Row, Col } from 'reactstrap';
import TokenDebugger from '../components/common/TokenDebugger';

const Debug = () => {
  return (
    <Container fluid>
      <Row>
        <Col>
          <h1 className="mt-4 mb-3">Debug Tools</h1>
        </Col>
      </Row>
      <Row>
        <Col>
          <TokenDebugger />
        </Col>
      </Row>
    </Container>
  );
};

export default Debug;
