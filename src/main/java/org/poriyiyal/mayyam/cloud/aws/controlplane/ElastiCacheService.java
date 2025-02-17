package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.springframework.stereotype.Service;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.elasticache.ElastiCacheClient;
import software.amazon.awssdk.services.elasticache.model.*;

import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.stream.Collectors;

@Service
public class ElastiCacheService extends BaseAwsService {

    private final ConcurrentMap<Region, ElastiCacheClient> clientCache = new ConcurrentHashMap<>();

    private ElastiCacheClient getElastiCacheClient(String region) {
        return clientCache.computeIfAbsent(Region.of(region), r -> ElastiCacheClient.builder()
                .region(r)
                .credentialsProvider(credentialsProvider)
                .build());
    }

    // Create an ElastiCache cluster
    public void createCacheCluster(String region, String clusterId, Map<String, Object> properties) {
        if (clusterId == null || clusterId.isEmpty()) {
            throw new IllegalArgumentException("Cluster ID cannot be null or empty");
        }

        try {
            ElastiCacheClient elastiCacheClient = getElastiCacheClient(region);
            CreateCacheClusterRequest.Builder requestBuilder = CreateCacheClusterRequest.builder()
                    .cacheClusterId(clusterId);

            // Set properties from the map
            properties.forEach((key, value) -> {
                switch (key) {
                    case "cacheNodeType":
                        requestBuilder.cacheNodeType((String) value);
                        break;
                    case "engine":
                        requestBuilder.engine((String) value);
                        break;
                    case "numCacheNodes":
                        requestBuilder.numCacheNodes((Integer) value);
                        break;
                    // Add more cases as needed for other properties
                }
            });

            elastiCacheClient.createCacheCluster(requestBuilder.build());
            System.out.println("Cache cluster created successfully: " + clusterId);
        } catch (ElastiCacheException e) {
            System.err.println("Failed to create cache cluster: " + e.getMessage());
            throw e;
        }
    }

    // Describe ElastiCache clusters
    public List<CacheCluster> describeCacheClusters(String region) {
        try {
            ElastiCacheClient elastiCacheClient = getElastiCacheClient(region);
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
    public CacheCluster describeCacheCluster(String region, String clusterId) {
        if (clusterId == null || clusterId.isEmpty()) {
            throw new IllegalArgumentException("Invalid input parameter for describing cache cluster");
        }

        try {
            ElastiCacheClient elastiCacheClient = getElastiCacheClient(region);
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
    public void deleteCacheCluster(String region, String clusterId) {
        if (clusterId == null || clusterId.isEmpty()) {
            throw new IllegalArgumentException("Cluster ID cannot be null or empty");
        }

        try {
            ElastiCacheClient elastiCacheClient = getElastiCacheClient(region);
            DeleteCacheClusterRequest request = DeleteCacheClusterRequest.builder()
                    .cacheClusterId(clusterId)
                    .build();
            elastiCacheClient.deleteCacheCluster(request);
            System.out.println("Cache cluster deleted successfully: " + clusterId);
        } catch (ElastiCacheException e) {
            System.err.println("Failed to delete cache cluster: " + e.getMessage());
            throw e;
        }
    }

    // Modify an ElastiCache cluster
    public CacheCluster modifyCacheCluster(String region, String clusterId, String nodeType, int numNodes) {
        if (clusterId == null || clusterId.isEmpty() || nodeType == null || nodeType.isEmpty() || numNodes <= 0) {
            throw new IllegalArgumentException("Invalid input parameters for modifying cache cluster");
        }

        try {
            ElastiCacheClient elastiCacheClient = getElastiCacheClient(region);
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
    public List<ReplicationGroup> listReplicationGroups(String region) {
        try {
            ElastiCacheClient elastiCacheClient = getElastiCacheClient(region);
            DescribeReplicationGroupsRequest request = DescribeReplicationGroupsRequest.builder().build();
            DescribeReplicationGroupsResponse response = elastiCacheClient.describeReplicationGroups(request);
            return response.replicationGroups();
        } catch (ElastiCacheException e) {
            throw new RuntimeException("Failed to list replication groups: " + e.getMessage(), e);
        }
    }

    // Describe a specific replication group
    public ReplicationGroup describeReplicationGroup(String region, String replicationGroupId) {
        if (replicationGroupId == null || replicationGroupId.isEmpty()) {
            throw new IllegalArgumentException("Invalid input parameter for describing replication group");
        }

        try {
            ElastiCacheClient elastiCacheClient = getElastiCacheClient(region);
            DescribeReplicationGroupsRequest request = DescribeReplicationGroupsRequest.builder()
                    .replicationGroupId(replicationGroupId)
                    .build();
            DescribeReplicationGroupsResponse response = elastiCacheClient.describeReplicationGroups(request);
            return response.replicationGroups().get(0);
        } catch (ElastiCacheException e) {
            throw new RuntimeException("Failed to describe replication group: " + e.getMessage(), e);
        }
    }

    // List ElastiCache clusters
    public List<CacheCluster> listCacheClusters(String region) {
        ElastiCacheClient elastiCacheClient = getElastiCacheClient(region);
        DescribeCacheClustersRequest request = DescribeCacheClustersRequest.builder()
                .showCacheNodeInfo(true)
                .build();
        DescribeCacheClustersResponse response = elastiCacheClient.describeCacheClusters(request);
        return response.cacheClusters();
    }
}