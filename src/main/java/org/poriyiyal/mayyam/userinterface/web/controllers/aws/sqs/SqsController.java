package org.poriyiyal.mayyam.userinterface.web.controllers.aws.sqs;

import org.poriyiyal.mayyam.cloud.aws.controlplane.SqsService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;

@RestController
@RequestMapping("/api/sqs")
public class SqsController {

    @Autowired
    private SqsService sqsService;

    @PostMapping("/create")
    public ResponseEntity<String> createQueue(@RequestParam String queueName) {
        try {
            sqsService.createQueue(queueName);
            return ResponseEntity.ok("Queue created successfully: " + queueName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to create queue: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete")
    public ResponseEntity<String> deleteQueue(@RequestParam String queueUrl) {
        try {
            sqsService.deleteQueue(queueUrl);
            return ResponseEntity.ok("Queue deleted successfully: " + queueUrl);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete queue: " + e.getMessage());
        }
    }

    @DeleteMapping("/deleteMultiple")
    public ResponseEntity<String> deleteMultipleQueues(@RequestBody String[] queueUrls) {
        try {
            for (String queueUrl : queueUrls) {
                sqsService.deleteQueue(queueUrl);
            }
            return ResponseEntity.ok("Queues deleted successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete queues: " + e.getMessage());
        }
    }

    @GetMapping("/list")
    public ResponseEntity<List<Map<String, String>>> listQueues() {
        try {
            return ResponseEntity.ok(sqsService.listQueues());
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}
