mod analytics;
mod commands;
mod state;
pub mod update;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_aptabase::Builder::new(analytics::APTABASE_KEY).build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Slave connection commands
            commands::create_slave_connection,
            commands::start_slave_connection,
            commands::stop_slave_connection,
            commands::delete_slave_connection,
            commands::list_slave_connections,
            // Slave device commands
            commands::add_slave_device,
            commands::remove_slave_device,
            commands::list_slave_devices,
            // Register commands
            commands::add_register,
            commands::remove_register,
            commands::read_register,
            commands::read_registers_bulk,
            commands::write_register,
            commands::list_registers,
            commands::export_registers,
            commands::import_registers,
            // Log commands
            commands::get_communication_logs,
            commands::clear_communication_logs,
            commands::export_logs_csv,
            // Tool commands
            commands::convert_plc_to_modbus,
            commands::convert_modbus_to_plc,
            commands::calculate_crc16,
            commands::calculate_lrc,
            commands::parse_hex,
            // State persistence commands
            commands::export_app_state,
            commands::import_app_state,
            commands::clear_app_state,
            // Simulation commands
            commands::random_mutate_registers,
            // Project file commands
            commands::save_project_file,
            commands::load_project_file,
            // Serial port commands
            commands::list_serial_ports,
            // Data source commands
            commands::set_data_source,
            commands::remove_data_source,
            commands::list_data_sources,
            commands::start_data_source_runner,
            // Update commands
            update::check_for_update,
            update::install_update,
            update::snooze_update,
            // Analytics commands
            analytics::get_analytics_enabled,
            analytics::set_analytics_enabled,
        ])
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            analytics::track_started(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                use tauri_plugin_aptabase::EventTracker;
                app_handle.flush_events_blocking();
            }
        });
}
