package org.poriyiyal.mayyam.userinterface.web;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.ComponentScan;
import org.springframework.data.jpa.repository.config.EnableJpaRepositories;
import org.springframework.boot.autoconfigure.domain.EntityScan;


@SpringBootApplication
@ComponentScan(basePackages = "org.poriyiyal.mayyam.*")
@EnableJpaRepositories(basePackages = "org.poriyiyal.mayyam.*")
@EntityScan(basePackages = "org.poriyiyal.mayyam.ticketmanagement.entity.*")
public class MayyamWebApplication {
    public static void main(String[] args) {
        SpringApplication.run(MayyamWebApplication.class, args);
    }
}
