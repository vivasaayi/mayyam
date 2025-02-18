package org.poriyiyal.mayyam.cloud.aws.controlplane;

import java.io.Serializable;

public class ReplicationFlow implements Serializable {
    private final String sourceArn;
    private final String targetArn;
    private final String type;
    private final String sourceRegion;
    private final String targetRegion;
    private final double[] sourceCoordinates;
    private final double[] targetCoordinates;

    public ReplicationFlow(String sourceArn, String targetArn, String type, String sourceRegion, String targetRegion, double[] sourceCoordinates, double[] targetCoordinates) {
        this.sourceArn = sourceArn;
        this.targetArn = targetArn;
        this.type = type;
        this.sourceRegion = sourceRegion;
        this.targetRegion = targetRegion;
        this.sourceCoordinates = sourceCoordinates;
        this.targetCoordinates = targetCoordinates;
    }

    public String getSourceArn() {
        return sourceArn;
    }

    public String getTargetArn() {
        return targetArn;
    }

    public String getType() {
        return type;
    }

    public String getSourceRegion() {
        return sourceRegion;
    }

    public String getTargetRegion() {
        return targetRegion;
    }

    public double[] getSourceCoordinates() {
        return sourceCoordinates;
    }

    public double[] getTargetCoordinates() {
        return targetCoordinates;
    }
}
