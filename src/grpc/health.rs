use super::GrpcClient;
use super::proto::HealthCheckRequest;
use super::proto::health_service_client::HealthServiceClient;

pub struct HealthStatus {
    pub healthy: bool,
    pub database_status: String,
}

pub async fn check(client: &GrpcClient) -> Result<HealthStatus, String> {
    let channel = client.channel().await.map_err(|e| format!("{e:?}"))?;
    let mut health_client = HealthServiceClient::new(channel);
    let response = health_client
        .check(HealthCheckRequest {})
        .await
        .map_err(|e| format!("{e:?}"))?;
    let resp = response.into_inner();
    Ok(HealthStatus {
        healthy: resp.healthy,
        database_status: resp.database_status,
    })
}
