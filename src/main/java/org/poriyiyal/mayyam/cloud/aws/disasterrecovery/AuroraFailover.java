package org.poriyiyal.mayyam.cloud.aws.disasterrecovery;

import com.amazonaws.services.rds.AmazonRDS;
import com.amazonaws.services.rds.AmazonRDSClientBuilder;
import com.amazonaws.services.rds.model.FailoverGlobalClusterRequest;
import com.amazonaws.services.rds.model.GlobalCluster;
import com.amazonaws.services.rds.model.DescribeGlobalClustersRequest;
import com.amazonaws.services.rds.model.DescribeGlobalClustersResult;
import java.util.List;

public class AuroraFailover {

    private final AmazonRDS rdsClient;

    public AuroraFailover() {
        this.rdsClient = AmazonRDSClientBuilder.defaultClient();
        List<GlobalCluster> globalClusters = rdsClient.describeGlobalClusters(new DescribeGlobalClustersRequest()).getGlobalClusters();
        List<String> globalClusterIdentifiers = List.of("your-cluster-identifier-1", "your-cluster-identifier-2"); // Add your actual cluster identifiers here
        String secondaryRegion = "your-secondary-region"; // Define your secondary region here
        for (String globalClusterIdentifier : globalClusterIdentifiers) {
            boolean exists = globalClusters.stream()
                    .anyMatch(gc -> gc.getGlobalClusterIdentifier().equals(globalClusterIdentifier));
            if (exists) {
                failoverCluster(globalClusterIdentifier, secondaryRegion);
            } else {
                System.out.println("Global cluster with identifier " + globalClusterIdentifier + " does not exist.");
            }
        }
    }

    public void failoverAllClusters(List<GlobalCluster> globalClusters, String secondaryRegion) {
        for (GlobalCluster globalCluster : globalClusters) {
            failoverCluster(globalCluster.getGlobalClusterIdentifier(), secondaryRegion);
        }
    }

    private void failoverCluster(String globalClusterIdentifier, String secondaryRegion) {
        FailoverGlobalClusterRequest request = new FailoverGlobalClusterRequest()
                .withGlobalClusterIdentifier(globalClusterIdentifier)
                .withTargetDbClusterIdentifier(secondaryRegion);

        rdsClient.failoverGlobalCluster(request);

        // Wait for the cluster status to be available after failover
        boolean isFailoverComplete = false;
        while (!isFailoverComplete) {
            GlobalCluster cluster = rdsClient.describeGlobalClusters(new DescribeGlobalClustersRequest())
                    .getGlobalClusters()
                    .stream()
                    .filter(gc -> gc.getGlobalClusterIdentifier().equals(globalClusterIdentifier))
                    .findFirst()
                    .orElse(null);

            if (cluster != null && "available".equals(cluster.getStatus())) {
                isFailoverComplete = true;
            } else {
                try {
                    Thread.sleep(5000); // Wait for 5 seconds before checking again
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new RuntimeException("Thread was interrupted", e);
                }
            }
        }

        rdsClient.failoverGlobalCluster(request);
    }
}
