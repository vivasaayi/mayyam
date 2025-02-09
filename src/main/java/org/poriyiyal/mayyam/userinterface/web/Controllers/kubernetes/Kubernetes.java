package org.poriyiyal.mayyam.userinterface.web.Controllers.kubernetes;

import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class Kubernetes {
    @RequestMapping("/kubernetes")
    public String kubernetes() {
        return "Kubernetes";
    }
}
