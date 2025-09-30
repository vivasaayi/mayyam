use crate::{errors::AppError, models::cluster::KubernetesClusterConfig};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use kube::{
    config::{Config as KubeConfig, KubeConfigOptions, Kubeconfig},
    Client,
};
use once_cell::sync::Lazy;
use secrecy::Secret;
use sha2::{Digest, Sha256};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tracing::{debug, info};

// Simple global cache keyed by a fingerprint of the cluster config
static CLIENT_CACHE: Lazy<dashmap::DashMap<u64, Arc<Client>>> =
    Lazy::new(|| dashmap::DashMap::new());

fn b64_if_needed(data_opt: &Option<String>) -> Option<String> {
    match data_opt {
        Some(s) if s.is_empty() => None,
        Some(s) => {
            // Heuristic: if it decodes as base64, assume already base64; else encode
            let is_b64 = BASE64.decode(s.as_bytes()).is_ok();
            if is_b64 {
                Some(s.clone())
            } else {
                Some(BASE64.encode(s.as_bytes()))
            }
        }
        None => None,
    }
}

fn fingerprint(cfg: &KubernetesClusterConfig) -> u64 {
    let mut hasher = DefaultHasher::new();
    cfg.kube_config_path.hash(&mut hasher);
    cfg.kube_context.hash(&mut hasher);
    cfg.api_server_url.hash(&mut hasher);
    // Don't include raw secret materials; include their hashes
    if let Some(t) = &cfg.token {
        let mut sha = Sha256::new();
        sha.update(t.as_bytes());
        format!("{:x}", sha.finalize()).hash(&mut hasher);
    }
    for s in [
        &cfg.certificate_authority_data,
        &cfg.client_certificate_data,
        &cfg.client_key_data,
    ] {
        if let Some(v) = s {
            let mut sha = Sha256::new();
            sha.update(v.as_bytes());
            format!("{:x}", sha.finalize()).hash(&mut hasher);
        }
    }
    hasher.finish()
}

pub struct ClientFactory;

impl ClientFactory {
    pub async fn get_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        let key = fingerprint(cluster_config);
        if let Some(entry) = CLIENT_CACHE.get(&key) {
            return Ok((*entry.value()).as_ref().clone());
        }

        let client = Self::build_client(cluster_config).await?;
        CLIENT_CACHE.insert(key, Arc::new(client.clone()));
        Ok(client)
    }

    async fn build_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        // 1) If kubeconfig path provided, honor it with optional context
        if let Some(path) = &cluster_config.kube_config_path {
            debug!(target: "mayyam::k8s::client", kubeconfig_path = %path, ctx = ?cluster_config.kube_context, "Building client from kubeconfig path");
            let kubeconfig = Kubeconfig::read_from(path).map_err(|e| {
                AppError::ExternalService(format!("Failed to read kubeconfig from path: {}", e))
            })?;
            let cfg = KubeConfig::from_custom_kubeconfig(
                kubeconfig,
                &KubeConfigOptions {
                    context: cluster_config.kube_context.clone(),
                    cluster: None,
                    user: None,
                },
            )
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to create Kubernetes client config: {}",
                    e
                ))
            })?;
            return Client::try_from(cfg).map_err(|e| {
                AppError::ExternalService(format!("Failed to create Kubernetes client: {}", e))
            });
        }

        // 2) If api_server_url present, synthesize a Kubeconfig in-memory
        if let Some(server) = &cluster_config.api_server_url {
            debug!(target: "mayyam::k8s::client", api_server = %server, "Building client from direct api server details");
            let cluster_name = "cluster".to_string();
            let user_name = "user".to_string();
            let context_name = "context".to_string();

            let mut kubeconfig = Kubeconfig::default();
            kubeconfig.preferences = None;
            kubeconfig.current_context = Some(context_name.clone());

            // Cluster with optional inline cert data
            let mut named_cluster = kube::config::NamedCluster::default();
            named_cluster.name = cluster_name.clone();
            named_cluster.cluster = Some(kube::config::Cluster {
                server: Some(server.clone()),
                certificate_authority_data: b64_if_needed(
                    &cluster_config.certificate_authority_data,
                ),
                ..Default::default()
            });
            kubeconfig.clusters.push(named_cluster);

            // User auth: prefer token; optionally support client cert/key
            let mut named_user = kube::config::NamedAuthInfo::default();
            named_user.name = user_name.clone();
            let mut auth = kube::config::AuthInfo::default();
            if let Some(token) = &cluster_config.token {
                auth.token = Some(Secret::new(token.clone()));
            }
            auth.client_certificate_data = b64_if_needed(&cluster_config.client_certificate_data);
            auth.client_key_data = b64_if_needed(&cluster_config.client_key_data).map(Secret::new);
            named_user.auth_info = Some(auth);
            kubeconfig.auth_infos.push(named_user);

            // Context
            let mut named_ctx = kube::config::NamedContext::default();
            named_ctx.name = context_name.clone();
            named_ctx.context = Some(kube::config::Context {
                cluster: cluster_name,
                user: user_name,
                namespace: None,
                ..Default::default()
            });
            kubeconfig.contexts.push(named_ctx);

            let cfg = KubeConfig::from_custom_kubeconfig(
                kubeconfig,
                &KubeConfigOptions {
                    context: Some(context_name),
                    cluster: None,
                    user: None,
                },
            )
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to create Kubernetes config from direct settings: {}",
                    e
                ))
            })?;

            return Client::try_from(cfg).map_err(|e| {
                AppError::ExternalService(format!("Failed to create Kubernetes client: {}", e))
            });
        }

        // 3) Fallback: infer from environment (in-cluster or default kubeconfig)
        info!(
            target = "mayyam::k8s::client",
            "Inferring Kubernetes config (no kubeconfig path or server provided)"
        );
        let infer_config = kube::Config::infer().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to infer Kubernetes config: {}", e))
        })?;
        Client::try_from(infer_config).map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to create Kubernetes client from inferred config: {}",
                e
            ))
        })
    }
}
