package org.poriyiyal.mayyam.cloud.aws.dataexport;

import org.poriyiyal.mayyam.cloud.aws.controlplane.SqsService;
import org.poriyiyal.mayyam.core.services.CsvExportService;
import software.amazon.awssdk.services.sqs.model.QueueAttributeName;

import java.io.IOException;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

public class SQSExportService extends BaseExportService<Map<QueueAttributeName, String>> {
    private final SqsService sqsService;

    public SQSExportService(SqsService sqsService, CsvExportService csvExportService) {
        super(csvExportService);
        this.sqsService = sqsService;
    }

    public void exportQueuesAsJson(String filePath) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            Map<String, Map<QueueAttributeName, String>> queuesMap = sqsService.listAllQueuesWithDetails();
            List<Map<QueueAttributeName, String>> queues = queuesMap.values().stream().collect(Collectors.toList());
            exportAsJson(queues, filePath);
        } catch (Exception e) {
            System.err.println("Error exporting queues as JSON: " + e.getMessage());
            throw e;
        }
    }

    public void exportQueuesAsCsv(String filePath) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            Map<String, Map<QueueAttributeName, String>> queuesMap = sqsService.listAllQueuesWithDetails();
            List<Map<QueueAttributeName, String>> queues = queuesMap.values().stream().collect(Collectors.toList());
            List<String[]> data = convertToDataFormat(queues);
            String[] headers = {"Queue URL", "Queue ARN", "Approximate Number of Messages"};
            exportAsCsv(data, headers, filePath);
        } catch (Exception e) {
            System.err.println("Error exporting queues as CSV: " + e.getMessage());
            throw e;
        }
    }

    public void exportQueuesAsCsv(String filePath, char delimiter) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            Map<String, Map<QueueAttributeName, String>> queuesMap = sqsService.listAllQueuesWithDetails();
            List<Map<QueueAttributeName, String>> queues = queuesMap.values().stream().collect(Collectors.toList());
            List<String[]> data = convertToDataFormat(queues);
            String[] headers = {"Queue URL", "Queue ARN", "Approximate Number of Messages"};
            exportAsCsv(data, headers, filePath, delimiter);
        } catch (Exception e) {
            System.err.println("Error exporting queues as CSV with delimiter: " + e.getMessage());
            throw e;
        }
    }

    public void exportQueuesAsExcel(String filePath) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            Map<String, Map<QueueAttributeName, String>> queuesMap = sqsService.listAllQueuesWithDetails();
            List<Map<QueueAttributeName, String>> queues = queuesMap.values().stream().collect(Collectors.toList());
            List<String[]> data = convertToDataFormat(queues);
            String[] headers = {"Queue URL", "Queue ARN", "Approximate Number of Messages"};
            exportAsExcel(data, headers, filePath);
        } catch (Exception e) {
            System.err.println("Error exporting queues as Excel: " + e.getMessage());
            throw e;
        }
    }

    @Override
    protected List<String[]> convertToDataFormat(List<Map<QueueAttributeName, String>> queues) {
        return queues.stream()
                .map(queue -> new String[]{
                        queue.get(QueueAttributeName.QUEUE_ARN),
                        queue.get(QueueAttributeName.APPROXIMATE_NUMBER_OF_MESSAGES)
                })
                .collect(Collectors.toList());
    }
}