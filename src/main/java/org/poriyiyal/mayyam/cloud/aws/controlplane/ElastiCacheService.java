package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.springframework.stereotype.Service;
import software.amazon.awssdk.services.elasticache.ElastiCacheClient;
import software.amazon.awssdk.services.elasticache.model.*;

import java.util.List;

@Service
public class ElastiCacheService extends BaseAwsService {
    private final ElastiCacheClient elastiCacheClient;

    public ElastiCacheService() {
        this.elastiCacheClient = ElastiCacheClient.builder()
                .region(region)
                .credentialsProvider(credentialsProvider)
                .build();
    }

    // Create an ElastiCache cluster
    public CacheCluster createCacheCluster(String clusterId, String nodeType, int numNodes, String engine) {
        if (clusterId == null || clusterId.isEmpty() || nodeType == null || nodeType.isEmpty() || engine == null || engine.isEmpty() || numNodes <= 0) {
            throw new IllegalArgumentException("Invalid input parameters for creating cache cluster");
        }

        try {
            CreateCacheClusterRequest request = CreateCacheClusterRequest.builder()
                    .cacheClusterId(clusterId)
                    .cacheNodeType(nodeType)
                    .numCacheNodes(numNodes)
                    .engine(engine)
                    .build();
            CreateCacheClusterResponse response = elastiCacheClient.createCacheCluster(request);
            return response.cacheCluster();
        } catch (ElastiCacheException e) {
            throw new RuntimeException("Failed to create cache cluster: " + e.getMessage(), e);
        }
    }

    // Describe ElastiCache clusters
    public List<CacheCluster> describeCacheClusters() {
        try {
            DescribeCacheClustersRequest request = DescribeCacheClustersRequest.builder()
                    .showCacheNodeInfo(true)
                    .build();
            DescribeCacheClustersResponse response = elastiCacheClient.describeCacheClusters(request);
            return response.cacheClusters();
        } catch (ElastiCacheException e) {
            throw new RuntimeException("Failed to describe cache clusters: " + e.getMessage(), e);
        }
    }

    // Describe a specific ElastiCache cluster
    public CacheCluster describeCacheCluster(String clusterId) {
        if (clusterId == null || clusterId.isEmpty()) {
            throw new IllegalArgumentException("Invalid input parameter for describing cache cluster");
        }

        try {
            DescribeCacheClustersRequest request = DescribeCacheClustersRequest.builder()
                    .cacheClusterId(clusterId)
                    .showCacheNodeInfo(true)
                    .build();
            DescribeCacheClustersResponse response = elastiCacheClient.describeCacheClusters(request);
            return response.cacheClusters().get(0);
        } catch (ElastiCacheException e) {
            throw new RuntimeException("Failed to describe cache cluster: " + e.getMessage(), e);
        }
    }

    // Delete an ElastiCache cluster
    public CacheCluster deleteCacheCluster(String clusterId) {
        if (clusterId == null || clusterId.isEmpty()) {
            throw new IllegalArgumentException("Invalid input parameter for deleting cache cluster");
        }

        try {
            DeleteCacheClusterRequest request = DeleteCacheClusterRequest.builder()
                    .cacheClusterId(clusterId)
                    .build();
            DeleteCacheClusterResponse response = elastiCacheClient.deleteCacheCluster(request);
            return response.cacheCluster();
        } catch (ElastiCacheException e) {
            throw new RuntimeException("Failed to delete cache cluster: " + e.getMessage(), e);
        }
    }

    // Modify an ElastiCache cluster
    public CacheCluster modifyCacheCluster(String clusterId, String nodeType, int numNodes) {
        if (clusterId == null || clusterId.isEmpty() || nodeType == null || nodeType.isEmpty() || numNodes <= 0) {
            throw new IllegalArgumentException("Invalid input parameters for modifying cache cluster");
        }

        try {
            ModifyCacheClusterRequest request = ModifyCacheClusterRequest.builder()
                    .cacheClusterId(clusterId)
                    .cacheNodeType(nodeType)
                    .numCacheNodes(numNodes)
                    .applyImmediately(true)
                    .build();
            ModifyCacheClusterResponse response = elastiCacheClient.modifyCacheCluster(request);
            return response.cacheCluster();
        } catch (ElastiCacheException e) {
            throw new RuntimeException("Failed to modify cache cluster: " + e.getMessage(), e);
        }
    }

    // List replication groups
    public List<ReplicationGroup> listReplicationGroups() {
        try {
            DescribeReplicationGroupsRequest request = DescribeReplicationGroupsRequest.builder().build();
            DescribeReplicationGroupsResponse response = elastiCacheClient.describeReplicationGroups(request);
            return response.replicationGroups();
        } catch (ElastiCacheException e) {
            throw new RuntimeException("Failed to list replication groups: " + e.getMessage(), e);
        }
    }

    // Describe a specific replication group
    public ReplicationGroup describeReplicationGroup(String replicationGroupId) {
        if (replicationGroupId == null || replicationGroupId.isEmpty()) {
            throw new IllegalArgumentException("Invalid input parameter for describing replication group");
        }

        try {
            DescribeReplicationGroupsRequest request = DescribeReplicationGroupsRequest.builder()
                    .replicationGroupId(replicationGroupId)
                    .build();
            DescribeReplicationGroupsResponse response = elastiCacheClient.describeReplicationGroups(request);
            return response.replicationGroups().get(0);
        } catch (ElastiCacheException e) {
            throw new RuntimeException("Failed to describe replication group: " + e.getMessage(), e);
        }
    }
}