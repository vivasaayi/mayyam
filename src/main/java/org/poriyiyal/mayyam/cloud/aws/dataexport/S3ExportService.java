package org.poriyiyal.mayyam.cloud.aws.dataexport;

import org.poriyiyal.mayyam.cloud.aws.controlplane.S3Service;
import org.poriyiyal.mayyam.core.services.CsvExportService;
import software.amazon.awssdk.services.s3.model.Bucket;

import java.io.IOException;
import java.util.List;
import java.util.stream.Collectors;

public class S3ExportService extends BaseExportService<Bucket> {
    private final S3Service s3Service;

    public S3ExportService(S3Service s3Service, CsvExportService csvExportService) {
        super(csvExportService);
        this.s3Service = s3Service;
    }

    public void exportBucketsAsJson(String filePath) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            List<Bucket> buckets = s3Service.listBuckets();
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
            List<Bucket> buckets = s3Service.listBuckets();
            List<String[]> data = convertToDataFormat(buckets);
            String[] headers = {"Bucket Name", "Creation Date"};
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
            List<Bucket> buckets = s3Service.listBuckets();
            List<String[]> data = convertToDataFormat(buckets);
            String[] headers = {"Bucket Name", "Creation Date"};
            exportAsExcel(data, headers, filePath);
        } catch (Exception e) {
            System.err.println("Error exporting buckets as Excel: " + e.getMessage());
            throw e;
        }
    }

    @Override
    protected List<String[]> convertToDataFormat(List<Bucket> buckets) {
        return buckets.stream()
                .map(bucket -> new String[]{bucket.name(), bucket.creationDate().toString()})
                .collect(Collectors.toList());
    }
}