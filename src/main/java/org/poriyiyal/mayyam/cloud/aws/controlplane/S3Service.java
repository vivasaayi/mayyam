package org.poriyiyal.mayyam.cloud.aws.controlplane;

import software.amazon.awssdk.services.s3.S3Client;
import software.amazon.awssdk.services.s3.model.*;
import software.amazon.awssdk.core.async.SdkPublisher;
import software.amazon.awssdk.core.exception.SdkException;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;

public class S3Service extends BaseAwsService {
    private final S3Client s3Client;

    public S3Service() {
        this.s3Client = S3Client.builder()
                .region(region)
                .credentialsProvider(credentialsProvider)
                .build();
    }

    public void createBucket(String bucketName) {
        if (bucketName == null || bucketName.isEmpty()) {
            throw new IllegalArgumentException("Bucket name cannot be null or empty");
        }

        try {
            CreateBucketRequest createBucketRequest = CreateBucketRequest.builder()
                    .bucket(bucketName)
                    .build();
            s3Client.createBucket(createBucketRequest);
        } catch (S3Exception e) {
            System.err.println(e.awsErrorDetails().errorMessage());
        }
    }

    public void deleteBucket(String bucketName) {
        if (bucketName == null || bucketName.isEmpty()) {
            throw new IllegalArgumentException("Bucket name cannot be null or empty");
        }

        try {
            DeleteBucketRequest deleteBucketRequest = DeleteBucketRequest.builder()
                    .bucket(bucketName)
                    .build();
            s3Client.deleteBucket(deleteBucketRequest);
        } catch (S3Exception e) {
            System.err.println(e.awsErrorDetails().errorMessage());
        }
    }

    public void listBuckets() {
        try {
            ListBucketsResponse listBucketsResponse = s3Client.listBuckets();
            listBucketsResponse.buckets().forEach(bucket -> System.out.println(bucket.name()));
        } catch (S3Exception e) {
            System.err.println(e.awsErrorDetails().errorMessage());
        }
    }

    public void uploadObject(String bucketName, String key, String filePath) {
        if (bucketName == null || bucketName.isEmpty() || key == null || key.isEmpty() || filePath == null
                || filePath.isEmpty()) {
            throw new IllegalArgumentException("Bucket name, key, and file path cannot be null or empty");
        }

        try {
            PutObjectRequest putObjectRequest = PutObjectRequest.builder()
                    .bucket(bucketName)
                    .key(key)
                    .build();
            s3Client.putObject(putObjectRequest,
                    software.amazon.awssdk.core.sync.RequestBody.fromFile(java.nio.file.Paths.get(filePath)));
        } catch (S3Exception e) {
            System.err.println(e.awsErrorDetails().errorMessage());
        }
    }

    public void downloadObject(String bucketName, String key, String destinationPath) {
        if (bucketName == null || bucketName.isEmpty() || key == null || key.isEmpty() || destinationPath == null
                || destinationPath.isEmpty()) {
            throw new IllegalArgumentException("Bucket name, key, and destination path cannot be null or empty");
        }

        try {
            GetObjectRequest getObjectRequest = GetObjectRequest.builder()
                    .bucket(bucketName)
                    .key(key)
                    .build();
            s3Client.getObject(getObjectRequest, software.amazon.awssdk.core.sync.ResponseTransformer
                    .toFile(java.nio.file.Paths.get(destinationPath)));
        } catch (S3Exception e) {
            System.err.println(e.awsErrorDetails().errorMessage());
        }
    }
}
