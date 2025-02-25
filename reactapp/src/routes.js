import React from 'react'

const Dashboard = React.lazy(() => import('./views/dashboard/Dashboard'))
const Colors = React.lazy(() => import('./views/theme/colors/Colors'))
const Typography = React.lazy(() => import('./views/theme/typography/Typography'))

// Base
const Accordion = React.lazy(() => import('./views/base/accordion/Accordion'))

// Buttons
const ButtonGroups = React.lazy(() => import('./views/buttons/button-groups/ButtonGroups'))

const RdsInstances = React.lazy(() => import('./components/RDS/RdsInstances'))

// ElastiCache
const ElastiCacheClusters = React.lazy(() => import('./views/ElastiCacheClusters'))
const ElastiCacheReplicationGroups = React.lazy(() => import('./views/ElastiCacheReplicationGroups'))

const KinesisList = React.lazy(() => import('./components/Kinesis/KinesisList'))
const SqsList = React.lazy(() => import('./components/Sqs/SqsList'))
const DynamoDbList = React.lazy(() => import('./components/DynamoDb/DynamoDbList'))
const S3List = React.lazy(() => import('./components/S3/S3List'))
const S3Replication = React.lazy(() => import('./components/S3/S3Replication'))
const DynamoDbReplication = React.lazy(() => import('./components/DynamoDb/DynamoDbReplication'))
const DynamoDbTablesWithoutPITR = React.lazy(() => import('./components/DynamoDb/DynamoDbTablesWithoutPITR'))
const ClusterMap = React.lazy(() => import('./components/RDS/ClusterMap'))
const GlobalClusterStatus = React.lazy(() => import('./components/RDS/GlobalClusterStatus'))
const KubernetesDashboard = React.lazy(() => import('./components/Kubernetes/KubernetesDashboard'))
const KubernetesPods = React.lazy(() => import('./components/Kubernetes/KubernetesPods'))
const KubernetesPodDetails = React.lazy(() => import('./components/Kubernetes/KubernetesPodDetails'))
const KubernetesTriaging = React.lazy(() => import('./components/Kubernetes/KubernetesTriaging'))
const SearchDomainTab = React.lazy(() => import('./components/Kubernetes/SearchDomainTab'))

const routes = [
  { path: '/', exact: true, name: 'Home' },
  { path: '/dashboard', name: 'Dashboard', element: Dashboard },
  { path: '/theme', name: 'Theme', element: Colors, exact: true },
  { path: '/theme/colors', name: 'Colors', element: Colors },
  { path: '/theme/typography', name: 'Typography', element: Typography },
  { path: '/base/accordion', name: 'Accordion', element: Accordion },
  { path: '/rds-instances', name: 'RdsInstances', element: RdsInstances },
  { path: '/elasticache-clusters', name: 'ElastiCacheClusters', element: ElastiCacheClusters },
  { path: '/elasticache-replication-groups', name: 'ElastiCacheReplicationGroups', element: ElastiCacheReplicationGroups },
  { path: '/kinesis/list', name: 'Kinesis Streams', element: KinesisList },
  { path: '/sqs/list', name: 'SQS Queues', element: SqsList },
  { path: '/dynamodb/list', name: 'DynamoDB Tables', element: DynamoDbList },
  { path: '/s3/list', name: 'S3 Buckets', element: S3List },
  { path: '/s3/replication', name: 'S3 Replication', element: S3Replication },
  { path: '/dynamodb/replication', name: 'DynamoDB Replication', element: DynamoDbReplication },
  { path: '/dynamodb/tablesWithoutPITR', name: 'DynamoDb Tables Without PITR', element: DynamoDbTablesWithoutPITR },
  { path: '/rds/cluster-map', name: 'Cluster Map', element: ClusterMap },
  { path: '/global-cluster-status', name: 'Global Cluster Status', element: GlobalClusterStatus },
  { path: '/kubernetes-dashboard', name: 'Kubernetes Dashboard', element: KubernetesDashboard },
  { path: '/kubernetes-pods', name: 'Kubernetes Pods', element: KubernetesPods },
  { path: '/kubernetes-pod-details', name: 'Kubernetes Pod Details', element: KubernetesPodDetails },
  { path: '/kubernetes/triaging', name: 'Kubernetes Triaging', element: KubernetesTriaging },
  { path: '/kubernetes/search-domain', name: 'Search Domain Tab', element: SearchDomainTab },
]

export default routes
