import React, { Suspense, useEffect } from 'react'
import { BrowserRouter as Router, Route, Routes, HashRouter } from 'react-router-dom'
import { useSelector } from 'react-redux'

import { CSpinner, useColorModes, CContainer } from '@coreui/react'
import './scss/style.scss'

// We use those styles to show code examples, you should remove them in your application.
import './scss/examples.scss'

// Containers
const DefaultLayout = React.lazy(() => import('./layout/DefaultLayout'))

// Pages
const RdsClusters = React.lazy(() => import('./components/RDS/RdsClusters'))
// const RDSClustersTable = React.lazy(() => import('./components/RDS/RDSClustersTable'))
const DynamoDbList = React.lazy(() => import('./components/DynamoDb/DynamoDbList'))
// const ElastiCacheClusters = React.lazy(() => import('./components/ElastiCache/ElastiCacheClusters'))
// const ElastiCacheReplicationGroups = React.lazy(() => import('./components/ElastiCache/ElastiCacheReplicationGroups'))
// const KinesisStreams = React.lazy(() => import('./components/Kinesis/KinesisStreams'))
// const SQSQueues = React.lazy(() => import('./components/SQS/SQSQueues'))
// const DynamoDBTables = React.lazy(() => import('./components/DynamoDB/DynamoDBTables'))
const S3List = React.lazy(() => import('./components/S3/S3List'))
const RdsInstances = React.lazy(() => import('./components/RDS/RdsInstances'))

const App = () => {
  const { isColorModeSet, setColorMode } = useColorModes('coreui-free-react-admin-template-theme')
  const storedTheme = useSelector((state) => state.theme)

  useEffect(() => {
    const urlParams = new URLSearchParams(window.location.href.split('?')[1])
    const theme = urlParams.get('theme') && urlParams.get('theme').match(/^[A-Za-z0-9\s]+/)[0]
    if (theme) {
      setColorMode(theme)
    }

    if (isColorModeSet()) {
      return
    }

    setColorMode(storedTheme)
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <HashRouter>
      <Suspense
        fallback={
          <div className="pt-3 text-center">
            <CSpinner color="primary" variant="grow" />
          </div>
        }
      >
        <CContainer>
          <Routes>
            {/* <Route path="/elasticache-clusters" component={ElastiCacheClusters} />
            <Route path="/elasticache-replication-groups" component={ElastiCacheReplicationGroups} /> */}
            {/* <Route path="/kinesis/list" component={KinesisStreams} /> */}
            {/* <Route path="/sqs/list" component={SQSQueues} /> */}
            {/* <Route path="/dynamodb/list" component={DynamoDBTables} /> */}
            {/* <Route path="/s3/list" component={S3List} /> */}
            <Route path="/rds-clusters" component={RdsClusters} />
            {/* <Route path="/rds-clusterstable" component={RdsClustersTable} /> */}
            <Route path="/rds-instances" component={RdsInstances} />
            <Route exact path="/rds-clusters" name="Rds Clusters" element={<RdsClusters />} />
            {/* <Route exact path="/rds-clusters-table" name="RDS Clusters Table" element={<RDSClustersTable />} /> */}
            <Route exact path="/dynamodb" name="DynamoDb List" element={<DynamoDbList />} />
            <Route path="*" name="Home" element={<DefaultLayout />} />
          </Routes>
        </CContainer>
      </Suspense>
    </HashRouter>
  )
}

export default App
