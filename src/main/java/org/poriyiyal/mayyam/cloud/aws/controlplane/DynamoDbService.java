package org.poriyiyal.mayyam.cloud.aws.controlplane;

import software.amazon.awssdk.services.dynamodb.model.CreateTableRequest;
import java.util.List;
import software.amazon.awssdk.services.dynamodb.model.DeleteTableRequest;
import software.amazon.awssdk.services.dynamodb.model.ListTablesRequest;
import software.amazon.awssdk.services.dynamodb.model.ListTablesResponse;
import software.amazon.awssdk.services.dynamodb.model.DescribeTableRequest;
import software.amazon.awssdk.services.dynamodb.model.DescribeTableResponse;
import software.amazon.awssdk.services.dynamodb.model.AttributeDefinition;
import software.amazon.awssdk.services.dynamodb.model.KeySchemaElement;
import software.amazon.awssdk.services.dynamodb.model.ProvisionedThroughput;

import software.amazon.awssdk.services.dynamodb.DynamoDbClient;

public class DynamoDbService extends BaseAwsService {
    private final DynamoDbClient dynamoDbClient;

    public DynamoDbService() {
        this.dynamoDbClient = DynamoDbClient.builder()
                .region(region)
                .credentialsProvider(credentialsProvider)
                .build();
    }

    public void createTable(String tableName, List<AttributeDefinition> attributeDefinitions, List<KeySchemaElement> keySchema, ProvisionedThroughput provisionedThroughput) {
        if (tableName == null || tableName.isEmpty()) {
            throw new IllegalArgumentException("Table name must be provided");
        }

        try {
            CreateTableRequest request = CreateTableRequest.builder()
                .tableName(tableName)
                .attributeDefinitions(attributeDefinitions)
                .keySchema(keySchema)
                .provisionedThroughput(provisionedThroughput)
                .build();
            dynamoDbClient.createTable(request);
        } catch (Exception e) {
            // Handle the exception appropriately
            System.err.println("Failed to create table: " + e.getMessage());
        }
    }

    public void deleteTable(String tableName) {
        try {
            DeleteTableRequest request = DeleteTableRequest.builder()
                .tableName(tableName)
                .build();
            dynamoDbClient.deleteTable(request);
        } catch (Exception e) {
            // Handle the exception appropriately
            System.err.println("Failed to delete table: " + e.getMessage());
        }
    }

    public ListTablesResponse listTables() {
        try {
            ListTablesRequest request = ListTablesRequest.builder().build();
            return dynamoDbClient.listTables(request);
        } catch (Exception e) {
            // Handle the exception appropriately
            System.err.println("Failed to describe table: " + e.getMessage());
            return null;
        }
    }
    
    public DescribeTableResponse describeTable(String tableName) {
        DescribeTableRequest request = DescribeTableRequest.builder()
                .tableName(tableName)
                .build();
        return dynamoDbClient.describeTable(request);
    }
}