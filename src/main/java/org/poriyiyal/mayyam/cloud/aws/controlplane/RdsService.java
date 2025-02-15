package org.poriyiyal.mayyam.cloud.aws.controlplane;

import software.amazon.awssdk.services.rds.RdsClient;
import software.amazon.awssdk.services.rds.model.*;
import org.springframework.stereotype.Service;

import java.util.HashMap;
import java.util.List;
import java.util.Scanner;
import java.util.stream.Collectors;

@Service
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

    public void scaleDBInstance(String dbInstanceIdentifier, String newDbInstanceClass) {
        if (dbInstanceIdentifier == null || dbInstanceIdentifier.isEmpty()) {
            throw new IllegalArgumentException("DB instance identifier cannot be null or empty");
        }
        if (newDbInstanceClass == null || newDbInstanceClass.isEmpty()) {
            throw new IllegalArgumentException("New DB instance class cannot be null or empty");
        }

        DBInstance dbInstance = describeDBInstance(dbInstanceIdentifier);
        if (dbInstance == null) {
            System.err.println("DB instance with identifier " + dbInstanceIdentifier + " does not exist.");
            return;
        }

        Scanner scanner = new Scanner(System.in);
        System.out.println("Are you sure you want to scale the DB instance " + dbInstanceIdentifier + " to class " + newDbInstanceClass + "? (yes/no)");
        String confirmation = scanner.nextLine();

        if (!confirmation.equalsIgnoreCase("yes")) {
            System.out.println("Scaling operation cancelled.");
            return;
        }

        try {
            ModifyDbInstanceRequest request = ModifyDbInstanceRequest.builder()
                    .dbInstanceIdentifier(dbInstanceIdentifier)
                    .dbInstanceClass(newDbInstanceClass)
                    .applyImmediately(true) // Apply changes immediately
                    .build();
            rdsClient.modifyDBInstance(request);
            System.out.println("DB instance " + dbInstanceIdentifier + " scaled to class " + newDbInstanceClass + " successfully.");
        } catch (RdsException e) {
            System.err.println("Error scaling DB instance: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public HashMap<String, DBInstance> listDBInstancesAsMap() {
        try {
            DescribeDbInstancesResponse response = rdsClient.describeDBInstances();
            return response.dbInstances().stream()
                    .collect(Collectors.toMap(DBInstance::dbInstanceIdentifier, dbInstance -> dbInstance, (oldValue, newValue) -> oldValue, HashMap::new));
        } catch (RdsException e) {
            System.err.println("Error listing DB instances as map: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public List<GlobalCluster> listGlobalClusters() {
        try {
            DescribeGlobalClustersResponse response = rdsClient.describeGlobalClusters();
            return response.globalClusters();
        } catch (RdsException e) {
            System.err.println("Error listing global clusters: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public List<DBCluster> listClusters() {
        DescribeDbClustersResponse response = rdsClient.describeDBClusters();
        return response.dbClusters();
    }
}