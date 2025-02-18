package org.poriyiyal.mayyam.opensearch;

import org.apache.http.HttpHost;
import org.elasticsearch.action.bulk.BulkRequest;
import org.elasticsearch.action.bulk.BulkResponse;
import org.elasticsearch.action.index.IndexRequest;
import org.elasticsearch.client.RequestOptions;
import org.elasticsearch.client.RestClient;
import org.elasticsearch.client.RestHighLevelClient;
import org.elasticsearch.common.xcontent.XContentType;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.io.IOException;
import java.util.List;
import java.util.Map;

@Service
public class OpenSearchService {
    private static final Logger logger = LoggerFactory.getLogger(OpenSearchService.class);
    private final RestHighLevelClient client;

    public OpenSearchService() {
        this.client = new RestHighLevelClient(
                RestClient.builder(new HttpHost("localhost", 9200, "http")));
    }

    public void bulkIndex(String index, List<Map<String, Object>> documents) {
        BulkRequest request = new BulkRequest();
        for (Map<String, Object> document : documents) {
            request.add(new IndexRequest(index)
                    .source(document, XContentType.JSON));
        }

        try {
            BulkResponse response = client.bulk(request, RequestOptions.DEFAULT);
            if (response.hasFailures()) {
                logger.error("Bulk indexing had failures: {}", response.buildFailureMessage());
            } else {
                logger.info("Bulk indexing completed successfully.");
            }
        } catch (IOException e) {
            logger.error("Error during bulk indexing: {}", e.getMessage());
        }
    }

    public void close() {
        try {
            client.close();
        } catch (IOException e) {
            logger.error("Error closing OpenSearch client: {}", e.getMessage());
        }
    }
}