package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.springframework.stereotype.Service;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.kinesis.KinesisClient;
import software.amazon.awssdk.services.kinesis.model.StreamDescription;
import software.amazon.awssdk.services.kinesis.model.StreamStatus;
import software.amazon.awssdk.services.kinesis.model.DescribeStreamResponse;

import java.util.Map;
import java.util.Collections;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.stream.Collectors;
import java.util.concurrent.Executors;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.List;
import java.util.HashMap;

@Service
public class KinesisService extends BaseAwsService {

    private final ConcurrentMap<Region, KinesisClient> clientCache = new ConcurrentHashMap<>();

    private KinesisClient getKinesisClient(String region) {
        return clientCache.computeIfAbsent(Region.of(region), r -> KinesisClient.builder()
                .region(r)
                .credentialsProvider(credentialsProvider)
                .build());
    }

    public void createStream(String region, String streamName, int shardCount) {
        if (streamName == null || streamName.isEmpty()) {
            throw new IllegalArgumentException("Stream name cannot be null or empty");
        }
        if (shardCount <= 0) {
            throw new IllegalArgumentException("Shard count must be greater than 0");
        }

        try {
            KinesisClient kinesisClient = getKinesisClient(region);
            if (kinesisClient.listStreams().streamNames().contains(streamName)) {
                throw new IllegalArgumentException("Stream with name " + streamName + " already exists");
            }

            kinesisClient.createStream(builder -> builder
                .streamName(streamName)
                .shardCount(shardCount)
                .build());

            // Wait for the stream to become active
            DescribeStreamResponse describeStreamResponse;
            do {
                describeStreamResponse = kinesisClient.describeStream(builder -> builder
                    .streamName(streamName)
                    .build());
                Thread.sleep(1000);
            } while (!describeStreamResponse.streamDescription().streamStatus().equals(StreamStatus.ACTIVE));

            // Print the stream details
            StreamDescription streamDescription = describeStreamResponse.streamDescription();
            System.out.println("Stream created successfully: " + streamDescription);
            System.out.println("Stream created successfully: " + streamName);
        } catch (Exception e) {
            System.err.println("Failed to create stream: " + e.getMessage());
            e.printStackTrace();
        }
    }

    public void deleteStream(String region, String streamName) {
        if (streamName == null || streamName.isEmpty()) {
            throw new IllegalArgumentException("Stream name cannot be null or empty");
        }

        try {
            KinesisClient kinesisClient = getKinesisClient(region);
            kinesisClient.deleteStream(builder -> builder
                .streamName(streamName)
                .build());
            System.out.println("Stream deleted successfully: " + streamName);
        } catch (Exception e) {
            System.err.println("Failed to delete stream: " + e.getMessage());
            e.printStackTrace();
        }
    }

    public Map<String, StreamDescription> listStreams(String region) {
        ExecutorService executorService = Executors.newFixedThreadPool(5);
        try {
            KinesisClient kinesisClient = getKinesisClient(region);
            List<CompletableFuture<Map.Entry<String, StreamDescription>>> futures = kinesisClient.listStreamsPaginator().stream()
                .flatMap(response -> response.streamNames().stream())
                .map(streamName -> CompletableFuture.supplyAsync(() -> {
                    try {
                        return Map.entry(streamName, describeStream(region, streamName));
                    } catch (Exception e) {
                        System.err.println("Failed to describe stream: " + streamName + " - " + e.getMessage());
                        e.printStackTrace();
                        return null;
                    }
                }, executorService))
                .collect(Collectors.toList());

            Map<String, StreamDescription> result = new HashMap<>();
            for (CompletableFuture<Map.Entry<String, StreamDescription>> future : futures) {
                try {
                    Map.Entry<String, StreamDescription> entry = future.get();
                    if (entry != null) {
                        result.put(entry.getKey(), entry.getValue());
                    }
                } catch (InterruptedException | ExecutionException e) {
                    System.err.println("Error retrieving stream description: " + e.getMessage());
                }
            }
            return result;
        } catch (Exception e) {
            System.err.println("Failed to list streams: " + e.getMessage());
            e.printStackTrace();
            return Collections.emptyMap();
        } finally {
            executorService.shutdown();
        }
    }

    public StreamDescription describeStream(String region, String streamName) {
        if (streamName == null || streamName.isEmpty()) {
            throw new IllegalArgumentException("Stream name cannot be null or empty");
        }

        try {
            KinesisClient kinesisClient = getKinesisClient(region);
            StreamDescription streamDescription = kinesisClient.describeStream(builder -> builder
                .streamName(streamName)
                .build())
                .streamDescription();
            System.out.println("Stream Description: " + streamDescription);
            return streamDescription;
        } catch (Exception e) {
            System.err.println("Failed to describe stream: " + e.getMessage());
            e.printStackTrace();
            return null;
        }        
    }
}