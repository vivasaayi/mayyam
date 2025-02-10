package org.poriyiyal.mayyam.userinterface.web.controllers;

import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class HealthController {
    @RequestMapping("/health")
    String home() {
        return "Hello World!";
    }
}
