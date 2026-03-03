use crate::grpc::GrpcClient;
use crate::grpc::config::{ConfigEntryItem, KeyDefinitionItem};
use crate::{AppWindow, SlintConfigEntry, SlintKeyDefinition};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};

pub fn setup_config_callbacks(app: &AppWindow, client: GrpcClient) {
    // キー定義リファレンスを一度だけフェッチ
    fetch_key_definitions(app.as_weak(), client.clone());

    // 初回ロード
    {
        let instance_id = app.get_config_instance_id().to_string();
        refresh_config_list(app.as_weak(), client.clone(), instance_id);
    }

    // config-refresh
    let weak = app.as_weak();
    let c = client.clone();
    app.on_config_refresh(move || {
        let instance_id = get_instance_id(&weak);
        refresh_config_list(weak.clone(), c.clone(), instance_id);
    });

    // config-select-entry
    let weak = app.as_weak();
    app.on_config_select_entry(move |index| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let entries = app.get_config_entries();
        if let Some(entry) = entries.row_data(index as usize) {
            app.set_config_editing_index(index);
            app.set_config_edit_value(entry.value);
        }
    });

    // config-cancel-edit
    let weak = app.as_weak();
    app.on_config_cancel_edit(move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        app.set_config_editing_index(-1);
    });

    // config-upsert
    let weak = app.as_weak();
    let c = client.clone();
    app.on_config_upsert(move |key, value| {
        let instance_id = get_instance_id(&weak);
        let weak = weak.clone();
        let c = c.clone();
        let key = key.to_string();
        let value = value.to_string();
        spawn(async move {
            let result = crate::grpc::config::upsert(&c, &instance_id, &key, &value, None).await;
            let Some(app) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(()) => {
                    app.set_config_status_message("保存しました".into());
                    app.set_config_error_message("".into());
                    app.set_config_editing_index(-1);
                    app.set_config_new_key("".into());
                    app.set_config_new_value("".into());
                    let id = app.get_config_instance_id().to_string();
                    refresh_config_list(weak.clone(), c, id);
                }
                Err(e) => {
                    c.reset().await;
                    app.set_config_error_message(SharedString::from(e.clone()));
                    app.set_config_status_message("".into());
                    tracing::warn!("Config upsert failed: {e}");
                }
            }
        });
    });

    // config-delete
    let weak = app.as_weak();
    let c = client;
    app.on_config_delete(move |key| {
        let instance_id = get_instance_id(&weak);
        let weak = weak.clone();
        let c = c.clone();
        let key = key.to_string();
        spawn(async move {
            let result = crate::grpc::config::delete(&c, &instance_id, &key).await;
            let Some(app) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(()) => {
                    app.set_config_status_message("削除しました".into());
                    app.set_config_error_message("".into());
                    app.set_config_editing_index(-1);
                    app.set_config_new_key("".into());
                    app.set_config_new_value("".into());
                    let id = app.get_config_instance_id().to_string();
                    refresh_config_list(weak.clone(), c, id);
                }
                Err(e) => {
                    c.reset().await;
                    app.set_config_error_message(SharedString::from(e.clone()));
                    app.set_config_status_message("".into());
                    tracing::warn!("Config delete failed: {e}");
                }
            }
        });
    });
}

fn get_instance_id(weak: &Weak<AppWindow>) -> String {
    weak.upgrade()
        .map(|app| app.get_config_instance_id().to_string())
        .unwrap_or_default()
}

fn refresh_config_list(weak: Weak<AppWindow>, client: GrpcClient, instance_id: String) {
    if instance_id.is_empty() {
        if let Some(app) = weak.upgrade() {
            let empty: Vec<SlintConfigEntry> = vec![];
            app.set_config_entries(ModelRc::new(VecModel::from(empty)));
            app.set_config_loaded(false);
            app.set_config_error_message("".into());
            app.set_config_status_message("".into());
        }
        return;
    }
    spawn(async move {
        let result = crate::grpc::config::get_all(&client, &instance_id).await;
        let Some(app) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(entries) => {
                let slint_entries: Vec<SlintConfigEntry> =
                    entries.into_iter().map(to_slint_entry).collect();
                app.set_config_entries(ModelRc::new(VecModel::from(slint_entries)));
                app.set_config_loaded(true);
                app.set_config_error_message("".into());
                tracing::debug!("Config list refreshed for instance={instance_id}");
            }
            Err(e) => {
                client.reset().await;
                app.set_config_error_message(SharedString::from(e.clone()));
                app.set_config_status_message("".into());
                tracing::warn!("Config refresh failed: {e}");
            }
        }
    });
}

fn fetch_key_definitions(weak: Weak<AppWindow>, client: GrpcClient) {
    spawn(async move {
        let result = crate::grpc::config::list_key_definitions(&client).await;
        let Some(app) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(mut items) => {
                items.sort_by(|a, b| a.key.cmp(&b.key));
                let slint_defs: Vec<SlintKeyDefinition> =
                    items.into_iter().map(to_slint_key_definition).collect();
                app.set_key_definitions(ModelRc::new(VecModel::from(slint_defs)));
                app.set_key_definitions_error("".into());
                tracing::debug!("Key definitions loaded");
            }
            Err(e) => {
                client.reset().await;
                app.set_key_definitions_error(SharedString::from(e.clone()));
                tracing::warn!("Key definitions fetch failed: {e}");
            }
        }
    });
}

fn to_slint_key_definition(item: KeyDefinitionItem) -> SlintKeyDefinition {
    SlintKeyDefinition {
        key: SharedString::from(item.key),
        description: SharedString::from(item.description),
        type_name: SharedString::from(item.type_name),
        resolved_value: SharedString::from(item.resolved_value),
    }
}

fn to_slint_entry(item: ConfigEntryItem) -> SlintConfigEntry {
    SlintConfigEntry {
        key: SharedString::from(item.key),
        value: SharedString::from(item.value),
        instance_id: SharedString::from(item.instance_id),
    }
}

fn spawn(future: impl std::future::Future<Output = ()> + 'static) {
    if let Err(e) = slint::spawn_local(async_compat::Compat::new(future)) {
        tracing::error!("Failed to spawn config task: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_slint_entry_maps_all_fields() {
        let item = ConfigEntryItem {
            key: "my_key".to_string(),
            value: "my_value".to_string(),
            instance_id: "inst-001".to_string(),
        };
        let entry = to_slint_entry(item);
        assert_eq!(entry.key, "my_key");
        assert_eq!(entry.value, "my_value");
        assert_eq!(entry.instance_id, "inst-001");
    }

    #[test]
    fn to_slint_entry_handles_empty_strings() {
        let item = ConfigEntryItem {
            key: "".to_string(),
            value: "".to_string(),
            instance_id: "".to_string(),
        };
        let entry = to_slint_entry(item);
        assert_eq!(entry.key, "");
        assert_eq!(entry.value, "");
        assert_eq!(entry.instance_id, "");
    }

    #[test]
    fn to_slint_key_definition_maps_all_fields() {
        let item = KeyDefinitionItem {
            key: "TRADE_ENABLED".to_string(),
            description: "Whether trading is enabled".to_string(),
            type_name: "bool".to_string(),
            resolved_value: "false".to_string(),
        };
        let def = to_slint_key_definition(item);
        assert_eq!(def.key, "TRADE_ENABLED");
        assert_eq!(def.description, "Whether trading is enabled");
        assert_eq!(def.type_name, "bool");
        assert_eq!(def.resolved_value, "false");
    }

    #[test]
    fn to_slint_key_definition_handles_empty_strings() {
        let item = KeyDefinitionItem {
            key: "".to_string(),
            description: "".to_string(),
            type_name: "".to_string(),
            resolved_value: "".to_string(),
        };
        let def = to_slint_key_definition(item);
        assert_eq!(def.key, "");
        assert_eq!(def.description, "");
        assert_eq!(def.type_name, "");
        assert_eq!(def.resolved_value, "");
    }
}
