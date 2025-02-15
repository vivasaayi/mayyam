package org.poriyiyal.mayyam.config;

import org.poriyiyal.mayyam.cloud.aws.controlplane.RdsService;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

@Configuration
public class AwsConfig {

    @Bean
    public RdsService rdsService() {
        return new RdsService();
    }
}
