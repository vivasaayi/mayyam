package org.poriyiyal.mayyam.cloud.aws.dataexport;

import software.amazon.awssdk.services.kinesis.model.StreamDescription;

public class StreamDescriptionWithRegion {
    private final String region;
    private final StreamDescription streamDescription;

    public StreamDescriptionWithRegion(String region, StreamDescription streamDescription) {
        this.region = region;
        this.streamDescription = streamDescription;
    }

    public String getRegion() {
        return region;
    }

    public StreamDescription getStreamDescription() {
        return streamDescription;
    }
}