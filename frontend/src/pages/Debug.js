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
