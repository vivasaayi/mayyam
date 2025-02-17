package org.poriyiyal.mayyam.cloud.aws.dataexport;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.poi.ss.usermodel.*;
import org.apache.poi.xssf.usermodel.XSSFWorkbook;
import org.poriyiyal.mayyam.core.services.CsvExportService;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;

public abstract class BaseExportService<T> {
    private final CsvExportService csvExportService;

    public BaseExportService(CsvExportService csvExportService) {
        this.csvExportService = csvExportService;
    }

    public void exportAsJson(List<T> data, String filePath) throws IOException {
        if (data == null || filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("Data and file path cannot be null or empty");
        }
        ObjectMapper objectMapper = new ObjectMapper();
        objectMapper.writeValue(Paths.get(filePath).toFile(), data);
    }

    public void exportAsCsv(List<String[]> data, String[] headers, String filePath) throws IOException {
        if (data == null || headers == null || filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("Data, headers, and file path cannot be null or empty");
        }
        csvExportService.exportDataAsCsv(data, headers, filePath);
    }

    public void exportAsCsv(List<String[]> data, String[] headers, String filePath, char delimiter) throws IOException {
        if (data == null || headers == null || filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("Data, headers, and file path cannot be null or empty");
        }
        csvExportService.exportDataAsCsv(data, headers, filePath, delimiter);
    }

    public void exportAsExcel(List<String[]> data, String[] headers, String filePath) throws IOException {
        if (data == null || headers == null || filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("Data, headers, and file path cannot be null or empty");
        }
        Workbook workbook = new XSSFWorkbook();
        Sheet sheet = workbook.createSheet("Data");

        Row headerRow = sheet.createRow(0);
        for (int i = 0; i < headers.length; i++) {
            headerRow.createCell(i).setCellValue(headers[i]);
        }

        int rowNum = 1;
        for (String[] rowData : data) {
            Row row = sheet.createRow(rowNum++);
            for (int i = 0; i < rowData.length; i++) {
                row.createCell(i).setCellValue(rowData[i]);
            }
        }

        try (var fileOut = Files.newOutputStream(Paths.get(filePath))) {
            workbook.write(fileOut);
        }
        workbook.close();
    }

    protected abstract List<String[]> convertToDataFormat(List<T> data);
}