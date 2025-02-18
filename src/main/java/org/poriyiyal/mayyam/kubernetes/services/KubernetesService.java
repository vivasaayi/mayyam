package org.poriyiyal.mayyam.kubernetes.services;

import com.fasterxml.jackson.databind.ObjectMapper;
import io.kubernetes.client.openapi.ApiClient;
import io.kubernetes.client.openapi.Configuration;
import io.kubernetes.client.openapi.apis.CoreV1Api;
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
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

@Service
public class KubernetesService {
    private static final Logger logger = LoggerFactory.getLogger(KubernetesService.class);
    private final ExecutorService executorService = Executors.newCachedThreadPool();
    private final Map<String, String> fieldsConfig = new HashMap<>();
    private final OpenSearchService openSearchService;

    public KubernetesService(OpenSearchService openSearchService) {
        this.openSearchService = openSearchService;
        loadFieldsConfig();
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
        ApiClient client = Config.defaultClient();
        Configuration.setDefaultApiClient(client);

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
    }
}
