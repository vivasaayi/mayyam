package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;
import software.amazon.awssdk.auth.credentials.DefaultCredentialsProvider;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.s3.S3Client;
import software.amazon.awssdk.services.s3.model.*;
import software.amazon.awssdk.services.s3.S3Configuration;

import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

@Service
public class S3Service extends BaseAwsService {
    private final S3Client s3Client;

    public S3Service(@Value("${aws.region:us-west-2}") String region) {
        if (region == null || region.isEmpty()) {
            throw new IllegalArgumentException("Region must not be blank or empty.");
        }
        this.s3Client = S3Client.builder()
                .region(Region.of(region))
                .credentialsProvider(DefaultCredentialsProvider.create())
                .serviceConfiguration(S3Configuration.builder().build())
                .build();
    }

    private S3Client getS3ClientForBucket(String bucketName) {
        GetBucketLocationResponse locationResponse = s3Client.getBucketLocation(GetBucketLocationRequest.builder().bucket(bucketName).build());
        Region bucketRegion = Region.of(locationResponse.locationConstraintAsString());
        return S3Client.builder()
                .region(bucketRegion)
                .credentialsProvider(DefaultCredentialsProvider.create())
                .serviceConfiguration(S3Configuration.builder().build())
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
            S3Client client = getS3ClientForBucket(bucketName);
            DeleteBucketRequest request = DeleteBucketRequest.builder()
                    .bucket(bucketName)
                    .build();
            client.deleteBucket(request);
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
            S3Client client = getS3ClientForBucket(bucketName);
            PutObjectRequest putObjectRequest = PutObjectRequest.builder()
                    .bucket(bucketName)
                    .key(key)
                    .build();
            client.putObject(putObjectRequest,
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
            S3Client client = getS3ClientForBucket(bucketName);
            GetObjectRequest getObjectRequest = GetObjectRequest.builder()
                    .bucket(bucketName)
                    .key(key)
                    .build();
            client.getObject(getObjectRequest, software.amazon.awssdk.core.sync.ResponseTransformer
                    .toFile(java.nio.file.Paths.get(destinationPath)));
        } catch (S3Exception e) {
            System.err.println(e.awsErrorDetails().errorMessage());
        }
    }

    public List<Map<String, String>> getBucketsWithoutReplication() {
        ListBucketsResponse listBucketsResponse = s3Client.listBuckets();
        return listBucketsResponse.buckets().stream()
                .map(bucket -> {
                    try {
                        S3Client client = getS3ClientForBucket(bucket.name());
                        GetBucketReplicationResponse replicationResponse = client.getBucketReplication(GetBucketReplicationRequest.builder().bucket(bucket.name()).build());
                        if (replicationResponse.replicationConfiguration() == null || replicationResponse.replicationConfiguration().rules().isEmpty()) {
                            return Map.of(
                                    "bucketName", bucket.name(),
                                    "creationDate", bucket.creationDate().toString()
                            );
                        }
                    } catch (S3Exception e) {
                        if (e.statusCode() == 404) {
                            return Map.of(
                                    "bucketName", bucket.name(),
                                    "creationDate", bucket.creationDate().toString()
                            );
                        } else {
                            throw e;
                        }
                    }
                    return null;
                })
                .filter(bucket -> bucket != null)
                .collect(Collectors.toList());
    }

    public List<Map<String, String>> getBucketsWithReplication() {
        ListBucketsResponse listBucketsResponse = s3Client.listBuckets();
        return listBucketsResponse.buckets().stream()
                .map(bucket -> {
                    try {
                        S3Client client = getS3ClientForBucket(bucket.name());
                        GetBucketReplicationResponse replicationResponse = client.getBucketReplication(GetBucketReplicationRequest.builder().bucket(bucket.name()).build());
                        if (replicationResponse.replicationConfiguration() != null && !replicationResponse.replicationConfiguration().rules().isEmpty()) {
                            return Map.of(
                                    "bucketName", bucket.name(),
                                    "creationDate", bucket.creationDate().toString(),
                                    "replicationRules", replicationResponse.replicationConfiguration().rules().stream()
                                            .map(rule -> rule.destination().bucket())
                                            .collect(Collectors.joining(", "))
                            );
                        }
                    } catch (S3Exception e) {
                        if (e.statusCode() == 404) {
                            return null;
                        } else {
                            throw e;
                        }
                    }
                    return null;
                })
                .filter(bucket -> bucket != null)
                .collect(Collectors.toList());
    }
}
