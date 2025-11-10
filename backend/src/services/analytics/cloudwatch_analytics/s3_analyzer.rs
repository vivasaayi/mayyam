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


use super::cloudwatch_analyzer::CloudWatchAnalyzer;
use crate::errors::AppError;
use crate::models::aws_resource;

pub struct S3Analyzer;

impl S3Analyzer {
    pub async fn analyze_s3_bucket(
        analyzer: &CloudWatchAnalyzer,
        resource: &aws_resource::Model,
        workflow: &str,
    ) -> Result<String, AppError> {
        let region = if resource.region.is_empty() {
            "us-east-1"
        } else {
            resource.region.as_str()
        };
        match workflow {
            "unused" | "detect_unused" => {
                let periods = ["7 days", "1 month", "2 months"];
                let mut lines = vec![
                    format!("# S3: {}", resource.resource_id),
                    String::from("## Unused check (hourly request metrics)\n"),
                ];
                for p in periods {
                    let (start, end) = analyzer.parse_time_period(p)?;
                    let unused = analyzer
                        .is_unused_in_window_by_hour(
                            "S3",
                            &resource.resource_id,
                            region,
                            start,
                            end,
                        )
                        .await?;
                    lines.push(format!(
                        "- {}: {}",
                        p,
                        if unused { "unused" } else { "active" }
                    ));
                }
                Ok(lines.join("\n"))
            }
            _ => Ok("S3 workflow not implemented yet".to_string()),
        }
    }
}
