use crate::grpc::GrpcClient;
use crate::grpc::config::ConfigEntryItem;
use crate::{AppWindow, SlintConfigEntry};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};

pub fn setup_config_callbacks(app: &AppWindow, client: GrpcClient) {
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
