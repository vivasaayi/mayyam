package org.poriyiyal.mayyam.cloud.aws.dataexport;

import org.poriyiyal.mayyam.cloud.aws.controlplane.SqsService;
import org.poriyiyal.mayyam.core.services.CsvExportService;
import software.amazon.awssdk.services.sqs.model.QueueAttributeName;

import java.io.IOException;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;
import java.util.ArrayList;

public class SQSExportService extends BaseExportService<QueueDescriptionWithRegion> {
    private final SqsService sqsService;
    private final List<String> regions;

    public SQSExportService(SqsService sqsService, CsvExportService csvExportService, List<String> regions) {
        super(csvExportService);
        this.sqsService = sqsService;
        this.regions = regions;
    }

    public void exportQueuesAsJson(String filePath) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            List<QueueDescriptionWithRegion> queues = getQueueDescriptions();
            exportAsJson(queues, filePath);
        } catch (Exception e) {
            System.err.println("Error exporting queues as JSON: " + e.getMessage());
            throw e;
        }
    }

    public void exportQueuesAsCsv(String filePath) throws IOException {
        exportQueuesAsCsv(filePath, ',');
    }

    public void exportQueuesAsCsv(String filePath, char delimiter) throws IOException {
        if (filePath == null || filePath.isEmpty()) {
            throw new IllegalArgumentException("File path cannot be null or empty");
        }
        try {
            List<QueueDescriptionWithRegion> queues = getQueueDescriptions();
            List<String[]> data = convertToDataFormat(queues);
            String[] headers = {"Region", "Queue URL", "Queue ARN", "Approximate Number of Messages"};
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
            List<QueueDescriptionWithRegion> queues = getQueueDescriptions();
            List<String[]> data = convertToDataFormat(queues);
            String[] headers = {"Region", "Queue URL", "Queue ARN", "Approximate Number of Messages"};
            exportAsExcel(data, headers, filePath);
        } catch (Exception e) {
            System.err.println("Error exporting queues as Excel: " + e.getMessage());
            throw e;
        }
    }

    @Override
    protected List<String[]> convertToDataFormat(List<QueueDescriptionWithRegion> queues) {
        return queues.stream()
                .map(queue -> new String[]{
                        queue.getRegion(),
                        queue.getQueueAttributes().get(QueueAttributeName.QUEUE_ARN),
                        queue.getQueueAttributes().get(QueueAttributeName.APPROXIMATE_NUMBER_OF_MESSAGES)
                })
                .collect(Collectors.toList());
    }

    private List<QueueDescriptionWithRegion> getQueueDescriptions() {
        List<QueueDescriptionWithRegion> allQueues = new ArrayList<>();
        for (String region : regions) {
            try {
                Map<String, Map<QueueAttributeName, String>> queuesMap = sqsService.listAllQueuesWithDetails(region);
                queuesMap.forEach((queueUrl, attributes) -> 
                    allQueues.add(new QueueDescriptionWithRegion(region, attributes))
                );
            } catch (Exception e) {
                System.err.println("Failed to get queue descriptions for region " + region + ": " + e.getMessage());
            }
        }
        return allQueues;
    }
}