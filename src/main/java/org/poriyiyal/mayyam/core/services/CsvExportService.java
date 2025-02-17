package org.poriyiyal.mayyam.core.services;

import com.opencsv.CSVWriter;

import java.io.FileWriter;
import java.io.IOException;
import java.util.List;

public class CsvExportService {

    public <T> void exportDataAsCsv(List<String[]> data, String[] headers, String filePath) throws IOException {
        writeCsv(data, headers, filePath, CSVWriter.DEFAULT_SEPARATOR);
    }

    public <T> void exportDataAsCsv(List<String[]> data, String[] headers, String filePath, char delimiter) throws IOException {
        writeCsv(data, headers, filePath, delimiter);
    }

    private void writeCsv(List<String[]> data, String[] headers, String filePath, char delimiter) throws IOException {
        java.nio.file.Path path = java.nio.file.Paths.get(filePath);
        if (!java.nio.file.Files.exists(path.getParent())) {
            throw new IOException("Directory does not exist: " + path.getParent());
        }

        try (CSVWriter writer = new CSVWriter(new FileWriter(filePath), delimiter, CSVWriter.NO_QUOTE_CHARACTER, CSVWriter.DEFAULT_ESCAPE_CHARACTER, CSVWriter.DEFAULT_LINE_END)) {
            writer.writeNext(headers);
            for (String[] row : data) {
                writer.writeNext(row);
            }
        } catch (IOException e) {
            throw new IOException("Error writing CSV file: " + e.getMessage(), e);
        }
    }
}