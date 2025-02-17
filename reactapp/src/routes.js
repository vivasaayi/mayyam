import React from 'react'

const Dashboard = React.lazy(() => import('./views/dashboard/Dashboard'))
const Colors = React.lazy(() => import('./views/theme/colors/Colors'))
const Typography = React.lazy(() => import('./views/theme/typography/Typography'))

// Base
const Accordion = React.lazy(() => import('./views/base/accordion/Accordion'))

// Buttons
const ButtonGroups = React.lazy(() => import('./views/buttons/button-groups/ButtonGroups'))

const RdsInstances = React.lazy(() => import('./views/RdsInstances'))

// ElastiCache
const ElastiCacheClusters = React.lazy(() => import('./views/ElastiCacheClusters'))
const ElastiCacheReplicationGroups = React.lazy(() => import('./views/ElastiCacheReplicationGroups'))

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
]

export default routes
