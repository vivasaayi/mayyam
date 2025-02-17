package org.poriyiyal.mayyam.userinterface.web.controllers.aws.dynamodb;

import org.poriyiyal.mayyam.cloud.aws.controlplane.DynamoDbService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import software.amazon.awssdk.services.dynamodb.model.*;

import java.util.List;
import java.util.Map;

@RestController
@RequestMapping("/api/dynamodb")
public class DynamoDbController {

    private static final Logger logger = LoggerFactory.getLogger(DynamoDbController.class);

    @Autowired
    private DynamoDbService dynamoDbService;

    @PostMapping("/create")
    public ResponseEntity<String> createTable(@RequestParam String region, @RequestParam String tableName, @RequestBody Map<String, Object> properties) {
        try {
            List<AttributeDefinition> attributeDefinitions = (List<AttributeDefinition>) properties.get("attributeDefinitions");
            List<KeySchemaElement> keySchema = (List<KeySchemaElement>) properties.get("keySchema");
            ProvisionedThroughput provisionedThroughput = (ProvisionedThroughput) properties.get("provisionedThroughput");
            dynamoDbService.createTable(region, tableName, attributeDefinitions, keySchema, provisionedThroughput);
            return ResponseEntity.ok("Table created successfully: " + tableName);
        } catch (Exception e) {
            logger.error("Failed to create table: {}", e.getMessage());
            return ResponseEntity.status(500).body("Failed to create table: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete")
    public ResponseEntity<String> deleteTable(@RequestParam String region, @RequestParam String tableName) {
        try {
            dynamoDbService.deleteTable(region, tableName);
            return ResponseEntity.ok("Table deleted successfully: " + tableName);
        } catch (Exception e) {
            logger.error("Failed to delete table: {}", e.getMessage());
            return ResponseEntity.status(500).body("Failed to delete table: " + e.getMessage());
        }
    }

    @DeleteMapping("/deleteMultiple")
    public ResponseEntity<String> deleteMultipleTables(@RequestParam String region, @RequestBody String[] tableNames) {
        try {
            for (String tableName : tableNames) {
                dynamoDbService.deleteTable(region, tableName);
            }
            return ResponseEntity.ok("Tables deleted successfully");
        } catch (Exception e) {
            logger.error("Failed to delete tables: {}", e.getMessage());
            return ResponseEntity.status(500).body("Failed to delete tables: " + e.getMessage());
        }
    }

    @GetMapping("/list")
    public ResponseEntity<List<TableDescription>> listTables(@RequestParam String region) {
        try {
            return ResponseEntity.ok(dynamoDbService.listTables(region));
        } catch (Exception e) {
            logger.error("Failed to list tables: {}", e.getMessage());
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/tablesWithoutReplication")
    public ResponseEntity<List<Map<String, String>>> getTablesWithoutGlobalReplication(@RequestParam String region) {
        try {
            return ResponseEntity.ok(dynamoDbService.getTablesWithoutGlobalReplication(region));
        } catch (Exception e) {
            logger.error("Failed to get tables without global replication: {}", e.getMessage());
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/tablesWithReplication")
    public ResponseEntity<List<Map<String, String>>> getTablesWithGlobalReplication(@RequestParam String region) {
        try {
            return ResponseEntity.ok(dynamoDbService.getTablesWithGlobalReplication(region));
        } catch (Exception e) {
            logger.error("Failed to get tables with global replication: {}", e.getMessage());
            return ResponseEntity.status(500).body(null);
        }
    }

    @PostMapping("/createDefault")
    public ResponseEntity<String> createTable(@RequestParam String tableName, @RequestBody Map<String, Object> properties) {
        return createTable("us-west-2", tableName, properties);
    }

    @DeleteMapping("/deleteDefault")
    public ResponseEntity<String> deleteTable(@RequestParam String tableName) {
        return deleteTable("us-west-2", tableName);
    }

    @DeleteMapping("/deleteMultipleDefault")
    public ResponseEntity<String> deleteMultipleTables(@RequestBody String[] tableNames) {
        return deleteMultipleTables("us-west-2", tableNames);
    }

    @GetMapping("/listDefault")
    public ResponseEntity<List<TableDescription>> listTables() {
        return listTables("us-west-2");
    }

    @GetMapping("/tablesWithoutReplicationDefault")
    public ResponseEntity<List<Map<String, String>>> getTablesWithoutGlobalReplication() {
        return getTablesWithoutGlobalReplication("us-west-2");
    }

    @GetMapping("/tablesWithReplicationDefault")
    public ResponseEntity<List<Map<String, String>>> getTablesWithGlobalReplication() {
        return getTablesWithGlobalReplication("us-west-2");
    }
}
