package org.poriyiyal.mayyam.userinterface.web.controllers.aws.elasticache;

import org.poriyiyal.mayyam.cloud.aws.controlplane.ElastiCacheService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import software.amazon.awssdk.services.elasticache.model.CacheCluster;
import software.amazon.awssdk.services.elasticache.model.ReplicationGroup;

import java.util.List;
import java.util.Map;

@RestController
@RequestMapping("/api/elasticache")
public class ElastiCacheController {

    private final ElastiCacheService elastiCacheService;

    @Autowired
    public ElastiCacheController(ElastiCacheService elastiCacheService) {
        this.elastiCacheService = elastiCacheService;
    }

    @PostMapping("/create")
    public ResponseEntity<String> createCacheCluster(@RequestParam String region, @RequestParam String clusterId, @RequestBody Map<String, Object> properties) {
        try {
            elastiCacheService.createCacheCluster(region, clusterId, properties);
            return ResponseEntity.ok("Cache cluster created successfully: " + clusterId);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to create cache cluster: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete")
    public ResponseEntity<String> deleteCacheCluster(@RequestParam String region, @RequestParam String clusterId) {
        try {
            elastiCacheService.deleteCacheCluster(region, clusterId);
            return ResponseEntity.ok("Cache cluster deleted successfully: " + clusterId);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete cache cluster: " + e.getMessage());
        }
    }

    @DeleteMapping("/deleteMultiple")
    public ResponseEntity<String> deleteMultipleCacheClusters(@RequestBody Map<String, String> clusterIdsAndRegions) {
        try {
            for (Map.Entry<String, String> entry : clusterIdsAndRegions.entrySet()) {
                elastiCacheService.deleteCacheCluster(entry.getValue(), entry.getKey());
            }
            return ResponseEntity.ok("Cache clusters deleted successfully");
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete cache clusters: " + e.getMessage());
        }
    }

    @GetMapping("/list")
    public ResponseEntity<List<CacheCluster>> listCacheClusters(@RequestParam String region) {
        try {
            return ResponseEntity.ok(elastiCacheService.listCacheClusters(region));
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/clusters")
    public ResponseEntity<?> describeCacheClusters(@RequestParam String region) {
        try {
            List<CacheCluster> clusters = elastiCacheService.describeCacheClusters(region);
            return ResponseEntity.ok(clusters);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to describe cache clusters: " + e.getMessage());
        }
    }

    @GetMapping("/cluster/{clusterId}")
    public ResponseEntity<?> describeCacheCluster(@RequestParam String region, @PathVariable String clusterId) {
        try {
            CacheCluster cluster = elastiCacheService.describeCacheCluster(region, clusterId);
            return ResponseEntity.ok(cluster);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to describe cache cluster: " + e.getMessage());
        }
    }

    @DeleteMapping("/delete-cluster/{clusterId}")
    public ResponseEntity<?> deleteCacheCluster(@RequestParam String region, @PathVariable String clusterId) {
        try {
            elastiCacheService.deleteCacheCluster(region, clusterId);
            return ResponseEntity.ok("Cache cluster deleted successfully: " + clusterId);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to delete cache cluster: " + e.getMessage());
        }
    }

    @PutMapping("/modify-cluster")
    public ResponseEntity<?> modifyCacheCluster(@RequestParam String region, @RequestParam String clusterId, @RequestParam String nodeType, @RequestParam int numNodes) {
        try {
            CacheCluster cluster = elastiCacheService.modifyCacheCluster(region, clusterId, nodeType, numNodes);
            return ResponseEntity.ok(cluster);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to modify cache cluster: " + e.getMessage());
        }
    }

    @GetMapping("/replication-groups")
    public ResponseEntity<?> listReplicationGroups(@RequestParam String region) {
        try {
            List<ReplicationGroup> groups = elastiCacheService.listReplicationGroups(region);
            return ResponseEntity.ok(groups);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to list replication groups: " + e.getMessage());
        }
    }

    @GetMapping("/replication-group/{groupId}")
    public ResponseEntity<?> describeReplicationGroup(@RequestParam String region, @PathVariable String groupId) {
        try {
            ReplicationGroup group = elastiCacheService.describeReplicationGroup(region, groupId);
            return ResponseEntity.ok(group);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Failed to describe replication group: " + e.getMessage());
        }
    }
}