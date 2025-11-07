use serde::{Deserialize, Serialize};

// Lambda Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaInvokeRequest {
    pub function_name: String,
    pub payload: serde_json::Value,
    pub invocation_type: Option<String>,
    pub client_context: Option<String>,
    pub qualifier: Option<String>,
}
