# Copyright (c) 2025 Rajan Panneer Selvam
#
# Licensed under the Business Source License 1.1 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     https://www.mariadb.com/bsl11
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.


#!/usr/bin/env python3
"""
Test Data Generator for Kafka Backup System Testing
Generates test messages for backup and restore validation
"""

import os
import json
import time
import random
import string
from datetime import datetime
from kafka import KafkaProducer
from kafka.errors import KafkaError

def generate_random_message(size_bytes=1024):
    """Generate a random message of specified size"""
    chars = string.ascii_letters + string.digits + string.punctuation + " "
    message = {
        "id": ''.join(random.choices(string.digits, k=10)),
        "timestamp": datetime.utcnow().isoformat(),
        "data": ''.join(random.choices(chars, k=size_bytes - 100)),  # Reserve space for metadata
        "source": "test-generator",
        "version": "1.0"
    }
    return json.dumps(message)

def generate_large_message(size_kb=64):
    """Generate a large message for testing"""
    size_bytes = size_kb * 1024
    return generate_random_message(size_bytes)

def main():
    # Configuration
    kafka_brokers = os.getenv('KAFKA_BROKERS', 'localhost:9092')
    test_topic = os.getenv('TEST_TOPIC', 'backup-test-topic')
    message_count = int(os.getenv('MESSAGE_COUNT', '1000'))
    batch_size = int(os.getenv('BATCH_SIZE', '100'))
    delay_ms = int(os.getenv('DELAY_MS', '10'))

    print(f"Starting test data generation...")
    print(f"Brokers: {kafka_brokers}")
    print(f"Topic: {test_topic}")
    print(f"Message Count: {message_count}")
    print(f"Batch Size: {batch_size}")

    # Create producer
    try:
        producer = KafkaProducer(
            bootstrap_servers=kafka_brokers,
            value_serializer=lambda v: json.dumps(v).encode('utf-8'),
            key_serializer=lambda k: k.encode('utf-8') if k else None,
            acks='all',
            retries=3,
            max_in_flight_requests_per_connection=5
        )
        print("✅ Connected to Kafka")
    except Exception as e:
        print(f"❌ Failed to connect to Kafka: {e}")
        return

    # Generate and send messages
    sent_count = 0
    start_time = time.time()

    try:
        for i in range(0, message_count, batch_size):
            batch_end = min(i + batch_size, message_count)

            for j in range(i, batch_end):
                # Generate message
                if j % 1000 == 0:
                    # Large message every 1000 messages
                    message_data = generate_large_message(64)  # 64KB
                    message_type = "large"
                else:
                    # Regular message
                    message_data = generate_random_message(1024)  # 1KB
                    message_type = "regular"

                # Send message
                key = f"key-{j:06d}"
                future = producer.send(
                    test_topic,
                    value=json.loads(message_data),
                    key=key
                )

                # Wait for batch completion
                if j % batch_size == 0 or j == message_count - 1:
                    producer.flush()
                    sent_count = j + 1

                    # Progress reporting
                    elapsed = time.time() - start_time
                    rate = sent_count / elapsed if elapsed > 0 else 0
                    print(f"📊 Progress: {sent_count}/{message_count} messages "
                          f"({rate:.1f} msg/s, {elapsed:.1f}s elapsed)")

                # Small delay between messages
                if delay_ms > 0:
                    time.sleep(delay_ms / 1000.0)

        # Final flush
        producer.flush()
        total_time = time.time() - start_time
        final_rate = message_count / total_time if total_time > 0 else 0

        print("
✅ Test data generation completed!"        print(f"📈 Total Messages: {message_count}")
        print(f"⏱️  Total Time: {total_time:.2f} seconds")
        print(f"🚀 Average Rate: {final_rate:.1f} messages/second")
        print(f"💾 Estimated Size: ~{message_count * 1.5 / 1024:.1f} MB")

    except KeyboardInterrupt:
        print("\n⚠️  Interrupted by user")
    except Exception as e:
        print(f"❌ Error during data generation: {e}")
    finally:
        producer.close()
        print("🔌 Connection closed")

if __name__ == "__main__":
    main()
