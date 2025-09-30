use config::{Config as ConfigFile, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub kafka: KafkaConfig,
    pub auth: AuthConfig,
    pub cloud: CloudConfig,
    pub ai: AIConfig,
    pub security: SecurityConfig,
    pub kubernetes: KubernetesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            kafka: KafkaConfig::default(),
            auth: AuthConfig::default(),
            cloud: CloudConfig::default(),
            ai: AIConfig::default(),
            security: SecurityConfig::default(),
            kubernetes: KubernetesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub postgres: Vec<PostgresConfig>,
    pub mysql: Vec<MySQLConfig>,
    pub redis: Vec<RedisConfig>,
    pub opensearch: Vec<OpenSearchConfig>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            postgres: vec![],
            mysql: vec![],
            redis: vec![],
            opensearch: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub ssl_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySQLConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub cluster_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSearchConfig {
    pub name: String,
    pub hosts: Vec<String>,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    pub clusters: Vec<KafkaClusterConfig>,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self { clusters: vec![] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaClusterConfig {
    pub name: String,
    pub bootstrap_servers: Vec<String>,
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
    pub sasl_mechanism: Option<String>,
    pub security_protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration: u64,
    pub enable_local_auth: bool,
    pub enable_token_auth: bool,
    pub enable_saml: bool,
    pub saml_metadata_url: Option<String>,
    pub encryption_key: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "default-jwt-secret-key-for-development-only".to_string(),
            jwt_expiration: 3600,
            enable_local_auth: true,
            enable_token_auth: true,
            enable_saml: false,
            saml_metadata_url: None,
            encryption_key: "default-encryption-key-for-development-only".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfig {
    pub aws: Vec<AwsConfig>,
    pub azure: Vec<AzureConfig>,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            aws: vec![],
            azure: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    pub name: String,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub region: String,
    pub role_arn: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureConfig {
    pub name: String,
    pub tenant_id: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub subscription_id: String,
    pub use_managed_identity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub endpoint: Option<String>,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            api_key: "default-api-key".to_string(),
            model: "gpt-4".to_string(),
            endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub encryption_key: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            encryption_key: "default-encryption-key-for-development-only".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesConfig {
    pub clusters: Vec<KubernetesClusterConfig>,
}

impl Default for KubernetesConfig {
    fn default() -> Self {
        Self { clusters: vec![] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesClusterConfig {
    pub name: String,
    pub context: String,
    pub config_path: Option<String>,
    pub api_url: Option<String>,
    pub ca_cert: Option<String>,
    pub token: Option<String>,
}

pub fn load_config() -> Result<Config, Box<dyn Error>> {
    // Load .env file if it exists
    dotenv::dotenv().ok();

    let config_path = env::var("CONFIG_FILE").unwrap_or_else(|_| "config".to_string());

    let config = ConfigFile::builder()
        // Start with default settings
        .add_source(File::with_name(&format!("{}.default", config_path)).required(false))
        // Add config file settings
        .add_source(File::with_name(&config_path).required(false))
        // Add environment variables (with prefix MAYYAM_)
        .add_source(Environment::with_prefix("MAYYAM").separator("__"))
        .build()?;

    let mut config: Config = config.try_deserialize()?;

    // Ensure kubernetes configuration exists even if not in config file
    if config.kubernetes.clusters.is_empty() {
        println!("Warning: No Kubernetes clusters configured. Add them to your config file.");
    }

    Ok(config)
}
