package org.poriyiyal.mayyam.cloud.aws.controlplane;

import software.amazon.awssdk.auth.credentials.ProfileCredentialsProvider;
import software.amazon.awssdk.regions.Region;

public abstract class BaseAwsService {
    protected final Region region = Region.US_EAST_1;
    protected final ProfileCredentialsProvider credentialsProvider = ProfileCredentialsProvider.create();

    // Common methods for AWS services can be added here
}