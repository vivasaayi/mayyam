// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


// Kinesis API service for interacting with the backend
import axios from 'axios';

const API_BASE_URL = process.env.REACT_APP_API_URL || 'http://localhost:8085';

class KinesisService {
  constructor() {
    this.client = axios.create({
      baseURL: API_BASE_URL,
      timeout: 30000,
      headers: {
        'Content-Type': 'application/json'
      }
    });

    // Add auth interceptor
    this.client.interceptors.request.use((config) => {
      const token = localStorage.getItem('token');
      if (token) {
        config.headers.Authorization = `Bearer ${token}`;
      }
      return config;
    });
  }

  // Control Plane Operations
  async getStreamLimits(profile, region) {
    const response = await this.client.get(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/limits`);
    return response.data;
  }

  async describeStream(profile, region, streamName) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/streams/describe`, {
      stream_name: streamName
    });
    return response.data;
  }

  async getStreamSummary(profile, region, streamName) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/streams/summary`, {
      stream_name: streamName
    });
    return response.data;
  }

  async createStream(profile, region, streamName, shardCount = 1, streamModeDetails = { stream_mode: "PROVISIONED" }) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/streams`, {
      stream_name: streamName,
      shard_count: shardCount,
      stream_mode_details: streamModeDetails
    });
    return response.data;
  }

  async deleteStream(profile, region, streamName) {
    const response = await this.client.delete(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/streams`, {
      data: { stream_name: streamName }
    });
    return response.data;
  }

  async increaseRetentionPeriod(profile, region, streamName, retentionPeriodHours) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/streams/retention/increase`, {
      stream_name: streamName,
      retention_period_hours: retentionPeriodHours
    });
    return response.data;
  }

  async decreaseRetentionPeriod(profile, region, streamName, retentionPeriodHours) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/streams/retention/decrease`, {
      stream_name: streamName,
      retention_period_hours: retentionPeriodHours
    });
    return response.data;
  }

  async enableEnhancedMonitoring(profile, region, streamName, shardLevelMetrics = ['IncomingRecords', 'OutgoingRecords']) {
    const response = await this.client.post(`/api/aws-data/profiles/{profile}/regions/${region}/kinesis/streams/monitoring/enable`, {
      stream_name: streamName,
      shard_level_metrics: shardLevelMetrics
    });
    return response.data;
  }

  async disableEnhancedMonitoring(profile, region, streamName, shardLevelMetrics = ['IncomingRecords', 'OutgoingRecords']) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/streams/monitoring/disable`, {
      stream_name: streamName,
      shard_level_metrics: shardLevelMetrics
    });
    return response.data;
  }

  async listShards(profile, region, streamName, nextToken = null, maxResults = 100) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/shards`, {
      stream_name: streamName,
      next_token: nextToken,
      max_results: maxResults
    });
    return response.data;
  }

  // Data Plane Operations
  async putRecord(profile, region, streamName, data, partitionKey) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis`, {
      stream_name: streamName,
      data: btoa(data), // Base64 encode
      partition_key: partitionKey
    });
    return response.data;
  }

  async putRecords(profile, region, streamName, records) {
    const encodedRecords = records.map(record => ({
      ...record,
      data: btoa(record.data) // Base64 encode each record
    }));
    
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/records/put`, {
      stream_name: streamName,
      records: encodedRecords
    });
    return response.data;
  }

  async getShardIterator(profile, region, streamName, shardId, shardIteratorType = 'LATEST', startingSequenceNumber = null, timestamp = null) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/shard-iterator`, {
      stream_name: streamName,
      shard_id: shardId,
      shard_iterator_type: shardIteratorType,
      starting_sequence_number: startingSequenceNumber,
      timestamp: timestamp
    });
    return response.data;
  }

  async getRecords(profile, region, shardIterator, limit = 10) {
    const response = await this.client.post(`/api/aws-data/profiles/${profile}/regions/${region}/kinesis/records/get`, {
      shard_iterator: shardIterator,
      limit: limit
    });
    
    // Decode base64 data in records
    if (response.data.records) {
      response.data.records = response.data.records.map(record => ({
        ...record,
        data: atob(record.data) // Base64 decode
      }));
    }
    
    return response.data;
  }

  // CloudWatch Metrics (using existing metrics endpoint)
  async getStreamMetrics(profile, region, streamName, metricName = 'IncomingRecords', startTime, endTime) {
    const response = await this.client.get(`/api/aws/profiles/${profile}/regions/${region}/metrics/kinesis/${streamName}`, {
      params: {
        metric_name: metricName,
        start_time: startTime,
        end_time: endTime,
        period: 300 // 5 minutes
      }
    });
    return response.data;
  }

  // Utility methods
  encodeData(data) {
    return btoa(typeof data === 'string' ? data : JSON.stringify(data));
  }

  decodeData(encodedData) {
    try {
      const decoded = atob(encodedData);
      try {
        return JSON.parse(decoded);
      } catch {
        return decoded;
      }
    } catch {
      return encodedData;
    }
  }
}

export default new KinesisService();
