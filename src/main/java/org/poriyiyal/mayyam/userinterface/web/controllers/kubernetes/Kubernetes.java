package org.poriyiyal.mayyam.userinterface.web.controllers.kubernetes;

import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class Kubernetes {
    @RequestMapping("/kubernetes")
    public String kubernetes() {
        return "Kubernetes";
    }
}
