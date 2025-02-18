package org.poriyiyal.mayyam.cloud.aws.dataexport;

import org.poriyiyal.mayyam.cloud.aws.controlplane.KinesisService;
import org.poriyiyal.mayyam.core.services.CsvExportService;
import software.amazon.awssdk.services.kinesis.model.StreamDescription;

import java.io.IOException;
import java.util.List;
import java.util.stream.Collectors;
import java.util.ArrayList;
import java.util.Collections;

public class KinesisExportService extends BaseExportService<StreamDescriptionWithRegion> {
    private final KinesisService kinesisService;
    private final List<String> regions;

    public KinesisExportService(KinesisService kinesisService, CsvExportService csvExportService, List<String> regions) {
        super(csvExportService);
        this.kinesisService = kinesisService;
        this.regions = regions;
    }

    public void exportStreamsAsJson(String filePath) throws IOException {
        List<StreamDescriptionWithRegion> streams = getStreamDescriptions();
        exportAsJson(streams, filePath);
    }

    public void exportStreamsAsCsv(String filePath) throws IOException {
        exportStreamsAsCsv(filePath, ',');
    }

    public void exportStreamsAsCsv(String filePath, char delimiter) throws IOException {
        List<StreamDescriptionWithRegion> streams = getStreamDescriptions();
        List<String[]> data = convertToDataFormat(streams);
        String[] headers = {"Region", "Stream Name", "Stream ARN", "Stream Status"};
        exportAsCsv(data, headers, filePath, delimiter);
    }

    public void exportStreamsAsExcel(String filePath) throws IOException {
        List<StreamDescriptionWithRegion> streams = getStreamDescriptions();
        List<String[]> data = convertToDataFormat(streams);
        String[] headers = {"Region", "Stream Name", "Stream ARN", "Stream Status"};
        exportAsExcel(data, headers, filePath);
    }

    @Override
    protected List<String[]> convertToDataFormat(List<StreamDescriptionWithRegion> streams) {
        return streams.stream()
                .map(stream -> new String[]{
                        stream.getRegion(),
                        stream.getStreamDescription().streamName(),
                        stream.getStreamDescription().streamARN(),
                        stream.getStreamDescription().streamStatusAsString()
                })
                .collect(Collectors.toList());
    }

    private List<StreamDescriptionWithRegion> getStreamDescriptions() {
        List<StreamDescriptionWithRegion> allStreams = new ArrayList<>();
        for (String region : regions) {
            try {
                var streamNames = kinesisService.listStreams(region);
                streamNames.values().forEach(streamDescription -> 
                    allStreams.add(new StreamDescriptionWithRegion(region, streamDescription))
                );
            } catch (Exception e) {
                System.err.println("Failed to get stream descriptions for region " + region + ": " + e.getMessage());
            }
        }
        return allStreams;
    }
}