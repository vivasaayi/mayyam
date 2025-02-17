package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.springframework.stereotype.Service;
import java.util.stream.Collectors;
import java.util.Map;
import java.util.Collections;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.kinesis.model.StreamDescription;
import software.amazon.awssdk.services.kinesis.model.StreamStatus;
import software.amazon.awssdk.services.kinesis.model.DescribeStreamResponse;
import software.amazon.awssdk.services.kinesis.KinesisClient;

@Service
public class KinesisService extends BaseAwsService {

    private KinesisClient getKinesisClient(String region) {
        return KinesisClient.builder()
                .region(Region.of(region))
                .credentialsProvider(credentialsProvider)
                .build();
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
        try {
            KinesisClient kinesisClient = getKinesisClient(region);
            return kinesisClient.listStreamsPaginator().stream()
            .flatMap(response -> response.streamNames().stream())
            .parallel()
            .collect(Collectors.toMap(
                streamName -> streamName,
                streamName -> {
                    try {
                        return describeStream(region, streamName);
                    } catch (Exception e) {
                        System.err.println("Failed to describe stream: " + streamName + " - " + e.getMessage());
                        e.printStackTrace();
                        return null;
                    }
                }
            ));
        } catch (Exception e) {
            System.err.println("Failed to list streams: " + e.getMessage());
            e.printStackTrace();
            return Collections.emptyMap();
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