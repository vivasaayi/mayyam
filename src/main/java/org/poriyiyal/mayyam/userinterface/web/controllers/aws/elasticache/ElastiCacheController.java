package org.poriyiyal.mayyam.userinterface.web.controllers.aws.elasticache;

import org.poriyiyal.mayyam.cloud.aws.controlplane.ElastiCacheService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import software.amazon.awssdk.services.elasticache.model.CacheCluster;
import software.amazon.awssdk.services.elasticache.model.ReplicationGroup;

import java.util.List;

@RestController
@RequestMapping("/api/elasticache")
public class ElastiCacheController {

    private final ElastiCacheService elastiCacheService;

    @Autowired
    public ElastiCacheController(ElastiCacheService elastiCacheService) {
        this.elastiCacheService = elastiCacheService;
    }

    @PostMapping("/create-cluster")
    public ResponseEntity<?> createCacheCluster(@RequestParam String clusterId, @RequestParam String nodeType, @RequestParam int numNodes, @RequestParam String engine) {
        try {
            CacheCluster cluster = elastiCacheService.createCacheCluster(clusterId, nodeType, numNodes, engine);
            return ResponseEntity.ok(cluster);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to create cache cluster: " + e.getMessage());
        }
    }

    @GetMapping("/clusters")
    public ResponseEntity<?> describeCacheClusters() {
        try {
            List<CacheCluster> clusters = elastiCacheService.describeCacheClusters();
            return ResponseEntity.ok(clusters);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to describe cache clusters: " + e.getMessage());
        }
    }

    @GetMapping("/cluster/{clusterId}")
    public ResponseEntity<?> describeCacheCluster(@PathVariable String clusterId) {
        try {
            CacheCluster cluster = elastiCacheService.describeCacheCluster(clusterId);
            return ResponseEntity.ok(cluster);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to describe cache cluster: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete-cluster/{clusterId}")
    public ResponseEntity<?> deleteCacheCluster(@PathVariable String clusterId) {
        try {
            CacheCluster cluster = elastiCacheService.deleteCacheCluster(clusterId);
            return ResponseEntity.ok(cluster);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete cache cluster: " + e.getMessage());
        }
    }

    @PutMapping("/modify-cluster")
    public ResponseEntity<?> modifyCacheCluster(@RequestParam String clusterId, @RequestParam String nodeType, @RequestParam int numNodes) {
        try {
            CacheCluster cluster = elastiCacheService.modifyCacheCluster(clusterId, nodeType, numNodes);
            return ResponseEntity.ok(cluster);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to modify cache cluster: " + e.getMessage());
        }
    }

    @GetMapping("/replication-groups")
    public ResponseEntity<?> listReplicationGroups() {
        try {
            List<ReplicationGroup> groups = elastiCacheService.listReplicationGroups();
            return ResponseEntity.ok(groups);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to list replication groups: " + e.getMessage());
        }
    }

    @GetMapping("/replication-group/{groupId}")
    public ResponseEntity<?> describeReplicationGroup(@PathVariable String groupId) {
        try {
            ReplicationGroup group = elastiCacheService.describeReplicationGroup(groupId);
            return ResponseEntity.ok(group);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to describe replication group: " + e.getMessage());
        }
    }
}