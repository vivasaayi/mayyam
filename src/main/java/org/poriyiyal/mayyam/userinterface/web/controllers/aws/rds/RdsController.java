package org.poriyiyal.mayyam.userinterface.web.controllers.aws.rds;

import org.poriyiyal.mayyam.cloud.aws.controlplane.RdsService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import software.amazon.awssdk.services.rds.model.DBCluster;
import software.amazon.awssdk.services.rds.model.DBInstance;

import jakarta.validation.Valid;
import java.util.List;

@RestController
@RequestMapping("/api/rds")
public class RdsController {

    private final RdsService rdsService;

    @Autowired
    public RdsController(RdsService rdsService) {
        this.rdsService = rdsService;
    }

    @GetMapping("/instances")
    public ResponseEntity<List<DBInstance>> listInstances() {
        List<DBInstance> instances = rdsService.listDBInstances();
        return ResponseEntity.ok(instances);
    }

    @GetMapping("/clusters")
    public List<DBCluster> listClusters() {
        return rdsService.listClusters();
    }

    @PostMapping("/create")
    public ResponseEntity<?> createInstance(@Valid @RequestBody CreateInstanceRequest request) {
        try {
            DBInstance instance = rdsService.createDBInstance(
                    request.getDbInstanceIdentifier(),
                    request.getDbInstanceClass(),
                    request.getEngine(),
                    request.getAllocatedStorage()
            );
            return ResponseEntity.ok(instance);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to create DB instance: " + e.getMessage());
        }
    }
}
