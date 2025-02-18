package org.poriyiyal.mayyam.cloud.aws.controlplane;

import javax.persistence.*;
import java.time.LocalDateTime;

@Entity
public class FailoverEvent {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    private String clusterId;
    private String sourceRegion;
    private String targetRegion;
    private String eventType;
    private String status;
    private LocalDateTime timestamp;

    public FailoverEvent() {
    }

    public FailoverEvent(String clusterId, String sourceRegion, String targetRegion, String eventType, String status) {
        this.clusterId = clusterId;
        this.sourceRegion = sourceRegion;
        this.targetRegion = targetRegion;
        this.eventType = eventType;
        this.status = status;
        this.timestamp = LocalDateTime.now();
    }

    // Getters and setters
}
