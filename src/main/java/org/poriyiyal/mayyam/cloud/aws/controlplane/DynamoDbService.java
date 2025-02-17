package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.dynamodb.DynamoDbClient;
import software.amazon.awssdk.services.dynamodb.model.*;

import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.stream.Collectors;

@Service
public class DynamoDbService extends BaseAwsService {

    private static final Logger logger = LoggerFactory.getLogger(DynamoDbService.class);
    private final ConcurrentMap<Region, DynamoDbClient> clientCache = new ConcurrentHashMap<>();

    private DynamoDbClient getDynamoDbClient(String region) {
        return clientCache.computeIfAbsent(Region.of(region), r -> DynamoDbClient.builder()
                .region(r)
                .credentialsProvider(credentialsProvider)
                .build());
    }

    public void createTable(String region, String tableName, List<AttributeDefinition> attributeDefinitions, List<KeySchemaElement> keySchema, ProvisionedThroughput provisionedThroughput) {
        if (region == null || region.isEmpty()) {
            throw new IllegalArgumentException("Region cannot be null or empty");
        }
        if (tableName == null || tableName.isEmpty()) {
            throw new IllegalArgumentException("Table name cannot be null or empty");
        }
        if (attributeDefinitions == null || attributeDefinitions.isEmpty()) {
            throw new IllegalArgumentException("Attribute definitions cannot be null or empty");
        }
        if (keySchema == null || keySchema.isEmpty()) {
            throw new IllegalArgumentException("Key schema cannot be null or empty");
        }
        if (provisionedThroughput == null) {
            throw new IllegalArgumentException("Provisioned throughput cannot be null");
        }

        try {
            DynamoDbClient dynamoDbClient = getDynamoDbClient(region);
            CreateTableRequest request = CreateTableRequest.builder()
                    .tableName(tableName)
                    .attributeDefinitions(attributeDefinitions)
                    .keySchema(keySchema)
                    .provisionedThroughput(provisionedThroughput)
                    .build();
            dynamoDbClient.createTable(request);
            logger.info("Table created successfully: {}", tableName);
        } catch (DynamoDbException e) {
            logger.error("Failed to create table: {}", e.getMessage());
            throw e;
        }
    }

    public void deleteTable(String region, String tableName) {
        if (region == null || region.isEmpty()) {
            throw new IllegalArgumentException("Region cannot be null or empty");
        }
        if (tableName == null || tableName.isEmpty()) {
            throw new IllegalArgumentException("Table name cannot be null or empty");
        }

        try {
            DynamoDbClient dynamoDbClient = getDynamoDbClient(region);
            DeleteTableRequest request = DeleteTableRequest.builder()
                    .tableName(tableName)
                    .build();
            dynamoDbClient.deleteTable(request);
            logger.info("Table deleted successfully: {}", tableName);
        } catch (DynamoDbException e) {
            logger.error("Failed to delete table: {}", e.getMessage());
            throw e;
        }
    }

    public Map<String, TableDescription> listTables(String region) {
        if (region == null || region.isEmpty()) {
            throw new IllegalArgumentException("Region cannot be null or empty");
        }

        try {
            DynamoDbClient dynamoDbClient = getDynamoDbClient(region);
            ListTablesResponse response = dynamoDbClient.listTables();
            return response.tableNames().stream()
                    .collect(Collectors.toMap(
                            tableName -> tableName,
                            tableName -> {
                                try {
                                    return dynamoDbClient.describeTable(DescribeTableRequest.builder()
                                            .tableName(tableName)
                                            .build()).table();
                                } catch (DynamoDbException e) {
                                    logger.error("Failed to describe table: {} - {}", tableName, e.getMessage());
                                    return null;
                                }
                            }
                    ));
        } catch (DynamoDbException e) {
            logger.error("Failed to list tables: {}", e.getMessage());
            throw e;
        }
    }

    public List<Map<String, String>> getTablesWithoutGlobalReplication(String region) {
        if (region == null || region.isEmpty()) {
            throw new IllegalArgumentException("Region cannot be null or empty");
        }

        Map<String, TableDescription> tables = listTables(region);
        return tables.entrySet().stream()
                .filter(entry -> entry.getValue().replicas() == null || entry.getValue().replicas().isEmpty())
                .map(entry -> Map.of(
                        "tableName", entry.getKey(),
                        "status", entry.getValue().tableStatusAsString()
                ))
                .collect(Collectors.toList());
    }

    public List<Map<String, String>> getTablesWithGlobalReplication(String region) {
        if (region == null || region.isEmpty()) {
            throw new IllegalArgumentException("Region cannot be null or empty");
        }

        Map<String, TableDescription> tables = listTables(region);
        return tables.entrySet().stream()
                .filter(entry -> entry.getValue().replicas() != null && !entry.getValue().replicas().isEmpty())
                .map(entry -> Map.of(
                        "tableName", entry.getKey(),
                        "status", entry.getValue().tableStatusAsString(),
                        "replicas", entry.getValue().replicas().stream()
                                .map(replica -> replica.regionName())
                                .collect(Collectors.joining(", "))
                ))
                .collect(Collectors.toList());
    }

    public Map<String, String> getTablesWithoutPITR(String region) {
        DynamoDbClient dynamoDbClient = getDynamoDbClient(region);
        Map<String, TableDescription> tables = listTables(region);
        return tables.entrySet().stream()
                .filter(entry -> {
                    DescribeContinuousBackupsResponse backupsResponse = dynamoDbClient.describeContinuousBackups(
                            DescribeContinuousBackupsRequest.builder()
                                    .tableName(entry.getKey())
                                    .build()
                    );
                    return backupsResponse.continuousBackupsDescription().pointInTimeRecoveryDescription().pointInTimeRecoveryStatus() != PointInTimeRecoveryStatus.ENABLED;
                })
                .collect(Collectors.toMap(
                        Map.Entry::getKey,
                        entry -> entry.getValue().tableStatusAsString()
                ));
    }
}