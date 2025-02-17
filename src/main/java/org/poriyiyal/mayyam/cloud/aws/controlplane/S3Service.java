package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.springframework.stereotype.Service;
import software.amazon.awssdk.services.s3.S3Client;
import software.amazon.awssdk.services.s3.model.*;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

@Service
public class S3Service extends BaseAwsService {
    private final S3Client s3Client;

    public S3Service() {
        this.s3Client = S3Client.builder()
                .region(region)
                .credentialsProvider(credentialsProvider)
                .build();
    }

    public void createBucket(String bucketName) {
        try {
            CreateBucketRequest request = CreateBucketRequest.builder()
                    .bucket(bucketName)
                    .build();
            s3Client.createBucket(request);
            System.out.println("Bucket created successfully: " + bucketName);
        } catch (S3Exception e) {
            System.err.println("Failed to create bucket: " + e.getMessage());
            throw e;
        }
    }

    public void deleteBucket(String bucketName) {
        try {
            DeleteBucketRequest request = DeleteBucketRequest.builder()
                    .bucket(bucketName)
                    .build();
            s3Client.deleteBucket(request);
            System.out.println("Bucket deleted successfully: " + bucketName);
        } catch (S3Exception e) {
            System.err.println("Failed to delete bucket: " + e.getMessage());
            throw e;
        }
    }

    public Map<String, Bucket> listBuckets() {
        try {
            ListBucketsResponse response = s3Client.listBuckets();
            return response.buckets().stream()
                    .collect(Collectors.toMap(Bucket::name, bucket -> bucket));
        } catch (S3Exception e) {
            System.err.println("Failed to list buckets: " + e.getMessage());
            throw e;
        }
    }

    public List<Bucket> getBucketsAsList() {
        return listBuckets().values().stream().collect(Collectors.toList());
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
