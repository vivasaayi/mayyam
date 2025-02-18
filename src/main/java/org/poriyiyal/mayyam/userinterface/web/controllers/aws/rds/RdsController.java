package org.poriyiyal.mayyam.userinterface.web.controllers.aws.rds;

import org.poriyiyal.mayyam.cloud.aws.controlplane.GlobalClusterDetails;
import org.poriyiyal.mayyam.cloud.aws.controlplane.RdsService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import software.amazon.awssdk.services.rds.model.DBInstance;
import software.amazon.awssdk.services.rds.model.DBCluster;
import software.amazon.awssdk.services.rds.model.GlobalCluster;
import java.util.List;
import java.io.Serializable;
import java.util.HashMap;
import java.util.Map;

@RestController
@RequestMapping("/api/rds")
public class RdsController {

    private final RdsService rdsService;

    @Autowired
    public RdsController(RdsService rdsService) {
        this.rdsService = rdsService;
    }

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

    @GetMapping("/clusters")
    public ResponseEntity<?> listClusters(@RequestParam("region") String regionName) {
        try {
            List<DBCluster> clusters = rdsService.listClusters(regionName);
            return ResponseEntity.ok(clusters);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(e.getMessage());
        }
    }

    @GetMapping("/global-clusters")
    public ResponseEntity<List<GlobalCluster>> listGlobalClusters(@RequestParam String region) {
        try {
            List<GlobalCluster> globalClusters = rdsService.listGlobalClusters(region);
            return ResponseEntity.ok(globalClusters);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/global-clusters-with-replication-flows")
    public ResponseEntity<List<GlobalClusterDetails>> listGlobalClustersWithReplicationFlows(@RequestParam String region) {
        try {
            List<GlobalClusterDetails> globalClusters = rdsService.listGlobalClustersWithReplicationFlows(region);
            return ResponseEntity.ok(globalClusters);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/region-details")
    public ResponseEntity<Map<String, Object>> getRegionDetails(@RequestParam String region) {
        try {
            Map<String, Object> regionDetails = rdsService.getRegionDetails(region);
            return ResponseEntity.ok(regionDetails);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @PostMapping("/failover")
    public ResponseEntity<String> initiateFailover(
            @RequestParam String region,
            @RequestParam String clusterId,
            @RequestParam String targetRegion,
            @RequestParam String targetDbClusterIdentifier) {
        try {
            rdsService.initiateFailover(region, clusterId, targetDbClusterIdentifier);
            return ResponseEntity.ok("Failover initiated successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to initiate failover: " + e.getMessage());
        }
    }

    @PostMapping("/failback")
    public ResponseEntity<String> initiateFailback(@RequestParam String region, @RequestParam String clusterId) {
        try {
            rdsService.initiateFailback(region, clusterId);
            return ResponseEntity.ok("Failback initiated successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to initiate failback: " + e.getMessage());
        }
    }

    @GetMapping("/failover-status")
    public ResponseEntity<Map<String, String>> getFailoverStatus(@RequestParam String region, @RequestParam String clusterId) {
        try {
            Map<String, String> status = rdsService.getFailoverStatus(region, clusterId);
            return ResponseEntity.ok(status);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}
