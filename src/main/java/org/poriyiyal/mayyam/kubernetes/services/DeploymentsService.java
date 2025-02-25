package org.poriyiyal.mayyam.kubernetes.services;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

import io.kubernetes.client.openapi.apis.AppsV1Api;
import io.kubernetes.client.openapi.models.V1Deployment;
import io.kubernetes.client.openapi.models.V1DeploymentList;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

@Service
public class DeploymentsService {
    private static final Logger logger = LoggerFactory.getLogger(DeploymentsService.class);

    public void getDeployments(AppsV1Api appsV1Api) {
        // Implement method to get deployments
        // Example: Use Kubernetes API to list deployments
        try {
            V1DeploymentList deploymentList = appsV1Api.listNamespacedDeployment("default", null, false, null, null, null, null, null, null, 0, false);
            for (V1Deployment deployment : deploymentList.getItems()) {
                logger.info("Deployment: {}", deployment.getMetadata().getName());
            }
        } catch (Exception e) {
            logger.error("Error getting deployments: {}", e.getMessage());
        }
    }


    public List<Object> getDeployments(AppsV1Api appsV1Api, String namespace) throws Exception {
        V1DeploymentList deploymentList = appsV1Api.listNamespacedDeployment(namespace, null, null, null, null, null, null, null, null, null, null);
        return deploymentList.getItems().stream().map(deployment -> {
            Map<String, Object> deploymentMap = new HashMap<>();
            deploymentMap.put("name", deployment.getMetadata().getName());
            deploymentMap.put("expectedReplicas", deployment.getSpec().getReplicas());
            deploymentMap.put("podsRunning", deployment.getStatus().getReadyReplicas());
            deploymentMap.put("podsPending", deployment.getStatus().getUnavailableReplicas());
            deploymentMap.put("podsNotStarted", deployment.getStatus().getReplicas() - deployment.getStatus().getReadyReplicas());
            return deploymentMap;
        }).collect(Collectors.toList());
    }
}
