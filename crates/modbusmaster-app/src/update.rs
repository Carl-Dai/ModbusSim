use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration as StdDuration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::{sync::Mutex, time::timeout};

const STORE_FILE: &str = "update_state.json";
const KEY_LAST_CHECK: &str = "last_check_at";
const KEY_SKIPPED_VERSION: &str = "skipped_version";
const KEY_INSTALL_ON_NEXT_LAUNCH: &str = "install_on_next_launch";
const THROTTLE_HOURS: i64 = 6;
const CHECK_ENDPOINT_TIMEOUT: StdDuration = StdDuration::from_secs(10);
const CHECK_TOTAL_TIMEOUT: StdDuration = StdDuration::from_secs(30);
const DOWNLOAD_TIMEOUT: StdDuration = StdDuration::from_secs(5 * 60);
const PROGRESS_EVENT: &str = "update-progress";
const CHECK_TIMEOUT_ERROR: &str = "UPDATE_CHECK_TIMEOUT";
const DOWNLOAD_TIMEOUT_ERROR: &str = "UPDATE_DOWNLOAD_TIMEOUT";

#[derive(Serialize, Clone)]
pub struct UpdateMeta {
    pub version: String,
    pub notes: String,
    pub pub_date: Option<String>,
}

#[derive(Serialize, Clone)]
struct UpdateProgress {
    stage: &'static str,
    downloaded: u64,
    total: Option<u64>,
    percent: Option<u8>,
}

struct PreparedUpdate {
    meta: UpdateMeta,
    update: Update,
    bytes: Vec<u8>,
}

#[derive(Default)]
pub struct UpdateState {
    prepared: Mutex<Option<PreparedUpdate>>,
}

fn read_str(app: &AppHandle, key: &str) -> Option<String> {
    let store = app.store(STORE_FILE).ok()?;
    store.get(key).and_then(|v| v.as_str().map(String::from))
}

fn read_bool(app: &AppHandle, key: &str) -> bool {
    let Ok(store) = app.store(STORE_FILE) else {
        return false;
    };
    store.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn write_str(app: &AppHandle, key: &str, value: &str) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.set(key, serde_json::Value::String(value.to_string()));
        let _ = store.save();
    }
}

fn write_bool(app: &AppHandle, key: &str, value: bool) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.set(key, serde_json::Value::Bool(value));
        let _ = store.save();
    }
}

fn remove_value(app: &AppHandle, key: &str) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.delete(key);
        let _ = store.save();
    }
}

fn parse_ts(s: Option<String>) -> Option<DateTime<Utc>> {
    s.and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn update_meta(update: &Update) -> UpdateMeta {
    UpdateMeta {
        version: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
        pub_date: update.date.map(|d| d.to_string()),
    }
}

fn progress_percent(downloaded: u64, total: Option<u64>) -> Option<u8> {
    total
        .filter(|total| *total > 0)
        .map(|total| ((downloaded.saturating_mul(100) / total).min(100)) as u8)
}

fn emit_update_progress(app: &AppHandle, stage: &'static str, downloaded: u64, total: Option<u64>) {
    let _ = app.emit(
        PROGRESS_EVENT,
        UpdateProgress {
            stage,
            downloaded,
            total,
            percent: progress_percent(downloaded, total),
        },
    );
}

async fn find_update(app: &AppHandle) -> Result<Option<Update>, String> {
    emit_update_progress(app, "checking", 0, None);
    let result = async {
        let updater = app
            .updater_builder()
            .timeout(CHECK_ENDPOINT_TIMEOUT)
            .build()
            .map_err(|e| e.to_string())?;
        timeout(CHECK_TOTAL_TIMEOUT, updater.check())
            .await
            .map_err(|_| CHECK_TIMEOUT_ERROR.to_string())?
            .map_err(|e| e.to_string())
    }
    .await;
    if result.is_err() {
        emit_update_progress(app, "idle", 0, None);
    }
    result
}

async fn download_update(app: &AppHandle, update: &Update) -> Result<Vec<u8>, String> {
    emit_update_progress(app, "downloading", 0, None);
    let progress_app = app.clone();
    let verify_app = app.clone();
    let downloaded = Arc::new(AtomicU64::new(0));
    let progress_downloaded = Arc::clone(&downloaded);
    let verify_downloaded = Arc::clone(&downloaded);
    let mut last_percent = None;
    let mut last_emitted_bytes = 0_u64;
    let download = update.download(
        move |chunk_len, total| {
            let downloaded = progress_downloaded
                .fetch_add(chunk_len as u64, Ordering::Relaxed)
                .saturating_add(chunk_len as u64);
            let percent = progress_percent(downloaded, total);
            let should_emit = percent != last_percent
                || (total.is_none() && downloaded.saturating_sub(last_emitted_bytes) >= 256 * 1024);
            if should_emit {
                emit_update_progress(&progress_app, "downloading", downloaded, total);
                last_percent = percent;
                last_emitted_bytes = downloaded;
            }
        },
        move || {
            emit_update_progress(
                &verify_app,
                "verifying",
                verify_downloaded.load(Ordering::Relaxed),
                None,
            );
            log::info!("update download finished; verifying release signature");
        },
    );

    let result = match timeout(DOWNLOAD_TIMEOUT, download).await {
        Ok(result) => result.map_err(|e| e.to_string()),
        Err(_) => Err(DOWNLOAD_TIMEOUT_ERROR.to_string()),
    };
    match result {
        Ok(bytes) => {
            emit_update_progress(app, "ready", bytes.len() as u64, Some(bytes.len() as u64));
            Ok(bytes)
        }
        Err(error) => {
            emit_update_progress(app, "idle", 0, None);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
    force: Option<bool>,
) -> Result<Option<UpdateMeta>, String> {
    let force = force.unwrap_or(false);
    if !force && read_bool(&app, KEY_INSTALL_ON_NEXT_LAUNCH) {
        return Ok(None);
    }

    let mut prepared = state.prepared.lock().await;
    if let Some(update) = prepared.as_ref() {
        emit_update_progress(
            &app,
            "ready",
            update.bytes.len() as u64,
            Some(update.bytes.len() as u64),
        );
        return Ok(Some(update.meta.clone()));
    }

    let now = Utc::now();
    if !force {
        let last = parse_ts(read_str(&app, KEY_LAST_CHECK));
        if !should_check(last, now, Duration::hours(THROTTLE_HOURS)) {
            emit_update_progress(&app, "idle", 0, None);
            return Ok(None);
        }
    }
    write_str(&app, KEY_LAST_CHECK, &now.to_rfc3339());

    let Some(update) = find_update(&app).await? else {
        emit_update_progress(&app, "idle", 0, None);
        return Ok(None);
    };
    if !force
        && is_skipped(
            read_str(&app, KEY_SKIPPED_VERSION).as_deref(),
            &update.version,
        )
    {
        emit_update_progress(&app, "idle", 0, None);
        return Ok(None);
    }

    let meta = update_meta(&update);
    let bytes = download_update(&app, &update).await?;
    *prepared = Some(PreparedUpdate {
        meta: meta.clone(),
        update,
        bytes,
    });
    Ok(Some(meta))
}

#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, UpdateState>) -> Result<(), String> {
    let prepared = state.prepared.lock().await;
    let ready = prepared
        .as_ref()
        .ok_or_else(|| "update package is not ready".to_string())?;
    ready
        .update
        .install(&ready.bytes)
        .map_err(|e| e.to_string())?;
    remove_value(&app, KEY_SKIPPED_VERSION);
    remove_value(&app, KEY_INSTALL_ON_NEXT_LAUNCH);
    drop(prepared);
    app.restart()
}

#[tauri::command]
pub async fn skip_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
    version: String,
) -> Result<(), String> {
    let mut prepared = state.prepared.lock().await;
    if !prepared
        .as_ref()
        .is_some_and(|update| update.meta.version == version)
    {
        return Err("update package is not ready".to_string());
    }
    *prepared = None;
    write_str(&app, KEY_SKIPPED_VERSION, &version);
    remove_value(&app, KEY_INSTALL_ON_NEXT_LAUNCH);
    Ok(())
}

#[tauri::command]
pub async fn schedule_update_on_next_launch(
    app: AppHandle,
    state: State<'_, UpdateState>,
    version: String,
) -> Result<(), String> {
    let prepared = state.prepared.lock().await;
    if !prepared
        .as_ref()
        .is_some_and(|update| update.meta.version == version)
    {
        return Err("update package is not ready".to_string());
    }
    write_bool(&app, KEY_INSTALL_ON_NEXT_LAUNCH, true);
    remove_value(&app, KEY_SKIPPED_VERSION);
    Ok(())
}

pub async fn install_pending_update(app: AppHandle) -> Result<(), String> {
    if !read_bool(&app, KEY_INSTALL_ON_NEXT_LAUNCH) {
        return Ok(());
    }

    let state = app.state::<UpdateState>();
    let mut prepared = state.prepared.lock().await;
    let Some(update) = find_update(&app).await? else {
        emit_update_progress(&app, "idle", 0, None);
        remove_value(&app, KEY_INSTALL_ON_NEXT_LAUNCH);
        return Ok(());
    };
    let meta = update_meta(&update);
    let bytes = download_update(&app, &update).await?;
    *prepared = Some(PreparedUpdate {
        meta,
        update,
        bytes,
    });
    let ready = prepared
        .as_ref()
        .expect("prepared update was just inserted");
    ready
        .update
        .install(&ready.bytes)
        .map_err(|e| e.to_string())?;
    remove_value(&app, KEY_SKIPPED_VERSION);
    remove_value(&app, KEY_INSTALL_ON_NEXT_LAUNCH);
    drop(prepared);
    app.restart()
}

pub fn should_check(
    last_check: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    throttle: Duration,
) -> bool {
    match last_check {
        None => true,
        Some(last) => now - last >= throttle,
    }
}

pub fn is_skipped(skipped_version: Option<&str>, remote_version: &str) -> bool {
    skipped_version == Some(remote_version)
}

#[cfg(test)]
mod tests {
    use super::progress_percent;

    #[test]
    fn progress_requires_a_non_zero_total() {
        assert_eq!(progress_percent(25, None), None);
        assert_eq!(progress_percent(25, Some(0)), None);
    }

    #[test]
    fn progress_is_an_integer_percentage_capped_at_100() {
        assert_eq!(progress_percent(25, Some(100)), Some(25));
        assert_eq!(progress_percent(200, Some(100)), Some(100));
    }
}
