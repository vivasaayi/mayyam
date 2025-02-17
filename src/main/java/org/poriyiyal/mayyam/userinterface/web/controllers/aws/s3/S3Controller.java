package org.poriyiyal.mayyam.userinterface.web.controllers.aws.s3;

import org.poriyiyal.mayyam.cloud.aws.controlplane.S3Service;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import software.amazon.awssdk.services.s3.model.Bucket;

import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

@RestController
@RequestMapping("/api/s3")
public class S3Controller {

    @Autowired
    private S3Service s3Service;

    @PostMapping("/create")
    public ResponseEntity<String> createBucket(@RequestBody Map<String, String> requestBody) {
        try {
            String bucketName = requestBody.get("bucketName");
            s3Service.createBucket(bucketName);
            return ResponseEntity.ok("Bucket created successfully: " + bucketName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to create bucket: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete")
    public ResponseEntity<String> deleteBucket(@RequestParam String bucketName) {
        try {
            s3Service.deleteBucket(bucketName);
            return ResponseEntity.ok("Bucket deleted successfully: " + bucketName);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete bucket: " + e.getMessage());
        }
    }

    @DeleteMapping("/deleteMultiple")
    public ResponseEntity<String> deleteMultipleBuckets(@RequestBody List<String> bucketNames) {
        try {
            for (String bucketName : bucketNames) {
                s3Service.deleteBucket(bucketName);
            }
            return ResponseEntity.ok("Buckets deleted successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete buckets: " + e.getMessage());
        }
    }

    @GetMapping("/list")
    public ResponseEntity<List<Bucket>> listBuckets() {
        try {
            Map<String, Bucket> bucketMap = s3Service.listBuckets();
            List<Bucket> bucketList = bucketMap.values().stream().collect(Collectors.toList());
            return ResponseEntity.ok(bucketList);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}
