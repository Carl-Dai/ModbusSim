use serde_json::json;
use tauri::AppHandle;
use tauri_plugin_aptabase::EventTracker;
use tauri_plugin_store::StoreExt;

/// 非机密：Aptabase App Key 随客户端分发，可直接提交。两端共用同一个 Key。
pub const APTABASE_KEY: &str = "A-US-1280171703";
const EDITION: &str = "slave";
const STORE_FILE: &str = "settings.json";
const KEY_ENABLED: &str = "analytics_enabled";

/// 读开关，缺省 true（opt-out）。
fn is_enabled(app: &AppHandle) -> bool {
    app.store(STORE_FILE)
        .ok()
        .and_then(|s| s.get(KEY_ENABLED))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// 启动时调用：开启则上报 app_started（带 edition 区分两端）。
pub fn track_started(app: &AppHandle) {
    if is_enabled(app) {
        let _ = app.track_event("app_started", Some(json!({ "edition": EDITION })));
    }
}

#[tauri::command]
pub fn get_analytics_enabled(app: AppHandle) -> bool {
    is_enabled(&app)
}

#[tauri::command]
pub fn set_analytics_enabled(app: AppHandle, enabled: bool) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.set(KEY_ENABLED, serde_json::Value::Bool(enabled));
        let _ = store.save();
    }
}
