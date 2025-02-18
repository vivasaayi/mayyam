package org.poriyiyal.mayyam.cloud.aws.controlplane;

import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.rds.RdsClient;
import software.amazon.awssdk.services.rds.model.*;

import org.springframework.stereotype.Service;

import java.util.HashMap;
import java.io.Serializable;
import java.util.List;
import java.util.Map;
import java.util.Scanner;
import java.util.concurrent.ConcurrentHashMap;
import java.util.stream.Collectors;

@Service
public class RdsService extends BaseAwsService {

    // Cache RdsClient objects by region
    private static final Map<String, RdsClient> rdsClientCache = new ConcurrentHashMap<>();

    /**
     * Retrieve or build an RdsClient for the specified region.
     */
    private RdsClient getRdsClient(String regionName) {
        return rdsClientCache.computeIfAbsent(regionName, r -> RdsClient.builder()
                .region(Region.of(r))
                .credentialsProvider(credentialsProvider)
                .build());
    }

    public List<DBInstance> listDBInstances(String regionName) {
        try {
            RdsClient rdsClient = getRdsClient(regionName);
            DescribeDbInstancesResponse response = rdsClient.describeDBInstances();
            return response.dbInstances();
        } catch (RdsException e) {
            System.err.println("Error listing DB instances: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public DBInstance describeDBInstance(String regionName, String dbInstanceIdentifier) {
        if (dbInstanceIdentifier == null || dbInstanceIdentifier.isEmpty()) {
            throw new IllegalArgumentException("DB instance identifier cannot be null or empty");
        }

        try {
            RdsClient rdsClient = getRdsClient(regionName);
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

    public DBInstance createDBInstance(String regionName, String dbInstanceIdentifier, String dbInstanceClass, String engine) {
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
            RdsClient rdsClient = getRdsClient(regionName);
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

    public DBInstance createDBInstance(String regionName, String dbInstanceIdentifier, String dbInstanceClass, String engine, int allocatedStorage) {
        RdsClient rdsClient = getRdsClient(regionName);
        CreateDbInstanceRequest request = CreateDbInstanceRequest.builder()
                .dbInstanceIdentifier(dbInstanceIdentifier)
                .dbInstanceClass(dbInstanceClass)
                .engine(engine)
                .allocatedStorage(allocatedStorage)
                .build();
        CreateDbInstanceResponse response = rdsClient.createDBInstance(request);
        return response.dbInstance();
    }

    public void deleteDBInstance(String regionName, String dbInstanceIdentifier) {
        if (dbInstanceIdentifier == null || dbInstanceIdentifier.isEmpty()) {
            throw new IllegalArgumentException("DB instance identifier cannot be null or empty");
        }

        try {
            RdsClient rdsClient = getRdsClient(regionName);
            DeleteDbInstanceRequest request = DeleteDbInstanceRequest.builder()
                    .dbInstanceIdentifier(dbInstanceIdentifier)
                    .skipFinalSnapshot(true)
                    .build();
            rdsClient.deleteDBInstance(request);
        } catch (RdsException e) {
            System.err.println("Error deleting DB instance: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public void scaleDBInstance(String regionName, String dbInstanceIdentifier, String newDbInstanceClass) {
        if (dbInstanceIdentifier == null || dbInstanceIdentifier.isEmpty()) {
            throw new IllegalArgumentException("DB instance identifier cannot be null or empty");
        }
        if (newDbInstanceClass == null || newDbInstanceClass.isEmpty()) {
            throw new IllegalArgumentException("New DB instance class cannot be null or empty");
        }

        DBInstance dbInstance = describeDBInstance(regionName, dbInstanceIdentifier);
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
            RdsClient rdsClient = getRdsClient(regionName);
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

    public HashMap<String, DBInstance> listDBInstancesAsMap(String regionName) {
        try {
            RdsClient rdsClient = getRdsClient(regionName);
            DescribeDbInstancesResponse response = rdsClient.describeDBInstances();
            return response.dbInstances().stream()
                    .collect(Collectors.toMap(DBInstance::dbInstanceIdentifier, dbInstance -> dbInstance, (oldValue, newValue) -> oldValue, HashMap::new));
        } catch (RdsException e) {
            System.err.println("Error listing DB instances as map: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public List<GlobalCluster> listGlobalClusters(String regionName) {
        try {
            RdsClient rdsClient = getRdsClient(regionName);
            DescribeGlobalClustersResponse response = rdsClient.describeGlobalClusters();
            return response.globalClusters();
        } catch (RdsException e) {
            System.err.println("Error listing global clusters: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public List<GlobalClusterDetails> listGlobalClustersWithReplicationFlows(String regionName) {
        try {
            RdsClient rdsClient = getRdsClient(regionName);
            DescribeGlobalClustersResponse response = rdsClient.describeGlobalClusters();
            List<GlobalCluster> globalClusters = response.globalClusters();
            
            return globalClusters.stream().map(cluster -> {
                List<ReplicationFlow> replicationFlows = calculateReplicationFlows(cluster);
                GlobalClusterMember writer = cluster.globalClusterMembers().stream()
                    .filter(GlobalClusterMember::isWriter)
                    .findFirst()
                    .orElse(null);
                String writerRegion = writer != null ? getRegionForArn(writer.dbClusterArn()) : "unknown-region";
                double[] writerCoordinates = writer != null ? getCoordinatesForRegion(writerRegion) : new double[]{0.0, 0.0};
                return new GlobalClusterDetails(
                    cluster.globalClusterIdentifier(),
                    cluster.status(),
                    replicationFlows,
                    writerCoordinates,
                    writerRegion
                );
            }).collect(Collectors.toList());
        } catch (RdsException e) {
            System.err.println("Error listing global clusters with replication flows: " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    private double[] getCoordinatesForRegion(String region) {
        switch (region) {
            case "us-east-1":
                return new double[]{-77.0369, 38.9072}; // Example coordinates for us-east-1
            case "us-west-2":
                return new double[]{-122.3321, 47.6062}; // Example coordinates for us-west-2
            case "eu-west-1":
                return new double[]{-6.2603, 53.3498}; // Example coordinates for eu-west-1
            case "us-east-2":
                return new double[]{-82.9988, 39.9612}; // Example coordinates for us-east-2
            case "us-west-1":
                return new double[]{-121.8947, 37.3394}; // Example coordinates for us-west-1
            case "eu-central-1":
                return new double[]{8.6821, 50.1109}; // Example coordinates for eu-central-1
            case "ap-southeast-1":
                return new double[]{103.851959, 1.290270}; // Example coordinates for ap-southeast-1
            case "ap-southeast-2":
                return new double[]{151.2093, -33.8688}; // Example coordinates for ap-southeast-2
            case "ap-northeast-1":
                return new double[]{139.6917, 35.6895}; // Example coordinates for ap-northeast-1
            case "ap-northeast-2":
                return new double[]{126.9780, 37.5665}; // Example coordinates for ap-northeast-2
            case "sa-east-1":
                return new double[]{-46.6333, -23.5505}; // Example coordinates for sa-east-1
            case "ca-central-1":
                return new double[]{-75.6972, 45.4215}; // Example coordinates for ca-central-1
            case "eu-west-2":
                return new double[]{-0.1276, 51.5074}; // Example coordinates for eu-west-2
            case "eu-west-3":
                return new double[]{2.3522, 48.8566}; // Example coordinates for eu-west-3
            case "eu-north-1":
                return new double[]{18.0686, 59.3293}; // Example coordinates for eu-north-1
            case "ap-south-1":
                return new double[]{72.8777, 19.0760}; // Example coordinates for ap-south-1
            case "me-south-1":
                return new double[]{55.2708, 25.2048}; // Example coordinates for me-south-1
            // Add more cases for other regions
            default:
                return new double[]{0.0, 0.0}; // Default coordinates
        }
    }

    private String getRegionForArn(String arn) {
        // Extract region from ARN
        String[] arnParts = arn.split(":");
        if (arnParts.length > 3) {
            return arnParts[3];
        }
        return "unknown-region";
    }

    public List<DBCluster> listClusters(String regionName) {
        RdsClient rdsClient = getRdsClient(regionName);
        DescribeDbClustersResponse response = rdsClient.describeDBClusters();
        return response.dbClusters();
    }

    public Map<String, Object> getRegionDetails(String region) {
        RdsClient rdsClient = getRdsClient(region);
        DescribeDbClustersResponse response = rdsClient.describeDBClusters();
        List<DBCluster> clusters = response.dbClusters();
        return Map.of(
                "region", region,
                "clusters", clusters
        );
    }

    public void initiateFailover(String region, String clusterId, String targetDbClusterIdentifier) {
        RdsClient rdsClient = getRdsClient(region);
        rdsClient.failoverGlobalCluster(FailoverGlobalClusterRequest.builder()
                .globalClusterIdentifier(clusterId)
                .targetDbClusterIdentifier(targetDbClusterIdentifier)
                .build());
    }

    public void initiateFailback(String region, String clusterId) {
        RdsClient rdsClient = getRdsClient(region);
        rdsClient.failoverGlobalCluster(FailoverGlobalClusterRequest.builder()
                .globalClusterIdentifier(clusterId)
                .targetDbClusterIdentifier(clusterId)
                .build());
    }

    public Map<String, String> getFailoverStatus(String region, String clusterId) {
        RdsClient rdsClient = getRdsClient(region);
        DescribeGlobalClustersResponse response = rdsClient.describeGlobalClusters(DescribeGlobalClustersRequest.builder()
                .globalClusterIdentifier(clusterId)
                .build());
        GlobalCluster cluster = response.globalClusters().get(0);
        
        // Calculate replicationFlows
        List<ReplicationFlow> replicationFlows = calculateReplicationFlows(cluster);

        return Map.of(
                "status", cluster.status(),
                "replicationFlows", replicationFlows.toString()
        );
    }

    private List<ReplicationFlow> calculateReplicationFlows(GlobalCluster cluster) {
        if (cluster.globalClusterMembers() == null || cluster.globalClusterMembers().isEmpty()) {
            return List.of();
        }

        // Find the writer
        GlobalClusterMember writer = cluster.globalClusterMembers().stream()
            .filter(GlobalClusterMember::isWriter)
            .findFirst()
            .orElse(null);

        if (writer == null) {
            return List.of(); // No writer found, no replication flows
        }

        String writerRegion = getRegionForArn(writer.dbClusterArn());
        double[] writerCoordinates = getCoordinatesForRegion(writerRegion);

        // Find the readers and create replication flows
        return cluster.globalClusterMembers().stream()
            .filter(member -> !member.isWriter())
            .map(reader -> new ReplicationFlow(
                writer.dbClusterArn(),
                reader.dbClusterArn(),
                "Replication",
                writerRegion,
                getRegionForArn(reader.dbClusterArn()),
                writerCoordinates,
                getCoordinatesForRegion(getRegionForArn(reader.dbClusterArn()))
            ))
            .collect(Collectors.toList());
    }
}