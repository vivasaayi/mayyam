package org.poriyiyal.mayyam.cloud.aws.dataexport;

import software.amazon.awssdk.services.sqs.model.QueueAttributeName;

import java.util.Map;

public class QueueDescriptionWithRegion {
    private final String region;
    private final Map<QueueAttributeName, String> queueAttributes;

    public QueueDescriptionWithRegion(String region, Map<QueueAttributeName, String> queueAttributes) {
        this.region = region;
        this.queueAttributes = queueAttributes;
    }

    public String getRegion() {
        return region;
    }

    public Map<QueueAttributeName, String> getQueueAttributes() {
        return queueAttributes;
    }
}