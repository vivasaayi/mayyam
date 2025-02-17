package org.poriyiyal.mayyam.userinterface.web.controllers.aws.dynamodb;

import org.poriyiyal.mayyam.cloud.aws.controlplane.DynamoDbService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import software.amazon.awssdk.services.dynamodb.model.AttributeDefinition;
import software.amazon.awssdk.services.dynamodb.model.KeySchemaElement;
import software.amazon.awssdk.services.dynamodb.model.ProvisionedThroughput;

import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

@RestController
@RequestMapping("/api/dynamodb")
public class DynamoDbController {

    @Autowired
    private DynamoDbService dynamoDbService;

    @PostMapping("/create")
    public ResponseEntity<String> createTable(@RequestBody Map<String, Object> requestBody) {
        try {
            String tableName = (String) requestBody.get("tableName");
            List<Map<String, String>> attributeDefinitionsMap = (List<Map<String, String>>) requestBody.get("attributeDefinitions");
            List<Map<String, String>> keySchemaMap = (List<Map<String, String>>) requestBody.get("keySchema");
            Map<String, Integer> provisionedThroughputMap = (Map<String, Integer>) requestBody.get("provisionedThroughput");

            List<AttributeDefinition> attributeDefinitions = attributeDefinitionsMap.stream()
                    .map(map -> AttributeDefinition.builder()
                            .attributeName(map.get("AttributeName"))
                            .attributeType(map.get("AttributeType"))
                            .build())
                    .collect(Collectors.toList());

            List<KeySchemaElement> keySchema = keySchemaMap.stream()
                    .map(map -> KeySchemaElement.builder()
                            .attributeName(map.get("AttributeName"))
                            .keyType(map.get("KeyType"))
                            .build())
                    .collect(Collectors.toList());

            ProvisionedThroughput provisionedThroughput = ProvisionedThroughput.builder()
                    .readCapacityUnits((long) provisionedThroughputMap.get("readCapacityUnits"))
                    .writeCapacityUnits((long) provisionedThroughputMap.get("writeCapacityUnits"))
                    .build();

            dynamoDbService.createTable(tableName, attributeDefinitions, keySchema, provisionedThroughput);
            return ResponseEntity.ok("Table created successfully: " + tableName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to create table: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete")
    public ResponseEntity<String> deleteTable(@RequestParam String tableName) {
        try {
            dynamoDbService.deleteTable(tableName);
            return ResponseEntity.ok("Table deleted successfully: " + tableName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete table: " + e.getMessage());
        }
    }

    @DeleteMapping("/deleteMultiple")
    public ResponseEntity<String> deleteMultipleTables(@RequestBody String[] tableNames) {
        try {
            for (String tableName : tableNames) {
                dynamoDbService.deleteTable(tableName);
            }
            return ResponseEntity.ok("Tables deleted successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete tables: " + e.getMessage());
        }
    }

    @GetMapping("/list")
    public ResponseEntity<List<Map<String, String>>> listTables() {
        try {
            return ResponseEntity.ok(dynamoDbService.listTables());
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/tablesWithoutReplication")
    public ResponseEntity<List<Map<String, String>>> getTablesWithoutGlobalReplication() {
        try {
            return ResponseEntity.ok(dynamoDbService.getTablesWithoutGlobalReplication());
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/tablesWithReplication")
    public ResponseEntity<List<Map<String, String>>> getTablesWithGlobalReplication() {
        try {
            return ResponseEntity.ok(dynamoDbService.getTablesWithGlobalReplication());
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}
