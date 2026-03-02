use super::GrpcClient;
use super::proto::config_service_client::ConfigServiceClient;
use super::proto::{
    DeleteConfigRequest, GetAllConfigRequest, ListKeyDefinitionsRequest, UpsertConfigRequest,
};

pub struct ConfigEntryItem {
    pub key: String,
    pub value: String,
    pub instance_id: String,
}

pub async fn get_all(
    client: &GrpcClient,
    instance_id: &str,
) -> Result<Vec<ConfigEntryItem>, String> {
    let channel = client.channel().await.map_err(|e| format!("{e:?}"))?;
    let mut svc = ConfigServiceClient::new(channel);
    let response = svc
        .get_all(GetAllConfigRequest {
            instance_id: instance_id.to_string(),
        })
        .await
        .map_err(|e| format!("{e:?}"))?;
    let entries = response
        .into_inner()
        .entries
        .into_iter()
        .map(|e| ConfigEntryItem {
            key: e.key,
            value: e.value,
            instance_id: e.instance_id,
        })
        .collect();
    Ok(entries)
}

pub async fn upsert(
    client: &GrpcClient,
    instance_id: &str,
    key: &str,
    value: &str,
    description: Option<&str>,
) -> Result<(), String> {
    let channel = client.channel().await.map_err(|e| format!("{e:?}"))?;
    let mut svc = ConfigServiceClient::new(channel);
    svc.upsert(UpsertConfigRequest {
        instance_id: instance_id.to_string(),
        key: key.to_string(),
        value: value.to_string(),
        description: description.map(|d| d.to_string()),
    })
    .await
    .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

pub async fn delete(client: &GrpcClient, instance_id: &str, key: &str) -> Result<(), String> {
    let channel = client.channel().await.map_err(|e| format!("{e:?}"))?;
    let mut svc = ConfigServiceClient::new(channel);
    svc.delete(DeleteConfigRequest {
        instance_id: instance_id.to_string(),
        key: key.to_string(),
    })
    .await
    .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

pub struct KeyDefinitionItem {
    pub key: String,
    pub description: String,
    pub type_name: String,
    pub resolved_value: String,
}

pub async fn list_key_definitions(client: &GrpcClient) -> Result<Vec<KeyDefinitionItem>, String> {
    let channel = client.channel().await.map_err(|e| format!("{e:?}"))?;
    let mut svc = ConfigServiceClient::new(channel);
    let response = svc
        .list_key_definitions(ListKeyDefinitionsRequest {})
        .await
        .map_err(|e| format!("{e:?}"))?;
    let items = response
        .into_inner()
        .definitions
        .into_iter()
        .map(|d| KeyDefinitionItem {
            key: d.key,
            description: d.description,
            type_name: d.type_name,
            resolved_value: d.resolved_value,
        })
        .collect();
    Ok(items)
}
