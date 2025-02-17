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

    public List<Map<String, String>> listTables(String region) {
        if (region == null || region.isEmpty()) {
            throw new IllegalArgumentException("Region cannot be null or empty");
        }

        try {
            DynamoDbClient dynamoDbClient = getDynamoDbClient(region);
            ListTablesResponse response = dynamoDbClient.listTables();
            return response.tableNames().stream()
                    .map(tableName -> {
                        try {
                            DescribeTableResponse describeResponse = dynamoDbClient.describeTable(DescribeTableRequest.builder()
                                    .tableName(tableName)
                                    .build());
                            return Map.of(
                                    "tableName", tableName,
                                    "tableStatus", describeResponse.table().tableStatusAsString(),
                                    "itemCount", String.valueOf(describeResponse.table().itemCount()),
                                    "tableSizeBytes", String.valueOf(describeResponse.table().tableSizeBytes())
                            );
                        } catch (DynamoDbException e) {
                            logger.error("Failed to describe table: {} - {}", tableName, e.getMessage());
                            return Map.of("tableName", tableName);
                        }
                    })
                    .collect(Collectors.toList());
        } catch (DynamoDbException e) {
            logger.error("Failed to list tables: {}", e.getMessage());
            throw e;
        }
    }

    public List<Map<String, String>> getTablesWithoutGlobalReplication(String region) {
        if (region == null || region.isEmpty()) {
            throw new IllegalArgumentException("Region cannot be null or empty");
        }

        DynamoDbClient dynamoDbClient = getDynamoDbClient(region);
        ListTablesResponse listTablesResponse = dynamoDbClient.listTables();
        return listTablesResponse.tableNames().stream()
                .map(tableName -> {
                    DescribeTableResponse describeTableResponse = dynamoDbClient.describeTable(DescribeTableRequest.builder().tableName(tableName).build());
                    if (describeTableResponse.table().replicas() == null || describeTableResponse.table().replicas().isEmpty()) {
                        return Map.of(
                                "tableName", tableName,
                                "status", describeTableResponse.table().tableStatusAsString()
                        );
                    }
                    return null;
                })
                .filter(table -> table != null)
                .collect(Collectors.toList());
    }

    public List<Map<String, String>> getTablesWithGlobalReplication(String region) {
        if (region == null || region.isEmpty()) {
            throw new IllegalArgumentException("Region cannot be null or empty");
        }

        DynamoDbClient dynamoDbClient = getDynamoDbClient(region);
        ListTablesResponse listTablesResponse = dynamoDbClient.listTables();
        return listTablesResponse.tableNames().stream()
                .map(tableName -> {
                    DescribeTableResponse describeTableResponse = dynamoDbClient.describeTable(DescribeTableRequest.builder().tableName(tableName).build());
                    if (describeTableResponse.table().replicas() != null && !describeTableResponse.table().replicas().isEmpty()) {
                        return Map.of(
                                "tableName", tableName,
                                "status", describeTableResponse.table().tableStatusAsString(),
                                "replicas", describeTableResponse.table().replicas().stream()
                                        .map(replica -> replica.regionName())
                                        .collect(Collectors.joining(", "))
                        );
                    }
                    return null;
                })
                .filter(table -> table != null)
                .collect(Collectors.toList());
    }

    // Existing functions without region parameter
    public void createTable(String tableName, List<AttributeDefinition> attributeDefinitions, List<KeySchemaElement> keySchema, ProvisionedThroughput provisionedThroughput) {
        createTable("us-west-2", tableName, attributeDefinitions, keySchema, provisionedThroughput);
    }

    public void deleteTable(String tableName) {
        deleteTable("us-west-2", tableName);
    }

    public List<Map<String, String>> listTables() {
        return listTables("us-west-2");
    }

    public List<Map<String, String>> getTablesWithoutGlobalReplication() {
        return getTablesWithoutGlobalReplication("us-west-2");
    }

    public List<Map<String, String>> getTablesWithGlobalReplication() {
        return getTablesWithGlobalReplication("us-west-2");
    }
}