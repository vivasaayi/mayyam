package org.poriyiyal.mayyam.userinterface.web.controllers.aws.rds;

import org.poriyiyal.mayyam.cloud.aws.controlplane.RdsService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;
import software.amazon.awssdk.services.rds.model.DBCluster;
import software.amazon.awssdk.services.rds.model.DBInstance;

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
    public List<DBInstance> listInstances() {
        return rdsService.listDBInstances();
    }

    @GetMapping("/clusters")
    public List<DBCluster> listClusters() {
        return rdsService.listClusters();
    }
}
