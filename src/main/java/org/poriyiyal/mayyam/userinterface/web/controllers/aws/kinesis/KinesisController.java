package org.poriyiyal.mayyam.userinterface.web.controllers.aws.kinesis;

import org.poriyiyal.mayyam.cloud.aws.controlplane.KinesisService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.kinesis.model.StreamDescription;

import java.util.HashMap;
import java.util.Map;

@RestController
@RequestMapping("/api/kinesis")
public class KinesisController {

    @Autowired
    private KinesisService kinesisService;

    @PostMapping("/create")
    public ResponseEntity<String> createStream(@RequestParam String region, @RequestParam String streamName, @RequestParam int shardCount) {
        try {
            kinesisService.createStream(region, streamName, shardCount);
            return ResponseEntity.ok("Stream created successfully: " + streamName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to create stream: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete")
    public ResponseEntity<String> deleteStream(@RequestParam String region, @RequestParam String streamName) {
        try {
            kinesisService.deleteStream(region, streamName);
            return ResponseEntity.ok("Stream deleted successfully: " + streamName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete stream: " + e.getMessage());
        }
    }

    @DeleteMapping("/deleteMultiple")
    public ResponseEntity<String> deleteMultipleStreams(@RequestBody Map<String, String> streamNamesAndRegions) {
        try {
            for (Map.Entry<String, String> entry : streamNamesAndRegions.entrySet()) {
                kinesisService.deleteStream(entry.getValue(), entry.getKey());
            }
            return ResponseEntity.ok("Streams deleted successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete streams: " + e.getMessage());
        }
    }

    @GetMapping("/list")
    public ResponseEntity<Map<String, StreamDescription>> listStreams(@RequestParam String region) {
        try {
            return ResponseEntity.ok(kinesisService.listStreams(region));
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/listAllRegions")
    public ResponseEntity<Map<String, Object>> listStreamsFromAllRegions() {
        Map<String, Object> allStreams = new HashMap<>();
        Map<String, String> errors = new HashMap<>();
        for (Region region : Region.regions()) {
            try {
                allStreams.putAll(kinesisService.listStreams(region.id()));
            } catch (Exception e) {
                errors.put(region.id(), e.getMessage());
            }
        }
        allStreams.put("errors", errors);
        return ResponseEntity.ok(allStreams);
    }

    @GetMapping("/describe")
    public ResponseEntity<StreamDescription> describeStream(@RequestParam String region, @RequestParam String streamName) {
        try {
            return ResponseEntity.ok(kinesisService.describeStream(region, streamName));
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}
