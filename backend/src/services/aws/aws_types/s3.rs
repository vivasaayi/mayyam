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


use serde::{Deserialize, Serialize};

// S3-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3BucketInfo {
    pub bucket_name: String,
    pub creation_date: String,
    pub region: String,
    pub versioning_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3GetObjectRequest {
    pub bucket: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3PutObjectRequest {
    pub bucket: String,
    pub key: String,
    pub content_type: Option<String>,
    pub body: String,
}
