package org.poriyiyal.mayyam.cloud.aws.dataexport;

import software.amazon.awssdk.services.s3.model.Bucket;

public class BucketWithRegion {
    private final String region;
    private final Bucket bucket;

    public BucketWithRegion(String region, Bucket bucket) {
        this.region = region;
        this.bucket = bucket;
    }

    public String getRegion() {
        return region;
    }

    public Bucket getBucket() {
        return bucket;
    }
}