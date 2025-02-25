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
    public ResponseEntity<String> deleteMultipleQueues(@RequestParam String region, @RequestBody List<String> queueUrls) {
        try {
            for (String queueUrl : queueUrls) {
                sqsService.deleteQueue(region, queueUrl);
            }
            return ResponseEntity.ok("Queues deleted successfully in region: " + region);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete queues in region: " + e.getMessage());
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

    @GetMapping("/listWithStatus")
    public ResponseEntity<Map<String, Map<String, String>>> listQueuesWithStatus(@RequestParam String region) {
        try {
            return ResponseEntity.ok(sqsService.listQueuesWithStatus(region));
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
