package org.poriyiyal.mayyam.kubernetes.services;

import io.kubernetes.client.openapi.ApiClient;
import io.kubernetes.client.openapi.apis.AppsV1Api;
import io.kubernetes.client.openapi.apis.CoreV1Api;
import io.kubernetes.client.openapi.models.*;
import io.kubernetes.client.util.Config;
import org.apache.poi.ss.usermodel.Row;
import org.apache.poi.ss.usermodel.Sheet;
import org.apache.poi.ss.usermodel.Workbook;
import org.apache.poi.xssf.usermodel.XSSFWorkbook;
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
import java.util.stream.Collectors;

import io.kubernetes.client.openapi.apis.BatchV1beta1Api;
import io.kubernetes.client.openapi.apis.StorageV1Api;
import io.kubernetes.client.openapi.ApiException;

// Add the missing import


@Service
public class KubernetesService {
    private static final Logger logger = LoggerFactory.getLogger(KubernetesService.class);
    private final ExecutorService executorService = Executors.newCachedThreadPool();

    private final Map<String, String> fieldsConfig = new HashMap<>();
    private final CoreV1Api coreV1Api;
    private final AppsV1Api appsV1Api;
    private final BatchV1beta1Api batchV1beta1Api;
    private final StorageV1Api storageV1Api;

    private final DeploymentsService deploymentsService;
    private final PodsService podsService;

    public KubernetesService(DeploymentsService deploymentsService, PodsService podsService) throws IOException {
        this.deploymentsService = deploymentsService;
        this.podsService = podsService;
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
            pvcMap.put("status", pvc.getStatus().getPhase());
            pvcMap.put("volume", pvc.getSpec().getVolumeName());
            if (pvc.getStatus() != null && pvc.getStatus().getCapacity() != null && pvc.getStatus().getCapacity().get("storage") != null) {
                pvcMap.put("capacity", pvc.getStatus().getCapacity().get("storage").toString());
            } else {
                pvcMap.put("capacity", "N/A");
            }
            pvcMap.put("accessModes", pvc.getSpec().getAccessModes().toString());
            pvcMap.put("storageClass", pvc.getSpec().getStorageClassName());
            pvcMap.put("volumeAttributesClass", pvc.getSpec().getVolumeMode());
            pvcMap.put("age", pvc.getMetadata().getCreationTimestamp().toString());
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
            storageClassMap.put("provisioner", storageClass.getProvisioner());
            storageClassMap.put("reclaimPolicy", storageClass.getReclaimPolicy());
            storageClassMap.put("volumeBindingMode", storageClass.getVolumeBindingMode());
            storageClassMap.put("allowVolumeExpansion", storageClass.getAllowVolumeExpansion());
            storageClassMap.put("age", storageClass.getMetadata().getCreationTimestamp().toString());
            return storageClassMap;
        }).collect(Collectors.toList());
    }

    public List<String> getNamespaces() throws Exception {
        V1NamespaceList namespaceList = coreV1Api.listNamespace(null, null, null, null, null, null, null, null, null, null);
        return namespaceList.getItems().stream()
                .map(V1Namespace::getMetadata)
                .map(metadata -> metadata.getName())
                .collect(Collectors.toList());
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

    public List<Object> getDeployments(String namespace) throws Exception {
        return deploymentsService.getDeployments(appsV1Api, namespace);
    }

    public List<Object> getPods(String namespace) throws Exception {
        return podsService.getPods(coreV1Api, namespace);
    }


    public List<Map<String, String>> checkSearchDomain(String namespace, String searchDomain) throws Exception {
        return podsService.checkSearchDomain(coreV1Api, namespace, searchDomain);
    }

    public Object getPodDetails(String podName, String namespace) throws ApiException {
        return podsService.getPodDetails(coreV1Api, podName, namespace);
    }

    public List<Object> getPodEvents(String podName, String namespace) throws ApiException {
        return podsService.getPodEvents(coreV1Api, podName, namespace);
    }

    public Object getPodStatus(String podName, String namespace) throws ApiException {
        return podsService.getPodStatus(coreV1Api, podName, namespace);
    }
}
