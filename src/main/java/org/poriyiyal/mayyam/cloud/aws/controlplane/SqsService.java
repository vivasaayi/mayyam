package org.poriyiyal.mayyam.cloud.aws.controlplane;

import software.amazon.awssdk.services.sqs.SqsClient;
import software.amazon.awssdk.services.sqs.model.CreateQueueRequest;
import software.amazon.awssdk.services.sqs.model.CreateQueueResponse;
import software.amazon.awssdk.services.sqs.model.DeleteQueueRequest;
import software.amazon.awssdk.services.sqs.model.GetQueueUrlRequest;
import software.amazon.awssdk.services.sqs.model.GetQueueUrlResponse;
import software.amazon.awssdk.services.sqs.model.SqsException;
import software.amazon.awssdk.services.sqs.model.ListQueuesRequest;
import software.amazon.awssdk.services.sqs.model.ListQueuesResponse;
import software.amazon.awssdk.services.sqs.model.GetQueueAttributesRequest;
import software.amazon.awssdk.services.sqs.model.GetQueueAttributesResponse;
import software.amazon.awssdk.services.sqs.model.QueueAttributeName;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.Map;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.stream.Collectors;

public class SqsService extends BaseAwsService {
    private final SqsClient sqsClient;

    public SqsService() {
        this.sqsClient = SqsClient.builder()
                .region(region)
                .credentialsProvider(credentialsProvider)
                .build();
    }

    public String createQueue(String queueName) {
        if (queueName == null || queueName.isEmpty()) {
            throw new IllegalArgumentException("Queue name cannot be null or empty");
        }

        try {
            CreateQueueRequest request = CreateQueueRequest.builder()
                    .queueName(queueName)
                    .build();
            CreateQueueResponse response = sqsClient.createQueue(request);
            return response.queueUrl();
        } catch (SqsException e) {
            System.err.println(e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public String getQueueUrl(String queueName) {
        if (queueName == null || queueName.isEmpty()) {
            throw new IllegalArgumentException("Queue name cannot be null or empty");
        }

        try {
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

    public void deleteQueue(String queueUrl) {
        if (queueUrl == null || queueUrl.isEmpty()) {
            throw new IllegalArgumentException("Queue URL cannot be null or empty");
        }

        try {
            DeleteQueueRequest request = DeleteQueueRequest.builder()
                    .queueUrl(queueUrl)
                    .build();
            sqsClient.deleteQueue(request);
        } catch (SqsException e) {
            System.err.println(e.awsErrorDetails().errorMessage());
            throw e;
        }
    }

    public Map<String, Map<QueueAttributeName, String>> listAllQueuesWithDetails() {
        List<String> allQueueUrls = listAllQueues();
        Map<String, Map<QueueAttributeName, String>> queueDetailsMap = new HashMap<>();

        List<CompletableFuture<Void>> futures = allQueueUrls.stream()
                .map(queueUrl -> CompletableFuture.runAsync(() -> {
                    Map<QueueAttributeName, String> details = getQueueDetails(queueUrl);
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

    public List<String> listAllQueues() {
        List<String> allQueueUrls = new ArrayList<>();
        String nextToken = null;

        do {
            try {
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

    public List<Map<QueueAttributeName, String>> listAllQueuesParallel() {
        List<String> allQueueUrls = listAllQueues();

        List<CompletableFuture<Map<QueueAttributeName, String>>> futures = allQueueUrls.stream()
                .map(queueUrl -> CompletableFuture.supplyAsync(() -> getQueueDetails(queueUrl)))
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

    private Map<QueueAttributeName, String> getQueueDetails(String queueUrl) {
        try {
            GetQueueAttributesRequest request = GetQueueAttributesRequest.builder()
                    .queueUrl(queueUrl)
                    .attributeNames(QueueAttributeName.ALL)
                    .build();
            GetQueueAttributesResponse response = sqsClient.getQueueAttributes(request);
            return response.attributes();
        } catch (SqsException e) {
            System.err.println("Error retrieving queue details for " + queueUrl + ": " + e.awsErrorDetails().errorMessage());
            throw e;
        }
    }
}