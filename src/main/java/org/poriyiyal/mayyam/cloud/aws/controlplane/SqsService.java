package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.springframework.stereotype.Service;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.sqs.SqsClient;
import software.amazon.awssdk.services.sqs.model.*;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.Map;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.Executors;
import java.util.concurrent.ExecutorService;
import java.util.stream.Collectors;

@Service
public class SqsService extends BaseAwsService {

    private static final Logger logger = LoggerFactory.getLogger(SqsService.class);
    private final ConcurrentMap<Region, SqsClient> clientCache = new ConcurrentHashMap<>();

    private SqsClient getSqsClient(String region) {
        return clientCache.computeIfAbsent(Region.of(region), r -> SqsClient.builder()
                .region(r)
                .credentialsProvider(credentialsProvider)
                .build());
    }

    public void createQueue(String region, String queueName) {
        if (queueName == null || queueName.isEmpty()) {
            throw new IllegalArgumentException("Queue name cannot be null or empty");
        }

        try {
            SqsClient sqsClient = getSqsClient(region);
            CreateQueueRequest request = CreateQueueRequest.builder()
                    .queueName(queueName)
                    .build();
            sqsClient.createQueue(request);
            System.out.println("Queue created successfully: " + queueName);
        } catch (SqsException e) {
            System.err.println("Failed to create queue: " + e.getMessage());
            throw e;
        }
    }

    public void deleteQueue(String region, String queueUrl) {
        if (queueUrl == null || queueUrl.isEmpty()) {
            throw new IllegalArgumentException("Queue URL cannot be null or empty");
        }

        try {
            SqsClient sqsClient = getSqsClient(region);
            DeleteQueueRequest request = DeleteQueueRequest.builder()
                    .queueUrl(queueUrl)
                    .build();
            sqsClient.deleteQueue(request);
            System.out.println("Queue deleted successfully: " + queueUrl);
        } catch (SqsException e) {
            System.err.println("Failed to delete queue: " + e.getMessage());
            throw e;
        }
    }

    public Map<String, String> listQueues(String region) {
        try {
            SqsClient sqsClient = getSqsClient(region);
            ListQueuesResponse response = sqsClient.listQueues();
            return response.queueUrls().stream()
                    .collect(Collectors.toMap(
                            queueUrl -> queueUrl.substring(queueUrl.lastIndexOf('/') + 1),
                            queueUrl -> queueUrl
                    ));
        } catch (SqsException e) {
            System.err.println("Failed to list queues: " + e.getMessage());
            throw e;
        }
    }

    public String getQueueStatus(String region, String queueUrl) {
        try {
            SqsClient sqsClient = getSqsClient(region);
            GetQueueAttributesRequest request = GetQueueAttributesRequest.builder()
                    .queueUrl(queueUrl)
                    .attributeNames(QueueAttributeName.QUEUE_ARN)
                    .build();
            GetQueueAttributesResponse response = sqsClient.getQueueAttributes(request);
            // Assuming the queue is active if it has an ARN
            return response.attributes().containsKey(QueueAttributeName.QUEUE_ARN) ? "Active" : "Inactive";
        } catch (SqsException e) {
            System.err.println("Failed to get queue status: " + e.getMessage());
            throw e;
        }
    }

    public Map<QueueAttributeName, String> getQueueAttributes(String region, String queueUrl) {
        if (queueUrl == null || queueUrl.isEmpty()) {
            throw new IllegalArgumentException("Queue URL cannot be null or empty");
        }

        try {
            SqsClient sqsClient = getSqsClient(region);
            GetQueueAttributesRequest request = GetQueueAttributesRequest.builder()
                    .queueUrl(queueUrl)
                    .attributeNames(QueueAttributeName.ALL)
                    .build();
            GetQueueAttributesResponse response = sqsClient.getQueueAttributes(request);
            return response.attributes();
        } catch (SqsException e) {
            System.err.println("Failed to get queue attributes: " + e.getMessage());
            throw e;
        }
    }

    public String getQueueUrl(String region, String queueName) {
        if (queueName == null || queueName.isEmpty()) {
            throw new IllegalArgumentException("Queue name cannot be null or empty");
        }

        try {
            SqsClient sqsClient = getSqsClient(region);
            GetQueueUrlRequest request = GetQueueUrlRequest.builder()
                    .queueName(queueName)
                    .build();
            GetQueueUrlResponse response = sqsClient.getQueueUrl(request);
            return response.queueUrl();
        } catch (SqsException e) {
            System.err.println(e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public Map<String, Map<QueueAttributeName, String>> listAllQueuesWithDetails(String region) {
        List<String> allQueueUrls = listAllQueues(region);
        Map<String, Map<QueueAttributeName, String>> queueDetailsMap = new HashMap<>();

        List<CompletableFuture<Void>> futures = allQueueUrls.stream()
                .map(queueUrl -> CompletableFuture.runAsync(() -> {
                    Map<QueueAttributeName, String> details = getQueueAttributes(region, queueUrl);
                    synchronized (queueDetailsMap) {
                        queueDetailsMap.put(queueUrl, details);
                    }
                }))
                .collect(Collectors.toList());

        for (CompletableFuture<Void> future : futures) {
            try {
                future.get();
            } catch (InterruptedException | ExecutionException e) {
                System.err.println("Error retrieving queue details: " + e.getMessage());
            }
        }

        return queueDetailsMap;
    }

    public List<String> listAllQueues(String region) {
        List<String> allQueueUrls = new ArrayList<>();
        String nextToken = null;

        do {
            try {
                SqsClient sqsClient = getSqsClient(region);
                ListQueuesRequest request = ListQueuesRequest.builder()
                        .nextToken(nextToken)
                        .build();
                ListQueuesResponse response = sqsClient.listQueues(request);
                allQueueUrls.addAll(response.queueUrls());
                nextToken = response.nextToken();
            } catch (SqsException e) {
                System.err.println(e.awsErrorDetails().errorMessage());
                throw e;
            }
        } while (nextToken != null);

        return allQueueUrls;
    }

    public List<Map<QueueAttributeName, String>> listAllQueuesParallel(String region) {
        List<String> allQueueUrls = listAllQueues(region);

        List<CompletableFuture<Map<QueueAttributeName, String>>> futures = allQueueUrls.stream()
                .map(queueUrl -> CompletableFuture.supplyAsync(() -> getQueueAttributes(region, queueUrl)))
                .collect(Collectors.toList());

        List<Map<QueueAttributeName, String>> queueDetails = new ArrayList<>();
        for (CompletableFuture<Map<QueueAttributeName, String>> future : futures) {
            try {
                queueDetails.add(future.get());
            } catch (InterruptedException | ExecutionException e) {
                System.err.println("Error retrieving queue details: " + e.getMessage());
            }
        }

        return queueDetails;
    }

    public List<Map<String, String>> listQueuesWithDetails(String region) {
        try {
            Map<String, Map<QueueAttributeName, String>> queueDetailsMap = listAllQueuesWithDetails(region);
            System.out.println("Queue details map: " + queueDetailsMap); // Add logging
            return queueDetailsMap.entrySet().stream()
                    .map(entry -> {
                        Map<String, String> queueDetails = new HashMap<>();
                        queueDetails.put("queueUrl", entry.getKey());
                        queueDetails.put("queueName", entry.getKey().substring(entry.getKey().lastIndexOf('/') + 1));
                        queueDetails.put("attributes", entry.getValue().toString());
                        return queueDetails;
                    })
                    .collect(Collectors.toList());
        } catch (Exception e) {
            System.err.println("Failed to list queues: " + e.getMessage());
            e.printStackTrace();
            return List.of();
        }
    }

    public Map<String, Map<String, String>> listQueuesWithStatus(String region) {
        ExecutorService executorService = Executors.newFixedThreadPool(5);
        try {
            logger.debug("Fetching SQS client for region: {}", region);
            SqsClient sqsClient = getSqsClient(region);
            logger.debug("Listing queues in region: {}", region);
            ListQueuesResponse response = sqsClient.listQueues();
            List<CompletableFuture<Map.Entry<String, Map<String, String>>>> futures = response.queueUrls().stream()
                    .map(queueUrl -> CompletableFuture.supplyAsync(() -> {
                        String queueName = queueUrl.substring(queueUrl.lastIndexOf('/') + 1);
                        logger.debug("Fetching status for queue: {}", queueName);
                        Map<String, String> details = new HashMap<>();
                        details.put("queueUrl", queueUrl);
                        details.put("status", getQueueStatus(region, queueUrl));
                        return Map.entry(queueName, details);
                    }, executorService))
                    .collect(Collectors.toList());

            Map<String, Map<String, String>> result = new HashMap<>();
            for (CompletableFuture<Map.Entry<String, Map<String, String>>> future : futures) {
                try {
                    Map.Entry<String, Map<String, String>> entry = future.get();
                    result.put(entry.getKey(), entry.getValue());
                    logger.debug("Queue status fetched: {} - {}", entry.getKey(), entry.getValue().get("status"));
                } catch (InterruptedException | ExecutionException e) {
                    logger.error("Error retrieving queue status: {}", e.getMessage());
                }
            }
            logger.debug("Completed listing queues with status in region: {}", region);
            return result;
        } catch (SqsException e) {
            logger.error("Failed to list queues with status: {}", e.getMessage());
            throw e;
        } finally {
            executorService.shutdown();
        }
    }
}