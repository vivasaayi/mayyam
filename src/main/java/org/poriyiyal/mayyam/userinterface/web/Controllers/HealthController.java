package org.poriyiyal.mayyam.userinterface.web.Controllers;

import org.springframework.stereotype.Controller;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class HealthController {
    @RequestMapping("/")
    String home() {
        return "Hello World!";
    }
}
