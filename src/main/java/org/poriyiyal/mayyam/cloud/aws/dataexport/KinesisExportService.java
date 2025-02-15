package org.poriyiyal.mayyam.cloud.aws.dataexport;

import org.poriyiyal.mayyam.cloud.aws.controlplane.KinesisService;
import org.poriyiyal.mayyam.core.services.CsvExportService;
import software.amazon.awssdk.services.kinesis.model.StreamDescription;

import java.io.IOException;
import java.util.List;
import java.util.stream.Collectors;
import java.util.ArrayList;
import java.util.Collections;

public class KinesisExportService extends BaseExportService<StreamDescription> {
    private final KinesisService kinesisService;

    public KinesisExportService(KinesisService kinesisService, CsvExportService csvExportService) {
        super(csvExportService);
        this.kinesisService = kinesisService;
    }

    public void exportStreamsAsJson(String filePath) throws IOException {
        List<StreamDescription> streams = getStreamDescriptions();
        exportAsJson(streams, filePath);
    }

    public void exportStreamsAsCsv(String filePath) throws IOException {
        exportStreamsAsCsv(filePath, ',');
    }

    public void exportStreamsAsCsv(String filePath, char delimiter) throws IOException {
        List<StreamDescription> streams = getStreamDescriptions();
        List<String[]> data = convertToDataFormat(streams);
        String[] headers = {"Stream Name", "Stream ARN", "Stream Status"};
        exportAsCsv(data, headers, filePath, delimiter);
    }

    public void exportStreamsAsExcel(String filePath) throws IOException {
        List<StreamDescription> streams = getStreamDescriptions();
        List<String[]> data = convertToDataFormat(streams);
        String[] headers = {"Stream Name", "Stream ARN", "Stream Status"};
        exportAsExcel(data, headers, filePath);
    }

    @Override
    protected List<String[]> convertToDataFormat(List<StreamDescription> streams) {
        return streams.stream()
                .map(stream -> new String[]{stream.streamName(), stream.streamARN(), stream.streamStatusAsString()})
                .collect(Collectors.toList());
    }

    private List<StreamDescription> getStreamDescriptions() {
        try {
            var streamNames = kinesisService.listStreams();
            return new ArrayList<>(streamNames.values());
        } catch (Exception e) {
            System.err.println("Failed to get stream descriptions: " + e.getMessage());
            return Collections.emptyList();
        }
    }
}