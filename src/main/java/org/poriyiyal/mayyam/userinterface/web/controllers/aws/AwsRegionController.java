package org.poriyiyal.mayyam.userinterface.web.controllers.aws;

import org.poriyiyal.mayyam.cloud.aws.controlplane.AwsRegionService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;

@RestController
@RequestMapping("/api/aws/regions")
public class AwsRegionController {

    @Autowired
    private AwsRegionService awsRegionService;

    @GetMapping
    public ResponseEntity<List<String>> listRegions() {
        try {
            List<String> regions = awsRegionService.listRegions();
            return ResponseEntity.ok(regions);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}