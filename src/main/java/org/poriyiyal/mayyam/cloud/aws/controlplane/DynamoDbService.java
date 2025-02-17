package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.springframework.stereotype.Service;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.dynamodb.DynamoDbClient;
import software.amazon.awssdk.services.dynamodb.model.*;

import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

@Service
public class DynamoDbService extends BaseAwsService {

    private DynamoDbClient getDynamoDbClient(String region) {
        return DynamoDbClient.builder()
                .region(Region.of(region))
                .credentialsProvider(credentialsProvider)
                .build();
    }

    public void createTable(String region, String tableName, List<AttributeDefinition> attributeDefinitions, List<KeySchemaElement> keySchema, ProvisionedThroughput provisionedThroughput) {
        if (tableName == null || tableName.isEmpty()) {
            throw new IllegalArgumentException("Table name cannot be null or empty");
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
            System.out.println("Table created successfully: " + tableName);
        } catch (DynamoDbException e) {
            System.err.println("Failed to create table: " + e.getMessage());
            throw e;
        }
    }

    public void deleteTable(String region, String tableName) {
        if (tableName == null || tableName.isEmpty()) {
            throw new IllegalArgumentException("Table name cannot be null or empty");
        }

        try {
            DynamoDbClient dynamoDbClient = getDynamoDbClient(region);
            DeleteTableRequest request = DeleteTableRequest.builder()
                    .tableName(tableName)
                    .build();
            dynamoDbClient.deleteTable(request);
            System.out.println("Table deleted successfully: " + tableName);
        } catch (DynamoDbException e) {
            System.err.println("Failed to delete table: " + e.getMessage());
            throw e;
        }
    }

    public List<Map<String, String>> listTables(String region) {
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
                            System.err.println("Failed to describe table: " + tableName + " - " + e.getMessage());
                            return Map.of("tableName", tableName);
                        }
                    })
                    .collect(Collectors.toList());
        } catch (DynamoDbException e) {
            System.err.println("Failed to list tables: " + e.getMessage());
            throw e;
        }
    }

    public List<Map<String, String>> getTablesWithoutGlobalReplication(String region) {
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
}