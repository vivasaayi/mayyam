package org.poriyiyal.mayyam.cloud.aws.dataexport;

import org.poriyiyal.mayyam.cloud.aws.controlplane.S3Service;
import org.poriyiyal.mayyam.core.services.CsvExportService;
import software.amazon.awssdk.services.s3.model.Bucket;

import java.io.IOException;
import java.util.List;
import java.util.stream.Collectors;
import java.util.ArrayList;

public class S3ExportService extends BaseExportService<BucketWithRegion> {
    private final S3Service s3Service;
    private final List<String> regions;

    public S3ExportService(S3Service s3Service, CsvExportService csvExportService, List<String> regions) {
        super(csvExportService);
        this.s3Service = s3Service;
        this.regions = regions;
    }

    public void exportBucketsAsJson(String filePath) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            List<BucketWithRegion> buckets = getBucketDescriptions();
            exportAsJson(buckets, filePath);
        } catch (Exception e) {
            System.err.println("Error exporting buckets as JSON: " + e.getMessage());
            throw e;
        }
    }

    public void exportBucketsAsCsv(String filePath) throws IOException {
        exportBucketsAsCsv(filePath, ',');
    }

    public void exportBucketsAsCsv(String filePath, char delimiter) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            List<BucketWithRegion> buckets = getBucketDescriptions();
            List<String[]> data = convertToDataFormat(buckets);
            String[] headers = {"Region", "Bucket Name", "Creation Date"};
            exportAsCsv(data, headers, filePath, delimiter);
        } catch (Exception e) {
            System.err.println("Error exporting buckets as CSV: " + e.getMessage());
            throw e;
        }
    }

    public void exportBucketsAsExcel(String filePath) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            List<BucketWithRegion> buckets = getBucketDescriptions();
            List<String[]> data = convertToDataFormat(buckets);
            String[] headers = {"Region", "Bucket Name", "Creation Date"};
            exportAsExcel(data, headers, filePath);
        } catch (Exception e) {
            System.err.println("Error exporting buckets as Excel: " + e.getMessage());
            throw e;
        }
    }

    @Override
    protected List<String[]> convertToDataFormat(List<BucketWithRegion> buckets) {
        return buckets.stream()
                .map(bucketWithRegion -> new String[]{
                        bucketWithRegion.getRegion(),
                        bucketWithRegion.getBucket().name(),
                        bucketWithRegion.getBucket().creationDate().toString()
                })
                .collect(Collectors.toList());
    }

    private List<BucketWithRegion> getBucketDescriptions() {
        List<BucketWithRegion> allBuckets = new ArrayList<>();
        for (String region : regions) {
            try {
                List<Bucket> buckets = s3Service.getBucketsAsList(region);
                buckets.forEach(bucket -> allBuckets.add(new BucketWithRegion(region, bucket)));
            } catch (Exception e) {
                System.err.println("Failed to get bucket descriptions for region " + region + ": " + e.getMessage());
            }
        }
        return allBuckets;
    }
}