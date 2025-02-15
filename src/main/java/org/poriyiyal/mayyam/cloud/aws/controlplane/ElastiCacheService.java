package org.poriyiyal.mayyam.cloud.aws.controlplane;

import software.amazon.awssdk.services.elasticache.ElastiCacheClient;

public class ElastiCacheService extends BaseAwsService {
    private final ElastiCacheClient elastiCacheClient;

    public ElastiCacheService() {
        this.elastiCacheClient = ElastiCacheClient.builder()
                .region(region)
                .credentialsProvider(credentialsProvider)
                .build();
    }

    // Add methods to interact with ElastiCache
}