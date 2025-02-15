package org.poriyiyal.mayyam.cloud.aws.controlplane;

import software.amazon.awssdk.services.rds.RdsClient;
import software.amazon.awssdk.services.rds.model.*;

import java.util.List;
import java.util.stream.Collectors;

public class RdsService extends BaseAwsService {
    private final RdsClient rdsClient;

    public RdsService() {
        this.rdsClient = RdsClient.builder()
                .region(region)
                .credentialsProvider(credentialsProvider)
                .build();
    }

    public List<DBInstance> listDBInstances() {
        try {
            DescribeDbInstancesResponse response = rdsClient.describeDBInstances();
            return response.dbInstances();
        } catch (RdsException e) {
            System.err.println("Error listing DB instances: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public DBInstance describeDBInstance(String dbInstanceIdentifier) {
        if (dbInstanceIdentifier == null || dbInstanceIdentifier.isEmpty()) {
            throw new IllegalArgumentException("DB instance identifier cannot be null or empty");
        }

        try {
            DescribeDbInstancesRequest request = DescribeDbInstancesRequest.builder()
                    .dbInstanceIdentifier(dbInstanceIdentifier)
                    .build();
            DescribeDbInstancesResponse response = rdsClient.describeDBInstances(request);
            return response.dbInstances().stream().findFirst().orElse(null);
        } catch (RdsException e) {
            System.err.println("Error describing DB instance: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public DBInstance createDBInstance(String dbInstanceIdentifier, String dbInstanceClass, String engine) {
        if (dbInstanceIdentifier == null || dbInstanceIdentifier.isEmpty()) {
            throw new IllegalArgumentException("DB instance identifier cannot be null or empty");
        }
        if (dbInstanceClass == null || dbInstanceClass.isEmpty()) {
            throw new IllegalArgumentException("DB instance class cannot be null or empty");
        }
        if (engine == null || engine.isEmpty()) {
            throw new IllegalArgumentException("Engine cannot be null or empty");
        }

        try {
            CreateDbInstanceRequest request = CreateDbInstanceRequest.builder()
                    .dbInstanceIdentifier(dbInstanceIdentifier)
                    .dbInstanceClass(dbInstanceClass)
                    .engine(engine)
                    .allocatedStorage(20) // Example value, adjust as needed
                    .build();
            CreateDbInstanceResponse response = rdsClient.createDBInstance(request);
            return response.dbInstance();
        } catch (RdsException e) {
            System.err.println("Error creating DB instance: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public void deleteDBInstance(String dbInstanceIdentifier) {
        if (dbInstanceIdentifier == null || dbInstanceIdentifier.isEmpty()) {
            throw new IllegalArgumentException("DB instance identifier cannot be null or empty");
        }

        try {
            DeleteDbInstanceRequest request = DeleteDbInstanceRequest.builder()
                    .dbInstanceIdentifier(dbInstanceIdentifier)
                    .skipFinalSnapshot(true) // Example value, adjust as needed
                    .build();
            rdsClient.deleteDBInstance(request);
        } catch (RdsException e) {
            System.err.println("Error deleting DB instance: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }
}