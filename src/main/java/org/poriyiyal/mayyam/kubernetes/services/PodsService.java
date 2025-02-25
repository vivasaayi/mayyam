package org.poriyiyal.mayyam.kubernetes.services;

import java.io.IOException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Scanner;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.stream.Collectors;

import com.fasterxml.jackson.databind.ObjectMapper;

import io.kubernetes.client.openapi.ApiException;
import io.kubernetes.client.openapi.apis.CoreV1Api;
import io.kubernetes.client.openapi.models.CoreV1EventList;
import io.kubernetes.client.openapi.models.V1Pod;
import io.kubernetes.client.openapi.models.V1PodList;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okio.ByteString;
import org.poriyiyal.mayyam.opensearch.OpenSearchService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;


@Service
class PodsService {
    private final Map<String, String> fieldsConfig = new HashMap<>();
    private final OpenSearchService openSearchService = new OpenSearchService();
    private final Logger logger = LoggerFactory.getLogger(PodsService.class);

    private final ExecutorService threadPool = Executors.newFixedThreadPool(10);

    private void followPodLogs(CoreV1Api api, V1Pod pod) {
        threadPool.submit(() -> {
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

    public List<Object> getPods(CoreV1Api coreV1Api, String namespace) throws ApiException {
        V1PodList podList = coreV1Api.listNamespacedPod(namespace, null, null, null, null, null, null, null, null, null, false);
        List<Object> pods = new ArrayList<>();
        for (V1Pod pod : podList.getItems()) {
            Map<String, Object> podInfo = new HashMap<>();
            podInfo.put("name", pod.getMetadata().getName());
            podInfo.put("status", pod.getStatus().getPhase());
            pods.add(podInfo);
        }
        return pods;
    }

    public List<Map<String, String>> checkSearchDomain(CoreV1Api coreV1Api, String namespace, String searchDomain) throws Exception {
        List<Map<String, String>> result = new ArrayList<>();
        result.addAll(checkAllPods(coreV1Api, namespace, searchDomain));
        return result;
    }

    private List<Map<String, String>> checkAllPods(CoreV1Api coreV1Api, String namespace, String searchDomain) throws Exception {
        List<Map<String, String>> result = new ArrayList<>();
        V1PodList podList = coreV1Api.listNamespacedPod(namespace, null, null, null, null, null, null, null, null, null, false);
        List<Future<Map<String, String>>> futures = new ArrayList<>();

        for (V1Pod pod : podList.getItems()) {
            if (pod.getStatus().getPhase().equals("Running")) {
                futures.add(threadPool.submit(() -> {
                    String podName = pod.getMetadata().getName();
                    String resolvConf = execCommand(namespace, podName, "cat /etc/resolv.conf");
                    String currentSearchDomain = parseResolvConf(resolvConf);
                    Map<String, String> podInfo = new HashMap<>();
                    podInfo.put("podName", podName);
                    podInfo.put("currentSearchDomain", currentSearchDomain);
                    return podInfo;
                }));
            }
        }

        for (Future<Map<String, String>> future : futures) {
            result.add(future.get());
        }

        return result;
    }

    private String execCommand(String namespace, String podName, String command) throws IOException {
        ProcessBuilder processBuilder = new ProcessBuilder("kubectl", "exec", "-n", namespace, podName, "--", "sh", "-c", command);
        Process process = processBuilder.start();
        try (Scanner scanner = new Scanner(process.getInputStream()).useDelimiter("\\A")) {
            return scanner.hasNext() ? scanner.next() : "";
        }
    }

    private String parseResolvConf(String resolvConf) {
        String[] lines = resolvConf.split("\n");
        for (String line : lines) {
            if (line.startsWith("search")) {
                return line.substring(7).trim(); // Remove "search " prefix and trim
            }
        }
        return "";
    }

    public Object getPodDetails(CoreV1Api coreV1Api,String podName, String namespace) throws ApiException {
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

    public List<Object> getPodEvents(CoreV1Api coreV1Api, String podName, String namespace) throws ApiException {
        try {
            return fetchPodEvents(coreV1Api, podName, namespace, null);
        } catch (ApiException e) {
            if (e.getCode() == 400 && e.getResponseBody().contains("invalid continue token")) {
                logger.error("Invalid continue token: {}", e.getResponseBody());
                // Retry without the continue token
                return fetchPodEvents(coreV1Api, podName, namespace, null);
            } else {
                logger.error("Error fetching pod events: {}", e.getMessage());
                throw e;
            }
        }
    }

    private List<Object> fetchPodEvents(CoreV1Api coreV1Api, String podName, String namespace, String continueToken) throws ApiException {
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

    public Object getPodStatus(CoreV1Api coreV1Api, String podName, String namespace) throws ApiException {
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