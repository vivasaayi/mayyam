package org.poriyiyal.mayyam.userinterface.web.controllers.aws.sqs;

import org.poriyiyal.mayyam.cloud.aws.controlplane.SqsService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import software.amazon.awssdk.services.sqs.model.QueueAttributeName;

import java.util.List;
import java.util.Map;

@RestController
@RequestMapping("/api/sqs")
public class SqsController {

    @Autowired
    private SqsService sqsService;

    @PostMapping("/create")
    public ResponseEntity<String> createQueue(@RequestParam String region, @RequestParam String queueName) {
        try {
            sqsService.createQueue(region, queueName);
            return ResponseEntity.ok("Queue created successfully: " + queueName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to create queue: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete")
    public ResponseEntity<String> deleteQueue(@RequestParam String region, @RequestParam String queueUrl) {
        try {
            sqsService.deleteQueue(region, queueUrl);
            return ResponseEntity.ok("Queue deleted successfully: " + queueUrl);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete queue: " + e.getMessage());
        }
    }

    @DeleteMapping("/deleteMultiple")
    public ResponseEntity<String> deleteMultipleQueues(@RequestBody Map<String, String> queueUrlsAndRegions) {
        try {
            for (Map.Entry<String, String> entry : queueUrlsAndRegions.entrySet()) {
                sqsService.deleteQueue(entry.getValue(), entry.getKey());
            }
            return ResponseEntity.ok("Queues deleted successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete queues: " + e.getMessage());
        }
    }

    @GetMapping("/list")
    public ResponseEntity<Map<String, String>> listQueues(@RequestParam String region) {
        try {
            return ResponseEntity.ok(sqsService.listQueues(region));
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/attributes")
    public ResponseEntity<Map<QueueAttributeName, String>> getQueueAttributes(@RequestParam String region, @RequestParam String queueUrl) {
        try {
            return ResponseEntity.ok(sqsService.getQueueAttributes(region, queueUrl));
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}
