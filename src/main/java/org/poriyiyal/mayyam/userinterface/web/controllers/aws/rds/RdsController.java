package org.poriyiyal.mayyam.userinterface.web.controllers.aws.rds;

import org.poriyiyal.mayyam.cloud.aws.controlplane.RdsService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import software.amazon.awssdk.services.rds.model.DBInstance;
import software.amazon.awssdk.services.rds.model.DBCluster;
import java.util.List;
import java.util.HashMap;

@RestController
@RequestMapping("/api/rds")
public class RdsController {

    @Autowired
    private RdsService rdsService;

    @GetMapping("/instances")
    public ResponseEntity<?> listDBInstances(@RequestParam("region") String regionName) {
        try {
            List<DBInstance> instances = rdsService.listDBInstances(regionName);
            return ResponseEntity.ok(instances);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(e.getMessage());
        }
    }

    @GetMapping("/instances/describe")
    public ResponseEntity<?> describeDBInstance(
            @RequestParam("region") String regionName,
            @RequestParam("dbInstanceIdentifier") String dbInstanceIdentifier) {
        try {
            DBInstance instance = rdsService.describeDBInstance(regionName, dbInstanceIdentifier);
            return ResponseEntity.ok(instance);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(e.getMessage());
        }
    }

    @PostMapping("/instances/create")
    public ResponseEntity<?> createDBInstance(
            @RequestParam("region") String regionName,
            @RequestParam("dbInstanceIdentifier") String dbInstanceIdentifier,
            @RequestParam("dbInstanceClass") String dbInstanceClass,
            @RequestParam("engine") String engine) {
        try {
            DBInstance instance =
                rdsService.createDBInstance(regionName, dbInstanceIdentifier, dbInstanceClass, engine);
            return ResponseEntity.ok(instance);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(e.getMessage());
        }
    }

    @DeleteMapping("/instances/delete")
    public ResponseEntity<?> deleteDBInstance(
            @RequestParam("region") String regionName,
            @RequestParam("dbInstanceIdentifier") String dbInstanceIdentifier) {
        try {
            rdsService.deleteDBInstance(regionName, dbInstanceIdentifier);
            return ResponseEntity.ok("DB Instance deleted successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body(e.getMessage());
        }
    }

    @PostMapping("/instances/scale")
    public ResponseEntity<?> scaleDBInstance(
            @RequestParam("region") String regionName,
            @RequestParam("dbInstanceIdentifier") String dbInstanceIdentifier,
            @RequestParam("newClass") String newDbInstanceClass) {
        try {
            rdsService.scaleDBInstance(regionName, dbInstanceIdentifier, newDbInstanceClass);
            return ResponseEntity.ok("Scaling in progress");
        } catch (Exception e) {
            return ResponseEntity.status(500).body(e.getMessage());
        }
    }

    @GetMapping("/instancesAsMap")
    public ResponseEntity<?> listDBInstancesAsMap(@RequestParam("region") String regionName) {
        try {
            HashMap<String, DBInstance> instancesMap = rdsService.listDBInstancesAsMap(regionName);
            return ResponseEntity.ok(instancesMap);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(e.getMessage());
        }
    }

    @GetMapping("/globalClusters")
    public ResponseEntity<?> listGlobalClusters(@RequestParam("region") String regionName) {
        try {
            return ResponseEntity.ok(rdsService.listGlobalClusters(regionName));
        } catch (Exception e) {
            return ResponseEntity.status(500).body(e.getMessage());
        }
    }

    @GetMapping("/clusters")
    public ResponseEntity<?> listClusters(@RequestParam("region") String regionName) {
        try {
            List<DBCluster> clusters = rdsService.listClusters(regionName);
            return ResponseEntity.ok(clusters);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(e.getMessage());
        }
    }
}
