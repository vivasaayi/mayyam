package org.poriyiyal.mayyam.userinterface.web.controllers.kubernetes;

import org.poriyiyal.mayyam.kubernetes.services.KubernetesService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;

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
    public ResponseEntity<List<Map<String, String>>> checkSearchDomain(@RequestParam String namespace, @RequestParam String searchDomain) {
        try {
            validateNamespace(namespace);
            List<Map<String, String>> result = kubernetesService.checkSearchDomain(namespace, searchDomain);
            return ResponseEntity.ok(result);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
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

    @GetMapping("/pods")
    public ResponseEntity<List<Object>> getPods(@RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            List<Object> pods = kubernetesService.getPods(namespace);
            return ResponseEntity.ok(pods);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/pod-details")
    public ResponseEntity<Object> getPodDetails(@RequestParam String podName, @RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            Object podDetails = kubernetesService.getPodDetails(podName, namespace);
            return ResponseEntity.ok(podDetails);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Error: " + e.getMessage());
        }
    }

    @GetMapping("/pod-events")
    public ResponseEntity<List<Object>> getPodEvents(@RequestParam String podName, @RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            List<Object> podEvents = kubernetesService.getPodEvents(podName, namespace);
            return ResponseEntity.ok(podEvents);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }

    @GetMapping("/pod-status")
    public ResponseEntity<Object> getPodStatus(@RequestParam String podName, @RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            Object podStatus = kubernetesService.getPodStatus(podName, namespace);
            return ResponseEntity.ok(podStatus);
        } catch (Exception e) {
            return ResponseEntity.status(500).body("Error: " + e.getMessage());
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

    @GetMapping("/services")
    public ResponseEntity<List<Object>> getServices(@RequestParam String namespace) {
        try {
            validateNamespace(namespace);
            List<Object> services = kubernetesService.getServices(namespace);
            return ResponseEntity.ok(services);
        } catch (Exception e) {
            return ResponseEntity.status(500).body(null);
        }
    }
}
