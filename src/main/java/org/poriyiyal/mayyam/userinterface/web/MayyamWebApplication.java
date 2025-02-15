package org.poriyiyal.mayyam.userinterface.web;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.ComponentScan;

@SpringBootApplication
@ComponentScan(basePackages = "org.poriyiyal.mayyam")
public class MayyamWebApplication {
    public static void main(String[] args) {
        SpringApplication.run(MayyamWebApplication.class, args);
    }
}
