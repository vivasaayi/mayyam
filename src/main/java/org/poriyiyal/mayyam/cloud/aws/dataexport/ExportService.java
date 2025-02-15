package org.poriyiyal.mayyam.cloud.aws.dataexport;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.poi.ss.usermodel.*;
import org.apache.poi.xssf.usermodel.XSSFWorkbook;
import org.poriyiyal.mayyam.cloud.aws.controlplane.S3Service;
import org.poriyiyal.mayyam.core.services.CsvExportService;

import software.amazon.awssdk.services.s3.model.Bucket;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;
import java.util.stream.Collectors;

public class ExportService {
    private final S3Service s3Service;
    private final CsvExportService csvExportService;

    public ExportService(S3Service s3Service, CsvExportService csvExportService) {
        this.s3Service = s3Service;
        this.csvExportService = csvExportService;
    }

    public void exportBucketsAsJson(String filePath) throws IOException {
        List<Bucket> buckets = s3Service.listBuckets();
        ObjectMapper objectMapper = new ObjectMapper();
        objectMapper.writeValue(Paths.get(filePath).toFile(), buckets);
    }

    public void exportBucketsAsCsv(String filePath) throws IOException {
        List<Bucket> buckets = s3Service.listBuckets();
        List<String[]> data = buckets.stream()
                .map(bucket -> new String[]{bucket.name(), bucket.creationDate().toString()})
                .collect(Collectors.toList());
        String[] headers = {"Bucket Name", "Creation Date"};
        csvExportService.exportDataAsCsv(data, headers, filePath);
    }

    public void exportBucketsAsExcel(String filePath) throws IOException {
        List<Bucket> buckets = s3Service.listBuckets();
        Workbook workbook = new XSSFWorkbook();
        Sheet sheet = workbook.createSheet("Buckets");

        Row headerRow = sheet.createRow(0);
        headerRow.createCell(0).setCellValue("Bucket Name");
        headerRow.createCell(1).setCellValue("Creation Date");

        int rowNum = 1;
        for (Bucket bucket : buckets) {
            Row row = sheet.createRow(rowNum++);
            row.createCell(0).setCellValue(bucket.name());
            row.createCell(1).setCellValue(bucket.creationDate().toString());
        }

        try (var fileOut = Files.newOutputStream(Paths.get(filePath))) {
            workbook.write(fileOut);
        }
        workbook.close();
    }
}