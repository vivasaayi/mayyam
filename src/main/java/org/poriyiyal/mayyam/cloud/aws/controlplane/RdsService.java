package org.poriyiyal.mayyam.cloud.aws.controlplane;

import software.amazon.awssdk.services.rds.RdsClient;

public class RdsService extends BaseAwsService {
    private final RdsClient rdsClient;

    public RdsService() {
        this.rdsClient = RdsClient.builder()
                .region(region)
                .credentialsProvider(credentialsProvider)
                .build();
    }

    // Add methods to interact with RDS
}