package org.poriyiyal.mayyam.kubernetes.services;

import com.fasterxml.jackson.databind.ObjectMapper;
import io.kubernetes.client.openapi.ApiClient;
import io.kubernetes.client.openapi.apis.AppsV1Api;
import io.kubernetes.client.openapi.apis.CoreV1Api;
import io.kubernetes.client.openapi.models.V1DaemonSet;
import io.kubernetes.client.openapi.models.V1DaemonSetList;
import io.kubernetes.client.openapi.models.V1Deployment;
import io.kubernetes.client.openapi.models.V1DeploymentList;
import io.kubernetes.client.openapi.models.V1Pod;
import io.kubernetes.client.openapi.models.V1PodList;
import io.kubernetes.client.util.Config;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okio.ByteString;
import org.apache.poi.ss.usermodel.Row;
import org.apache.poi.ss.usermodel.Sheet;
import org.apache.poi.ss.usermodel.Workbook;
import org.apache.poi.xssf.usermodel.XSSFWorkbook;
import org.poriyiyal.mayyam.opensearch.OpenSearchService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.io.FileInputStream;
import java.io.IOException;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Scanner;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.stream.Collectors;

import io.kubernetes.client.openapi.models.V1StatefulSetList;
import io.kubernetes.client.openapi.models.V1PersistentVolumeClaimList;
import io.kubernetes.client.openapi.models.V1PersistentVolumeList;
import io.kubernetes.client.openapi.models.V1StorageClassList;
import io.kubernetes.client.openapi.apis.BatchV1beta1Api;
import io.kubernetes.client.openapi.apis.StorageV1Api;
import io.kubernetes.client.openapi.models.V1beta1CronJobList;
import io.kubernetes.client.openapi.models.V1Namespace;
import io.kubernetes.client.openapi.models.V1NamespaceList;
import io.kubernetes.client.openapi.ApiException;
import java.util.ArrayList;
import io.kubernetes.client.openapi.models.V1ServiceList;
import io.kubernetes.client.openapi.models.CoreV1EventList;

@Service
public class KubernetesService {
    private static final Logger logger = LoggerFactory.getLogger(KubernetesService.class);
    private final ExecutorService executorService = Executors.newCachedThreadPool();
    private final Map<String, String> fieldsConfig = new HashMap<>();
    private final OpenSearchService openSearchService;
    private final CoreV1Api coreV1Api;
    private final AppsV1Api appsV1Api;
    private final BatchV1beta1Api batchV1beta1Api;
    private final StorageV1Api storageV1Api;

    public KubernetesService(OpenSearchService openSearchService) throws IOException {
        this.openSearchService = openSearchService;
        loadFieldsConfig();
        ApiClient client;
        String kubeConfigPath = System.getenv("KUBECONFIG_PATH");
        if (kubeConfigPath != null && !kubeConfigPath.isEmpty()) {
            client = Config.fromConfig(kubeConfigPath);
        } else {
            client = Config.defaultClient();
        }
        io.kubernetes.client.openapi.Configuration.setDefaultApiClient(client);
        this.coreV1Api = new CoreV1Api();
        this.appsV1Api = new AppsV1Api();
        this.batchV1beta1Api = new BatchV1beta1Api();
        this.storageV1Api = new StorageV1Api();
    }

    private void loadFieldsConfig() {
        try (FileInputStream fis = new FileInputStream("fields_config.xlsx");
             Workbook workbook = new XSSFWorkbook(fis)) {
            Sheet sheet = workbook.getSheetAt(0);
            for (Row row : sheet) {
                String fieldName = row.getCell(0).getStringCellValue();
                String fieldType = row.getCell(1).getStringCellValue();
                fieldsConfig.put(fieldName, fieldType);
            }
        } catch (IOException e) {
            logger.error("Error loading fields config: {}", e.getMessage());
        }
    }

    public void initialize() throws Exception {
        CoreV1Api api = new CoreV1Api();
        V1PodList list = api.listPodForAllNamespaces(
                null, null, null, null, null,
                null, null, null, null, null);
        for (V1Pod item : list.getItems()) {
            followPodLogs(api, item);
        }
    }

    private void followPodLogs(CoreV1Api api, V1Pod pod) {
        executorService.submit(() -> {
            try {
                String namespace = pod.getMetadata().getNamespace();
                String podName = pod.getMetadata().getName();
                String containerName = pod.getSpec().getContainers().get(0).getName();

                OkHttpClient httpClient = new OkHttpClient();
                Request request = new Request.Builder()
                        .url(api.getApiClient().getBasePath() + "/api/v1/namespaces/" + namespace + "/pods/" + podName + "/log?container=" + containerName + "&follow=true")
                        .build();

                httpClient.newWebSocket(request, new WebSocketListener() {
                    @Override
                    public void onMessage(WebSocket webSocket, String text) {
                        parseAndStoreLog(text);
                    }

                    @Override
                    public void onMessage(WebSocket webSocket, ByteString bytes) {
                        parseAndStoreLog(bytes.utf8());
                    }

                    @Override
                    public void onFailure(WebSocket webSocket, Throwable t, Response response) {
                        logger.error("Error following logs for pod {}: {}", podName, t.getMessage());
                    }
                });
            } catch (Exception e) {
                logger.error("Error following logs for pod {}: {}", pod.getMetadata().getName(), e.getMessage());
            }
        });
    }

    @SuppressWarnings("unchecked")
    private void parseAndStoreLog(String log) {
        try {
            Map<String, Object> parsedLog = new HashMap<>();
            if (isJson(log)) {
                Map<String, Object> logMap = new ObjectMapper().readValue(log, Map.class);
                for (Map.Entry<String, String> entry : fieldsConfig.entrySet()) {
                    String fieldName = entry.getKey();
                    String fieldType = entry.getValue();
                    Object value = logMap.get(fieldName);
                    if (value != null) {
                        parsedLog.put(fieldName, convertValue(value, fieldType));
                    }
                }
            } else {
                // Handle non-JSON log
                // Example: Split log by lines and process each line
            }
            // Push parsed logs to OpenSearch
            openSearchService.bulkIndex("logs", List.of(parsedLog));
        } catch (Exception e) {
            logger.error("Error parsing log: {}", e.getMessage());
        }
    }

    private Object convertValue(Object value, String fieldType) {
        switch (fieldType.toLowerCase()) {
            case "string":
                return value.toString();
            case "integer":
                return Integer.parseInt(value.toString());
            case "double":
                return Double.parseDouble(value.toString());
            case "boolean":
                return Boolean.parseBoolean(value.toString());
            default:
                return value;
        }
    }

    private boolean isJson(String log) {
        try {
            new ObjectMapper().readTree(log);
            return true;
        } catch (IOException e) {
            return false;
        }
    }

    public void getDeployments() {
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

    public boolean checkSearchDomain(String namespace, String searchDomain) throws Exception {
        boolean result = false;
        result |= checkDeployments(namespace, searchDomain);
        result |= checkDaemonSets(namespace, searchDomain);
        return result;
    }

    private boolean checkDeployments(String namespace, String searchDomain) throws Exception {
        V1DeploymentList deploymentList = appsV1Api.listNamespacedDeployment(namespace, null, null, null, null, null, null, null, null, null, null);
        for (V1Deployment deployment : deploymentList.getItems()) {
            if (checkPods(namespace, deployment.getMetadata().getName(), searchDomain)) {
                return true;
            }
        }
        return false;
    }

    private boolean checkDaemonSets(String namespace, String searchDomain) throws Exception {
        V1DaemonSetList daemonSetList = appsV1Api.listNamespacedDaemonSet(namespace, null, null, null, null, null, null, null, null, null, null);
        for (V1DaemonSet daemonSet : daemonSetList.getItems()) {
            if (checkPods(namespace, daemonSet.getMetadata().getName(), searchDomain)) {
                return true;
            }
        }
        return false;
    }

    private boolean checkPods(String namespace, String name, String searchDomain) throws Exception {
        V1PodList podList = coreV1Api.listNamespacedPod(namespace, null, null, null, "metadata.ownerReferences[0].name=" + name, null, null, null, null, null, false);
        for (V1Pod pod : podList.getItems()) {
            if (pod.getStatus().getPhase().equals("Running")) {
                String podName = pod.getMetadata().getName();
                String resolvConf = execCommand(namespace, podName, "cat /etc/resolv.conf");
                if (parseResolvConf(resolvConf, searchDomain)) {
                    return true;
                }
            }
        }
        return false;
    }

    private String execCommand(String namespace, String podName, String command) throws IOException {
        ProcessBuilder processBuilder = new ProcessBuilder("kubectl", "exec", "-n", namespace, podName, "--", "sh", "-c", command);
        Process process = processBuilder.start();
        try (Scanner scanner = new Scanner(process.getInputStream()).useDelimiter("\\A")) {
            return scanner.hasNext() ? scanner.next() : "";
        }
    }

    private boolean parseResolvConf(String resolvConf, String searchDomain) {
        String[] lines = resolvConf.split("\n");
        for (String line : lines) {
            if (line.startsWith("search")) {
                String[] domains = line.split(" ");
                for (String domain : domains) {
                    if (domain.equals(searchDomain)) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    public List<Object> getDeployments(String namespace) throws Exception {
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

    public List<Object> getCronJobs(String namespace) throws Exception {
        V1beta1CronJobList cronJobList = batchV1beta1Api.listNamespacedCronJob(namespace, null, null, null, null, null, null, null, null, null, null);
        return cronJobList.getItems().stream().map(cronJob -> {
            Map<String, Object> cronJobMap = new HashMap<>();
            cronJobMap.put("name", cronJob.getMetadata().getName());
            return cronJobMap;
        }).collect(Collectors.toList());
    }

    public List<Object> getDaemonSets(String namespace) throws Exception {
        V1DaemonSetList daemonSetList = appsV1Api.listNamespacedDaemonSet(namespace, null, null, null, null, null, null, null, null, null, null);
        return daemonSetList.getItems().stream().map(daemonSet -> {
            Map<String, Object> daemonSetMap = new HashMap<>();
            daemonSetMap.put("name", daemonSet.getMetadata().getName());
            return daemonSetMap;
        }).collect(Collectors.toList());
    }

    public List<Object> getStatefulSets(String namespace) throws Exception {
        V1StatefulSetList statefulSetList = appsV1Api.listNamespacedStatefulSet(namespace, null, null, null, null, null, null, null, null, null, null);
        return statefulSetList.getItems().stream().map(statefulSet -> {
            Map<String, Object> statefulSetMap = new HashMap<>();
            statefulSetMap.put("name", statefulSet.getMetadata().getName());
            return statefulSetMap;
        }).collect(Collectors.toList());
    }

    public List<Object> getPVCs(String namespace) throws Exception {
        V1PersistentVolumeClaimList pvcList = coreV1Api.listNamespacedPersistentVolumeClaim(namespace, null, null, null, null, null, null, null, null, null, null);
        return pvcList.getItems().stream().map(pvc -> {
            Map<String, Object> pvcMap = new HashMap<>();
            pvcMap.put("name", pvc.getMetadata().getName());
            return pvcMap;
        }).collect(Collectors.toList());
    }

    public List<Object> getPVs() throws Exception {
        V1PersistentVolumeList pvList = coreV1Api.listPersistentVolume(null, null, null, null, null, null, null, null, null, null);
        return pvList.getItems().stream().map(pv -> {
            Map<String, Object> pvMap = new HashMap<>();
            pvMap.put("name", pv.getMetadata().getName());
            return pvMap;
        }).collect(Collectors.toList());
    }

    public List<Object> getStorageClasses() throws Exception {
        V1StorageClassList storageClassList = storageV1Api.listStorageClass(null, null, null, null, null, null, null, null, null, null);
        return storageClassList.getItems().stream().map(storageClass -> {
            Map<String, Object> storageClassMap = new HashMap<>();
            storageClassMap.put("name", storageClass.getMetadata().getName());
            return storageClassMap;
        }).collect(Collectors.toList());
    }

    public List<String> getNamespaces() {
        try {
            V1NamespaceList namespaceList = coreV1Api.listNamespace(null, null, null, null, null, null, null, null, null, null);
            return namespaceList.getItems().stream()
                    .map(V1Namespace::getMetadata)
                    .map(metadata -> metadata.getName())
                    .collect(Collectors.toList());
        } catch (Exception e) {
            throw new RuntimeException("Error fetching namespaces: " + e.getMessage(), e);
        }
    }

    public List<Object> getPods(String namespace) throws ApiException {
        try {
            V1PodList podList = coreV1Api.listNamespacedPod(namespace, null, null, null, null, null, null, null, null, null, false);
            List<Object> pods = new ArrayList<>();
            for (V1Pod pod : podList.getItems()) {
                Map<String, Object> podInfo = new HashMap<>();
                podInfo.put("name", pod.getMetadata().getName());
                podInfo.put("status", pod.getStatus().getPhase());
                pods.add(podInfo);
            }
            return pods;
        } catch (ApiException e) {
            if (e.getCode() == 400 && e.getResponseBody().contains("invalid continue token")) {
                logger.error("Invalid continue token: {}", e.getResponseBody());
            } else {
                logger.error("Error fetching pods: {}", e.getMessage());
            }
            throw e;
        }
    }

    public Object getPodDetails(String podName, String namespace) throws ApiException {
        V1Pod pod = coreV1Api.readNamespacedPod(podName, namespace, null);
        Map<String, Object> podDetails = new HashMap<>();
        podDetails.put("name", pod.getMetadata().getName());
        podDetails.put("namespace", pod.getMetadata().getNamespace());
        podDetails.put("status", pod.getStatus().getPhase());
        podDetails.put("containers", pod.getSpec().getContainers().stream().map(container -> {
            Map<String, Object> containerDetails = new HashMap<>();
            containerDetails.put("name", container.getName());
            containerDetails.put("envVars", container.getEnv().stream().map(envVar -> {
                Map<String, String> envVarDetails = new HashMap<>();
                envVarDetails.put("name", envVar.getName());
                envVarDetails.put("value", envVar.getValue());
                return envVarDetails;
            }).collect(Collectors.toList()));
            containerDetails.put("volumes", pod.getSpec().getVolumes().stream().map(volume -> {
                Map<String, String> volumeDetails = new HashMap<>();
                volumeDetails.put("name", volume.getName());
                if (volume.getHostPath() != null) {
                    volumeDetails.put("mountPath", volume.getHostPath().getPath());
                } else {
                    volumeDetails.put("mountPath", "N/A");
                }
                return volumeDetails;
            }).collect(Collectors.toList()));
            return containerDetails;
        }).collect(Collectors.toList()));
        return podDetails;
    }

    public List<Object> getServices(String namespace) throws ApiException {
        V1ServiceList serviceList = coreV1Api.listNamespacedService(namespace, null, null, null, null, null, null, null, null, null, false);
        return serviceList.getItems().stream().map(service -> {
            Map<String, Object> serviceMap = new HashMap<>();
            serviceMap.put("name", service.getMetadata().getName());
            serviceMap.put("type", service.getSpec().getType());
            serviceMap.put("clusterIP", service.getSpec().getClusterIP());
            return serviceMap;
        }).collect(Collectors.toList());
    }

    public List<Object> getPodEvents(String podName, String namespace) throws ApiException {
        try {
            return fetchPodEvents(podName, namespace, null);
        } catch (ApiException e) {
            if (e.getCode() == 400 && e.getResponseBody().contains("invalid continue token")) {
                logger.error("Invalid continue token: {}", e.getResponseBody());
                // Retry without the continue token
                return fetchPodEvents(podName, namespace, null);
            } else {
                logger.error("Error fetching pod events: {}", e.getMessage());
                throw e;
            }
        }
    }

    private List<Object> fetchPodEvents(String podName, String namespace, String continueToken) throws ApiException {
        CoreV1EventList eventList = coreV1Api.listNamespacedEvent(
            namespace, 
            null, 
            null, 
            continueToken, 
            "involvedObject.name=" + podName, 
            null, 
            null, 
            null, 
            null, 
            null, 
            false
        );
        return eventList.getItems().stream().map(event -> {
            Map<String, Object> eventMap = new HashMap<>();
            eventMap.put("type", event.getType());
            eventMap.put("reason", event.getReason());
            eventMap.put("message", event.getMessage());
            eventMap.put("firstTimestamp", event.getFirstTimestamp());
            eventMap.put("lastTimestamp", event.getLastTimestamp());
            return eventMap;
        }).collect(Collectors.toList());
    }

    public Object getPodStatus(String podName, String namespace) throws ApiException {
        V1Pod pod = coreV1Api.readNamespacedPod(podName, namespace, null);
        Map<String, Object> statusMap = new HashMap<>();
        statusMap.put("phase", pod.getStatus().getPhase());
        statusMap.put("conditions", pod.getStatus().getConditions().stream().map(condition -> {
            Map<String, Object> conditionMap = new HashMap<>();
            conditionMap.put("type", condition.getType());
            conditionMap.put("status", condition.getStatus());
            conditionMap.put("lastTransitionTime", condition.getLastTransitionTime());
            return conditionMap;
        }).collect(Collectors.toList()));
        return statusMap;
    }
}
