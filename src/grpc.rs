pub mod config;
pub mod health;
pub mod portfolio;

use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};

pub mod proto {
    tonic::include_proto!("zaciraci.v1");
}

fn grpc_url() -> String {
    std::env::var("ZCRC_GRPC_URL").unwrap_or_else(|_| "http://localhost:50051".to_string())
}

#[derive(Clone)]
pub struct GrpcClient {
    channel: Arc<RwLock<Option<Channel>>>,
    url: String,
}

impl GrpcClient {
    pub fn new() -> Self {
        let url = grpc_url();
        tracing::info!(url = %url, "gRPC client created");
        Self {
            channel: Arc::new(RwLock::new(None)),
            url,
        }
    }

    pub async fn channel(&self) -> Result<Channel, tonic::transport::Error> {
        {
            let guard = self.channel.read().await;
            if let Some(ch) = guard.as_ref() {
                return Ok(ch.clone());
            }
        }

        let mut endpoint = Endpoint::from_shared(self.url.clone()).expect("invalid gRPC URL");
        if self.url.starts_with("https://") {
            endpoint = endpoint
                .tls_config(ClientTlsConfig::new().with_native_roots())
                .expect("TLS config failed");
        }
        let ch = endpoint.connect().await?;

        let mut guard = self.channel.write().await;
        *guard = Some(ch.clone());
        Ok(ch)
    }

    pub async fn reset(&self) {
        let mut guard = self.channel.write().await;
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_client_with_empty_channel() {
        let client = GrpcClient::new();
        let channel = client.channel.blocking_read();
        assert!(channel.is_none());
    }

    #[test]
    fn new_stores_url() {
        let client = GrpcClient::new();
        assert!(!client.url.is_empty());
    }

    #[tokio::test]
    async fn reset_clears_channel() {
        let client = GrpcClient::new();
        client.reset().await;
        let guard = client.channel.read().await;
        assert!(guard.is_none());
    }
}
