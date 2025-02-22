package org.poriyiyal.mayyam.userinterface.web.controllers.kubernetes;

import org.poriyiyal.mayyam.kubernetes.services.KubernetesService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;

@RestController
@RequestMapping("/api/kubernetes")
public class KubernetesController {

    @Autowired
    private KubernetesService kubernetesService;

    private void validateNamespace(String namespace) {
        if (namespace == null || namespace.isEmpty()) {
            throw new IllegalArgumentException("Namespace cannot be null or empty");
        }
    }

    @GetMapping("/checkSearchDomain")
    public ResponseEntity<String> checkSearchDomain(@RequestParam String namespace, @RequestParam String searchDomain) {
        try {
            validateNamespace(namespace);
            boolean result = kubernetesService.checkSearchDomain(namespace, searchDomain);
            if (result) {
                return ResponseEntity.ok("Search domain matches.");
            } else {
                return ResponseEntity.ok("Search domain does not match.");
            }
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Error: " + e.getMessage());
        }
    }

    @GetMapping("/deployments")
    public ResponseEntity<List<Object>> getDeployments(@RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            List<Object> deployments = kubernetesService.getDeployments(namespace);
            return ResponseEntity.ok(deployments);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/cronjobs")
    public ResponseEntity<List<Object>> getCronJobs(@RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            List<Object> cronJobs = kubernetesService.getCronJobs(namespace);
            return ResponseEntity.ok(cronJobs);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/daemonsets")
    public ResponseEntity<List<Object>> getDaemonSets(@RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            List<Object> daemonSets = kubernetesService.getDaemonSets(namespace);
            return ResponseEntity.ok(daemonSets);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/statefulsets")
    public ResponseEntity<List<Object>> getStatefulSets(@RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            List<Object> statefulSets = kubernetesService.getStatefulSets(namespace);
            return ResponseEntity.ok(statefulSets);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/pvcs")
    public ResponseEntity<List<Object>> getPVCs(@RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            List<Object> pvcs = kubernetesService.getPVCs(namespace);
            return ResponseEntity.ok(pvcs);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/pvs")
    public ResponseEntity<List<Object>> getPVs() {
        try {
            List<Object> pvs = kubernetesService.getPVs();
            return ResponseEntity.ok(pvs);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/storageclasses")
    public ResponseEntity<List<Object>> getStorageClasses() {
        try {
            List<Object> storageClasses = kubernetesService.getStorageClasses();
            return ResponseEntity.ok(storageClasses);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    
    @GetMapping("/namespaces")
    public ResponseEntity<List<String>> getNamespaces() {
        try {
            List<String> namespaces = kubernetesService.getNamespaces();
            return ResponseEntity.ok(namespaces);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}
