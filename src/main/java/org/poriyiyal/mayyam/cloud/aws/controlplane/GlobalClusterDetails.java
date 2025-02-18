package org.poriyiyal.mayyam.cloud.aws.controlplane;

import java.io.Serializable;
import java.util.List;

public class GlobalClusterDetails implements Serializable {
    private final String globalClusterId;
    private final String status;
    private final List<ReplicationFlow> replicationFlows;
    private final double[] coordinates;
    private final String region;

    public GlobalClusterDetails(String globalClusterId, String status, List<ReplicationFlow> replicationFlows, double[] coordinates, String region) {
        this.globalClusterId = globalClusterId;
        this.status = status;
        this.replicationFlows = replicationFlows;
        this.coordinates = coordinates;
        this.region = region;
    }

    public String getGlobalClusterId() {
        return globalClusterId;
    }

    public String getStatus() {
        return status;
    }

    public List<ReplicationFlow> getReplicationFlows() {
        return replicationFlows;
    }

    public double[] getCoordinates() {
        return coordinates;
    }

    public String getRegion() {
        return region;
    }
}
