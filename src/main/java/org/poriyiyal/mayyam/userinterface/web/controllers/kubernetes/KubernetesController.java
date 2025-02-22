package org.poriyiyal.mayyam.userinterface.web.controllers.kubernetes;

import org.poriyiyal.mayyam.kubernetes.services.KubernetesService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/api/kubernetes")
public class KubernetesController {

    @Autowired
    private KubernetesService kubernetesService;

    @GetMapping("/checkSearchDomain")
    public ResponseEntity<String> checkSearchDomain(@RequestParam String namespace, @RequestParam String searchDomain) {
        try {
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
}
