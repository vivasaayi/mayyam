package org.poriyiyal.mayyam.userinterface.web.controllers.aws.kinesis;

import org.poriyiyal.mayyam.cloud.aws.controlplane.KinesisService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import software.amazon.awssdk.services.kinesis.model.StreamDescription;

import java.util.Map;

@RestController
@RequestMapping("/api/kinesis")
public class KinesisController {

    @Autowired
    private KinesisService kinesisService;

    @PostMapping("/create")
    public ResponseEntity<String> createStream(@RequestParam String streamName, @RequestParam int shardCount) {
        try {
            kinesisService.createStream(streamName, shardCount);
            return ResponseEntity.ok("Stream created successfully: " + streamName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to create stream: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete")
    public ResponseEntity<String> deleteStream(@RequestParam String streamName) {
        try {
            kinesisService.deleteStream(streamName);
            return ResponseEntity.ok("Stream deleted successfully: " + streamName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete stream: " + e.getMessage());
        }
    }

    @DeleteMapping("/deleteMultiple")
    public ResponseEntity<String> deleteMultipleStreams(@RequestBody String[] streamNames) {
        try {
            for (String streamName : streamNames) {
                kinesisService.deleteStream(streamName);
            }
            return ResponseEntity.ok("Streams deleted successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete streams: " + e.getMessage());
        }
    }

    @GetMapping("/list")
    public ResponseEntity<Map<String, StreamDescription>> listStreams() {
        try {
            return ResponseEntity.ok(kinesisService.listStreams());
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/describe")
    public ResponseEntity<StreamDescription> describeStream(@RequestParam String streamName) {
        try {
            return ResponseEntity.ok(kinesisService.describeStream(streamName));
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}
