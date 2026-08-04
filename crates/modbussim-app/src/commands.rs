//! Tauri commands for ModbusSim.
//!
//! These commands are invoked from the frontend via the Tauri IPC bridge.

use crate::state::{
    AppState, MutationKey, MutationRuntimeState, RegisterValueInfo, SlaveConnectionInfo,
    SlaveConnectionState, SlaveDeviceInfo, MUTATION_BASE_TICK_MS,
};
use modbussim_core::data_source::{DataSource, DataSourceConfig, DataSourceState};
use modbussim_core::log_collector::LogCollector;
use modbussim_core::log_entry::LogEntry;
use modbussim_core::log_helpers;
use modbussim_core::mutation::{MutationConfig, MutationMode};
use modbussim_core::parse::{
    parse_data_type, parse_endian, parse_register_type, register_type_to_str,
};
use modbussim_core::project::{self, ProjectFile};
use modbussim_core::register::{
    occupied_address_range, register_definitions_overlap, Endian, RegisterDef, RegisterType,
};
use modbussim_core::slave::{SlaveConnection, SlaveDevice};
use modbussim_core::tools;
use modbussim_core::transport::{self, Parity, SerialConfig, SlaveTlsConfig, Transport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

// ---------------------------------------------------------------------------
// Event Payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SlaveConnectionEvent {
    pub id: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LogAppendedEvent {
    pub connection_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RegisterChangePayload {
    pub slave_id: u8,
    pub register_type: &'static str,
    pub address: u16,
    pub value: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RegisterValueEvent {
    pub connection_id: String,
    pub changes: Vec<RegisterChangePayload>,
}

// ---------------------------------------------------------------------------
// Transport Request Types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportRequest {
    Tcp {
        port: u16,
    },
    TcpTls {
        port: u16,
    },
    Rtu {
        serial_port: String,
        baud_rate: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: String,
    },
    Ascii {
        serial_port: String,
        baud_rate: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: String,
    },
    RtuOverTcp {
        host: String,
        port: u16,
    },
}

fn parse_parity(s: &str) -> Parity {
    match s {
        "odd" => Parity::Odd,
        "even" => Parity::Even,
        _ => Parity::None,
    }
}

fn to_transport(req: &TransportRequest) -> Transport {
    match req {
        TransportRequest::Tcp { port } => Transport::Tcp {
            host: "0.0.0.0".into(),
            port: *port,
        },
        TransportRequest::TcpTls { port } => Transport::TcpTls {
            host: "0.0.0.0".into(),
            port: *port,
        },
        TransportRequest::Rtu {
            serial_port,
            baud_rate,
            data_bits,
            stop_bits,
            parity,
        } => Transport::Rtu(SerialConfig {
            port: serial_port.clone(),
            baud_rate: *baud_rate,
            data_bits: *data_bits,
            stop_bits: *stop_bits,
            parity: parse_parity(parity),
        }),
        TransportRequest::Ascii {
            serial_port,
            baud_rate,
            data_bits,
            stop_bits,
            parity,
        } => Transport::Ascii(SerialConfig {
            port: serial_port.clone(),
            baud_rate: *baud_rate,
            data_bits: *data_bits,
            stop_bits: *stop_bits,
            parity: parse_parity(parity),
        }),
        TransportRequest::RtuOverTcp { host, port } => Transport::RtuOverTcp {
            host: host.clone(),
            port: *port,
        },
    }
}

// ---------------------------------------------------------------------------
// Slave Connection Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateSlaveRequest {
    pub transport: TransportRequest,
    pub init_mode: Option<String>,
    #[serde(default = "default_modbus_slave_id")]
    pub slave_id: u8,
    #[serde(default)]
    pub name: String,
    pub use_tls: Option<bool>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub ca_file: Option<String>,
    pub require_client_cert: Option<bool>,
    pub pkcs12_file: Option<String>,
    pub pkcs12_password: Option<String>,
}

fn default_modbus_slave_id() -> u8 {
    1
}

#[tauri::command]
pub async fn create_slave_connection(
    state: State<'_, AppState>,
    request: CreateSlaveRequest,
) -> Result<SlaveConnectionInfo, String> {
    if !(1..=247).contains(&request.slave_id) {
        return Err("slave_id must be between 1 and 247".to_string());
    }
    let id = {
        let mut counter = state.next_slave_id.write().await;
        let id = format!("slave_{}", *counter);
        *counter += 1;
        id
    };

    let transport = to_transport(&request.transport);

    let (bind_address, port) = match &transport {
        Transport::Tcp { host, port }
        | Transport::TcpTls { host, port }
        | Transport::RtuOverTcp { host, port } => (host.clone(), *port),
        Transport::Rtu(sc) | Transport::Ascii(sc) => (sc.port.clone(), 0),
    };

    let log_collector = Arc::new(LogCollector::new());
    let connection = SlaveConnection::new(transport);
    let connection = connection.with_log_collector(log_collector.clone());

    let connection = if request.use_tls.unwrap_or(false) {
        connection.with_tls_config(SlaveTlsConfig {
            enabled: true,
            cert_file: request.cert_file.unwrap_or_default(),
            key_file: request.key_file.unwrap_or_default(),
            ca_file: request.ca_file.unwrap_or_default(),
            require_client_cert: request.require_client_cert.unwrap_or(false),
            pkcs12_file: request.pkcs12_file.unwrap_or_default(),
            pkcs12_password: request.pkcs12_password.unwrap_or_default(),
        })
    } else {
        connection
    };

    // Auto-create the initial slave device with user-supplied identity.
    let default_device = match request.init_mode.as_deref() {
        Some("random") => SlaveDevice::with_random_registers(request.slave_id, request.name, 20000),
        _ => SlaveDevice::with_default_registers(request.slave_id, request.name, 20000),
    };
    connection
        .add_device(default_device)
        .await
        .map_err(|e| format!("failed to add default device: {}", e))?;

    let info = SlaveConnectionInfo {
        id: id.clone(),
        bind_address,
        port,
        state: format!("{:?}", connection.state()),
        device_count: 1,
    };

    state.slave_connections.write().await.insert(
        id,
        SlaveConnectionState {
            connection,
            log_collector,
        },
    );

    Ok(info)
}

#[tauri::command]
pub async fn start_slave_connection(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let state_str: String;
    {
        let mut connections = state.slave_connections.write().await;
        let conn = connections
            .get_mut(&id)
            .ok_or_else(|| format!("connection {} not found", id))?;

        let conn_id: std::sync::Arc<str> = std::sync::Arc::from(id.as_str());
        let cb_handle = app_handle.clone();
        let cb_conn_id = conn_id.clone();
        conn.connection.set_change_callback(std::sync::Arc::new(
            move |changes: &[modbussim_core::slave::RegisterChange]| {
                let event = RegisterValueEvent {
                    connection_id: cb_conn_id.to_string(),
                    changes: changes
                        .iter()
                        .map(|c| RegisterChangePayload {
                            slave_id: c.slave_id,
                            register_type: register_type_to_str(c.register_type),
                            address: c.address,
                            value: c.value,
                        })
                        .collect(),
                };
                let _ = cb_handle.emit("register-value-changed", event);
            },
        ));

        let log_handle = app_handle.clone();
        let log_conn_id = conn_id;
        conn.log_collector
            .set_append_callback(std::sync::Arc::new(move |_entry| {
                let _ = log_handle.emit(
                    "log-appended",
                    LogAppendedEvent {
                        connection_id: log_conn_id.to_string(),
                    },
                );
            }));

        conn.connection
            .start()
            .await
            .map_err(|e| format!("failed to start: {}", e))?;
        state_str = format!("{:?}", conn.connection.state());
    }

    let event = SlaveConnectionEvent {
        id: id.clone(),
        state: state_str,
    };
    app_handle
        .emit("slave-connection-state", event)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn stop_slave_connection(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let state_str: String;
    {
        let mut connections = state.slave_connections.write().await;
        let conn = connections
            .get_mut(&id)
            .ok_or_else(|| format!("connection {} not found", id))?;

        conn.connection
            .stop()
            .await
            .map_err(|e| format!("failed to stop: {}", e))?;
        state_str = format!("{:?}", conn.connection.state());
    }

    let event = SlaveConnectionEvent {
        id: id.clone(),
        state: state_str,
    };
    app_handle
        .emit("slave-connection-state", event)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_slave_connection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut connections = state.slave_connections.write().await;
    let connection = connections
        .get_mut(&id)
        .ok_or_else(|| format!("connection {} not found", id))?;
    connection
        .connection
        .stop()
        .await
        .map_err(|e| format!("failed to stop connection: {}", e))?;
    connections.remove(&id);
    drop(connections);
    state
        .mutation_runtime
        .write()
        .await
        .retain(|key, _| key.connection_id != id);
    let prefix = format!("{}:", id);
    state
        .data_sources
        .write()
        .await
        .retain(|key, _| !key.starts_with(&prefix));
    Ok(())
}

#[tauri::command]
pub async fn list_slave_connections(
    state: State<'_, AppState>,
) -> Result<Vec<SlaveConnectionInfo>, String> {
    let connections = state.slave_connections.read().await;
    let mut result = Vec::new();

    for (id, conn_state) in connections.iter() {
        let device_count = conn_state.connection.devices.read().await.len();
        let (bind_address, port) = match &conn_state.connection.transport {
            Transport::Tcp { host, port }
            | Transport::TcpTls { host, port }
            | Transport::RtuOverTcp { host, port } => (host.clone(), *port),
            Transport::Rtu(sc) | Transport::Ascii(sc) => (sc.port.clone(), 0),
        };
        result.push(SlaveConnectionInfo {
            id: id.clone(),
            bind_address,
            port,
            state: format!("{:?}", conn_state.connection.state()),
            device_count,
        });
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Slave Device Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddSlaveDeviceRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub name: String,
    pub init_mode: Option<String>,
}

#[tauri::command]
pub async fn add_slave_device(
    state: State<'_, AppState>,
    request: AddSlaveDeviceRequest,
) -> Result<SlaveDeviceInfo, String> {
    if !(1..=247).contains(&request.slave_id) {
        return Err("slave_id must be between 1 and 247".to_string());
    }
    let mut connections = state.slave_connections.write().await;
    let conn = connections
        .get_mut(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let name = request.name.clone();
    let device = match request.init_mode.as_deref() {
        Some("random") => SlaveDevice::with_random_registers(request.slave_id, name.clone(), 20000),
        Some("zero") => SlaveDevice::with_default_registers(request.slave_id, name.clone(), 20000),
        _ => SlaveDevice::new(request.slave_id, name.clone()),
    };
    let register_count = device.register_defs.len();
    conn.connection
        .add_device(device)
        .await
        .map_err(|e| format!("failed to add device: {}", e))?;

    Ok(SlaveDeviceInfo {
        slave_id: request.slave_id,
        name,
        register_count,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateSlaveDeviceRequest {
    pub connection_id: String,
    pub original_slave_id: u8,
    pub slave_id: u8,
    pub name: String,
}

#[tauri::command]
pub async fn update_slave_device(
    state: State<'_, AppState>,
    request: UpdateSlaveDeviceRequest,
) -> Result<SlaveDeviceInfo, String> {
    if !(1..=247).contains(&request.slave_id) {
        return Err("slave_id must be between 1 and 247".to_string());
    }
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;
    let mut devices = conn.connection.devices.write().await;
    if request.slave_id != request.original_slave_id && devices.contains_key(&request.slave_id) {
        return Err(format!("slave ID {} already exists", request.slave_id));
    }
    let mut device = devices
        .remove(&request.original_slave_id)
        .ok_or_else(|| format!("slave {} not found", request.original_slave_id))?;
    device.slave_id = request.slave_id;
    device.name = request.name.clone();
    let register_count = device.register_defs.len();
    devices.insert(request.slave_id, device);
    drop(devices);
    drop(connections);

    state.mutation_runtime.write().await.retain(|key, _| {
        key.connection_id != request.connection_id || key.slave_id != request.original_slave_id
    });
    if request.slave_id != request.original_slave_id {
        let old_prefix = format!("{}:{}:", request.connection_id, request.original_slave_id);
        let new_prefix = format!("{}:{}:", request.connection_id, request.slave_id);
        let mut data_sources = state.data_sources.write().await;
        let moved: Vec<_> = data_sources
            .keys()
            .filter(|key| key.starts_with(&old_prefix))
            .cloned()
            .collect();
        for old_key in moved {
            if let Some(value) = data_sources.remove(&old_key) {
                data_sources.insert(old_key.replacen(&old_prefix, &new_prefix, 1), value);
            }
        }
    }

    Ok(SlaveDeviceInfo {
        slave_id: request.slave_id,
        name: request.name,
        register_count,
    })
}

#[tauri::command]
pub async fn remove_slave_device(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
) -> Result<(), String> {
    let mut connections = state.slave_connections.write().await;
    let conn = connections
        .get_mut(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    conn.connection
        .remove_device(slave_id)
        .await
        .map_err(|e| format!("failed to remove device: {}", e))?;
    drop(connections);
    state
        .mutation_runtime
        .write()
        .await
        .retain(|key, _| key.connection_id != connection_id || key.slave_id != slave_id);
    let prefix = format!("{}:{}:", connection_id, slave_id);
    state
        .data_sources
        .write()
        .await
        .retain(|key, _| !key.starts_with(&prefix));

    Ok(())
}

#[tauri::command]
pub async fn list_slave_devices(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<SlaveDeviceInfo>, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let devices = conn.connection.devices.read().await;
    let result: Vec<SlaveDeviceInfo> = devices
        .values()
        .map(|d| SlaveDeviceInfo {
            slave_id: d.slave_id,
            name: d.name.clone(),
            register_count: d.register_defs.len(),
        })
        .collect();

    Ok(result)
}

// ---------------------------------------------------------------------------
// Register Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddRegisterRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub address: u16,
    pub register_type: String,
    pub data_type: String,
    pub endian: Option<String>,
    pub name: Option<String>,
    pub comment: Option<String>,
}

fn register_def_from_request(request: &AddRegisterRequest) -> Result<RegisterDef, String> {
    Ok(RegisterDef {
        address: request.address,
        register_type: parse_register_type(&request.register_type)?,
        data_type: parse_data_type(&request.data_type)?,
        endian: match &request.endian {
            Some(value) => parse_endian(value)?,
            None => Endian::Big,
        },
        name: request.name.clone().unwrap_or_default(),
        comment: request.comment.clone().unwrap_or_default(),
        mutation: None,
    })
}

fn validate_register_definition_set(definitions: &[RegisterDef]) -> Result<(), String> {
    for (index, definition) in definitions.iter().enumerate() {
        if occupied_address_range(definition).is_none() {
            return Err(format!(
                "register {}@{} exceeds address 65535",
                register_type_to_str(definition.register_type),
                definition.address
            ));
        }
        if let Some(other) = definitions
            .iter()
            .skip(index + 1)
            .find(|other| register_definitions_overlap(definition, other))
        {
            return Err(format!(
                "register {}@{} overlaps point at address {}",
                register_type_to_str(definition.register_type),
                definition.address,
                other.address
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WriteRegisterRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub register_type: String,
    pub address: u16,
    pub value: u16,
}

#[tauri::command]
pub async fn add_register(
    state: State<'_, AppState>,
    request: AddRegisterRequest,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let def = register_def_from_request(&request)?;

    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;

    let mut prospective = device.register_defs.clone();
    prospective.push(def.clone());
    validate_register_definition_set(&prospective)?;
    device.register_map.ensure_from_def(&def);
    device.register_defs.push(def);
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateRegisterRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub original_address: u16,
    pub original_register_type: String,
    pub address: u16,
    pub register_type: String,
    pub data_type: String,
    pub endian: Option<String>,
    pub name: Option<String>,
    pub comment: Option<String>,
}

#[tauri::command]
pub async fn update_register(
    state: State<'_, AppState>,
    request: UpdateRegisterRequest,
) -> Result<(), String> {
    let original_type = parse_register_type(&request.original_register_type)?;
    let replacement_request = AddRegisterRequest {
        connection_id: request.connection_id.clone(),
        slave_id: request.slave_id,
        address: request.address,
        register_type: request.register_type,
        data_type: request.data_type,
        endian: request.endian,
        name: request.name,
        comment: request.comment,
    };
    let mut replacement = register_def_from_request(&replacement_request)?;
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;
    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;
    let index = device
        .register_defs
        .iter()
        .position(|definition| {
            definition.address == request.original_address
                && definition.register_type == original_type
        })
        .ok_or_else(|| "original register not found".to_string())?;
    replacement.mutation = device.register_defs[index].mutation.clone();
    let mut prospective = device.register_defs.clone();
    prospective[index] = replacement.clone();
    validate_register_definition_set(&prospective)?;
    device.register_map.ensure_from_def(&replacement);
    device.register_defs[index] = replacement;
    drop(devices);
    drop(connections);
    state
        .mutation_runtime
        .write()
        .await
        .remove(&MutationKey::new(
            request.connection_id.clone(),
            request.slave_id,
            original_type,
            request.original_address,
        ));
    let old_data_source_key = format!(
        "{}:{}:{}:{}",
        request.connection_id,
        request.slave_id,
        register_type_to_str(original_type),
        request.original_address
    );
    let new_data_source_key = format!(
        "{}:{}:{}:{}",
        request.connection_id,
        request.slave_id,
        replacement_request.register_type,
        replacement_request.address
    );
    if old_data_source_key != new_data_source_key {
        let mut data_sources = state.data_sources.write().await;
        if let Some(value) = data_sources.remove(&old_data_source_key) {
            data_sources.insert(new_data_source_key, value);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_register(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
    address: u16,
    register_type: String,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let reg_type = parse_register_type(&register_type)?;

    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&slave_id)
        .ok_or_else(|| format!("slave {} not found", slave_id))?;

    device
        .register_defs
        .retain(|d| !(d.address == address && d.register_type == reg_type));
    drop(devices);
    drop(connections);
    state
        .mutation_runtime
        .write()
        .await
        .remove(&MutationKey::new(
            connection_id.clone(),
            slave_id,
            reg_type,
            address,
        ));
    let data_source_key = format!(
        "{}:{}:{}:{}",
        connection_id, slave_id, register_type, address
    );
    state.data_sources.write().await.remove(&data_source_key);
    Ok(())
}

#[tauri::command]
pub async fn read_register(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
    register_type: String,
    address: u16,
) -> Result<RegisterValueInfo, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let reg_type = parse_register_type(&register_type)?;

    let devices = conn.connection.devices.read().await;
    let device = devices
        .get(&slave_id)
        .ok_or_else(|| format!("slave {} not found", slave_id))?;

    let value = match reg_type {
        RegisterType::Coil => device
            .register_map
            .coils
            .get(&address)
            .copied()
            .unwrap_or(false) as u16,
        RegisterType::DiscreteInput => device
            .register_map
            .discrete_inputs
            .get(&address)
            .copied()
            .unwrap_or(false) as u16,
        RegisterType::HoldingRegister => device
            .register_map
            .holding_registers
            .get(&address)
            .copied()
            .unwrap_or(0),
        RegisterType::InputRegister => device
            .register_map
            .input_registers
            .get(&address)
            .copied()
            .unwrap_or(0),
    };

    Ok(RegisterValueInfo { address, value })
}

#[tauri::command]
pub async fn read_registers_bulk(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
    register_type: String,
) -> Result<Vec<RegisterValueInfo>, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let reg_type = parse_register_type(&register_type)?;

    let devices = conn.connection.devices.read().await;
    let device = devices
        .get(&slave_id)
        .ok_or_else(|| format!("slave {} not found", slave_id))?;

    let mut out = Vec::with_capacity(device.register_defs.len());
    for def in device
        .register_defs
        .iter()
        .filter(|d| d.register_type == reg_type)
    {
        let value = match reg_type {
            RegisterType::Coil => device
                .register_map
                .coils
                .get(&def.address)
                .copied()
                .unwrap_or(false) as u16,
            RegisterType::DiscreteInput => device
                .register_map
                .discrete_inputs
                .get(&def.address)
                .copied()
                .unwrap_or(false) as u16,
            RegisterType::HoldingRegister => device
                .register_map
                .holding_registers
                .get(&def.address)
                .copied()
                .unwrap_or(0),
            RegisterType::InputRegister => device
                .register_map
                .input_registers
                .get(&def.address)
                .copied()
                .unwrap_or(0),
        };
        out.push(RegisterValueInfo {
            address: def.address,
            value,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn write_register(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    request: WriteRegisterRequest,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let reg_type = parse_register_type(&request.register_type)?;

    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;

    match reg_type {
        RegisterType::Coil => device
            .register_map
            .write_coil(request.address, request.value != 0),
        RegisterType::DiscreteInput => {
            device
                .register_map
                .discrete_inputs
                .insert(request.address, request.value != 0);
        }
        RegisterType::HoldingRegister => device
            .register_map
            .write_holding_register(request.address, request.value),
        RegisterType::InputRegister => {
            device
                .register_map
                .input_registers
                .insert(request.address, request.value);
        }
    }

    let event = RegisterValueEvent {
        connection_id: request.connection_id,
        changes: vec![RegisterChangePayload {
            slave_id: request.slave_id,
            register_type: register_type_to_str(reg_type),
            address: request.address,
            value: request.value,
        }],
    };
    app_handle
        .emit("register-value-changed", event)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn list_registers(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
) -> Result<Vec<RegisterDef>, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let devices = conn.connection.devices.read().await;
    let device = devices
        .get(&slave_id)
        .ok_or_else(|| format!("slave {} not found", slave_id))?;

    Ok(device.register_defs.clone())
}

#[tauri::command]
pub async fn export_registers(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
) -> Result<String, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let devices = conn.connection.devices.read().await;
    let device = devices
        .get(&slave_id)
        .ok_or_else(|| format!("slave {} not found", slave_id))?;

    serde_json::to_string_pretty(&device.register_defs)
        .map_err(|e| format!("failed to serialize: {}", e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImportRegistersRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub registers: Vec<RegisterDef>,
}

#[tauri::command]
pub async fn import_registers(
    state: State<'_, AppState>,
    request: ImportRegistersRequest,
) -> Result<usize, String> {
    let connections = state.slave_connections.write().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;

    let count = request.registers.len();
    let mut prospective = device.register_defs.clone();
    prospective.extend(request.registers.iter().cloned());
    validate_register_definition_set(&prospective)?;
    for reg in request.registers {
        device.register_map.ensure_from_def(&reg);
        device.register_defs.push(reg);
    }

    Ok(count)
}

// ---------------------------------------------------------------------------
// Log Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_communication_logs(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<LogEntry>, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;
    Ok(log_helpers::get_all_logs(&conn.log_collector).await)
}

#[tauri::command]
pub async fn clear_communication_logs(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;
    log_helpers::clear_logs(&conn.log_collector).await;
    Ok(())
}

#[tauri::command]
pub async fn export_logs_csv(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<String, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;
    Ok(log_helpers::export_csv(&conn.log_collector).await)
}

// ---------------------------------------------------------------------------
// Tool Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddressConversionRequest {
    pub address: u32,
    pub address_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AddressConversionResult {
    pub plc_address: u32,
    pub protocol_address: u16,
    pub register_type: String,
}

#[tauri::command]
pub fn convert_plc_to_modbus(
    request: AddressConversionRequest,
) -> Result<AddressConversionResult, String> {
    let addr = tools::plc_to_modbus_address(request.address).map_err(|e| format!("{}", e))?;

    Ok(AddressConversionResult {
        plc_address: request.address,
        protocol_address: addr.address,
        register_type: format!("{:?}", addr.address_type).to_lowercase(),
    })
}

#[tauri::command]
pub fn convert_modbus_to_plc(address: u16, register_type: String) -> Result<u32, String> {
    let reg_type = match register_type.as_str() {
        "coil" => tools::ModbusAddressType::Coil,
        "discrete_input" => tools::ModbusAddressType::DiscreteInput,
        "input_register" => tools::ModbusAddressType::InputRegister,
        "holding_register" => tools::ModbusAddressType::HoldingRegister,
        _ => return Err(format!("unknown register type: {}", register_type)),
    };

    Ok(tools::modbus_to_plc_address(address, reg_type))
}

#[tauri::command]
pub fn calculate_crc16(data: String) -> Result<String, String> {
    let bytes = tools::parse_hex_string(&data).map_err(|e| format!("{}", e))?;
    let crc = tools::crc16(&bytes);
    Ok(format!("{:04X}", crc))
}

#[tauri::command]
pub fn calculate_lrc(data: String) -> Result<String, String> {
    let bytes = tools::parse_hex_string(&data).map_err(|e| format!("{}", e))?;
    let lrc = tools::lrc(&bytes);
    Ok(format!("{:02X}", lrc))
}

#[tauri::command]
pub fn parse_hex(data: String) -> Result<Vec<u8>, String> {
    tools::parse_hex_string(&data).map_err(|e| format!("{}", e))
}

// ---------------------------------------------------------------------------
// State Persistence Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PersistedSlaveConnection {
    pub bind_address: String,
    pub port: u16,
    pub devices: Vec<PersistedDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PersistedDevice {
    pub slave_id: u8,
    pub name: String,
    pub registers: Vec<RegisterDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PersistedAppState {
    pub version: u32,
    pub slave_connections: Vec<PersistedSlaveConnection>,
}

#[tauri::command]
pub async fn export_app_state(state: State<'_, AppState>) -> Result<String, String> {
    let connections = state.slave_connections.read().await;

    let mut persisted_connections = Vec::new();

    for (_id, conn_state) in connections.iter() {
        let devices = conn_state.connection.devices.read().await;
        let mut persisted_devices = Vec::new();

        for (_slave_id, device) in devices.iter() {
            persisted_devices.push(PersistedDevice {
                slave_id: device.slave_id,
                name: device.name.clone(),
                registers: device.register_defs.clone(),
            });
        }

        let (bind_address, port) = match &conn_state.connection.transport {
            Transport::Tcp { host, port }
            | Transport::TcpTls { host, port }
            | Transport::RtuOverTcp { host, port } => (host.clone(), *port),
            Transport::Rtu(sc) | Transport::Ascii(sc) => (sc.port.clone(), 0),
        };
        persisted_connections.push(PersistedSlaveConnection {
            bind_address,
            port,
            devices: persisted_devices,
        });
    }

    let app_state = PersistedAppState {
        version: 1,
        slave_connections: persisted_connections,
    };

    serde_json::to_string_pretty(&app_state).map_err(|e| format!("failed to serialize: {}", e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PersistedAppStateInput {
    pub version: u32,
    pub slave_connections: Vec<PersistedSlaveConnection>,
}

#[tauri::command]
pub async fn import_app_state(
    state: State<'_, AppState>,
    input: PersistedAppStateInput,
) -> Result<usize, String> {
    if input.version != 1 {
        return Err(format!("unsupported state version: {}", input.version));
    }

    let mut total_devices = 0;

    for conn_input in input.slave_connections {
        let id = {
            let mut counter = state.next_slave_id.write().await;
            let id = format!("slave_{}", *counter);
            *counter += 1;
            id
        };

        let transport = Transport::Tcp {
            host: conn_input.bind_address.clone(),
            port: conn_input.port,
        };

        let log_collector = Arc::new(LogCollector::new());
        let connection = SlaveConnection::new(transport);
        let connection = connection.with_log_collector(log_collector.clone());

        // Add devices
        for device_input in conn_input.devices {
            let mut device = SlaveDevice::new(device_input.slave_id, device_input.name.clone());
            validate_register_definition_set(&device_input.registers)?;

            // Add registers
            for reg in device_input.registers {
                device.register_map.ensure_from_def(&reg);
                device.register_defs.push(reg);
            }

            let _ = connection.add_device(device).await;
            total_devices += 1;
        }

        state.slave_connections.write().await.insert(
            id,
            SlaveConnectionState {
                connection,
                log_collector,
            },
        );
    }

    Ok(total_devices)
}

#[tauri::command]
pub async fn clear_app_state(state: State<'_, AppState>) -> Result<(), String> {
    state.slave_connections.write().await.clear();
    state.mutation_runtime.write().await.clear();
    state.data_sources.write().await.clear();
    state.mutation_running.store(false, Ordering::Relaxed);
    *state.next_slave_id.write().await = 0;
    Ok(())
}

/// Map a core `MutationMode` to its frontend string token.
fn mutation_mode_str(mode: MutationMode) -> &'static str {
    match mode {
        MutationMode::Flip => "flip",
        MutationMode::Increment => "increment",
        MutationMode::Decrement => "decrement",
        MutationMode::Random => "random",
    }
}

fn normalize_mutation_config(
    register_type: RegisterType,
    config: &mut MutationConfig,
) -> Result<(), String> {
    if matches!(
        register_type,
        RegisterType::Coil | RegisterType::DiscreteInput
    ) {
        // Bit areas always flip; numeric mutation fields are intentionally ignored.
        config.mode = MutationMode::Flip;
    } else {
        if !config.min.is_finite() || !config.max.is_finite() || !config.step.is_finite() {
            return Err("mutation min, max and step must be finite".to_string());
        }
        if config.min > config.max {
            return Err("mutation min must not exceed max".to_string());
        }
        if matches!(
            config.mode,
            MutationMode::Increment | MutationMode::Decrement
        ) && config.step <= 0.0
        {
            return Err("mutation step must be greater than zero".to_string());
        }
    }
    config.period_ms = config.period_ms.max(MUTATION_BASE_TICK_MS);
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SetPointMutationRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub register_type: String,
    pub address: u16,
    pub config: MutationConfig,
}

/// Set (or update) the mutation config for a single register point.
#[tauri::command]
pub async fn set_point_mutation(
    state: State<'_, AppState>,
    request: SetPointMutationRequest,
) -> Result<(), String> {
    let register_type = parse_register_type(&request.register_type)?;
    let mut config = request.config;
    normalize_mutation_config(register_type, &mut config)?;
    let runtime = MutationRuntimeState::new(config.mode, config.period_ms);
    let key = MutationKey::new(
        request.connection_id.clone(),
        request.slave_id,
        register_type,
        request.address,
    );
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;
    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;
    let def = device
        .register_defs
        .iter_mut()
        .find(|d| d.register_type == register_type && d.address == request.address)
        .ok_or_else(|| {
            format!(
                "register {}@{} not found",
                request.register_type, request.address
            )
        })?;
    def.mutation = Some(config);
    state.mutation_runtime.write().await.insert(key, runtime);
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClearPointMutationRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub register_type: String,
    pub address: u16,
}

/// Disable mutation for a single register point.
#[tauri::command]
pub async fn clear_point_mutation(
    state: State<'_, AppState>,
    request: ClearPointMutationRequest,
) -> Result<(), String> {
    let register_type = parse_register_type(&request.register_type)?;
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;
    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;
    let def = device
        .register_defs
        .iter_mut()
        .find(|d| d.register_type == register_type && d.address == request.address)
        .ok_or_else(|| {
            format!(
                "register {}@{} not found",
                request.register_type, request.address
            )
        })?;
    def.mutation = None;
    state
        .mutation_runtime
        .write()
        .await
        .remove(&MutationKey::new(
            request.connection_id,
            request.slave_id,
            register_type,
            request.address,
        ));
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PointMutationInfo {
    pub register_type: String,
    pub address: u16,
    pub mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListPointMutationsRequest {
    pub connection_id: String,
    pub slave_id: u8,
}

/// List the points that currently have mutation enabled, with their mode.
#[tauri::command]
pub async fn list_point_mutations(
    state: State<'_, AppState>,
    request: ListPointMutationsRequest,
) -> Result<Vec<PointMutationInfo>, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;
    let devices = conn.connection.devices.read().await;
    let device = devices
        .get(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;
    let list = device
        .register_defs
        .iter()
        .filter_map(|d| {
            d.mutation
                .as_ref()
                .filter(|c| c.enabled)
                .map(|c| PointMutationInfo {
                    register_type: register_type_to_str(d.register_type).to_string(),
                    address: d.address,
                    mode: mutation_mode_str(c.mode).to_string(),
                })
        })
        .collect();
    Ok(list)
}

/// Master switch: start/stop point mutation.
#[tauri::command]
pub async fn set_mutation_running(state: State<'_, AppState>, running: bool) -> Result<(), String> {
    state.mutation_running.store(running, Ordering::Relaxed);
    Ok(())
}

// ---------------------------------------------------------------------------
// Project File Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn save_project_file(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let mut proj = ProjectFile::new_slave();

    for (id, conn_state) in connections.iter() {
        let conn = &conn_state.connection;
        let (name, proj_transport) = match &conn.transport {
            Transport::Tcp { host, port } => (
                format!("{}:{}", host, port),
                project::TransportConfig::Tcp {
                    host: host.clone(),
                    port: *port,
                },
            ),
            Transport::RtuOverTcp { host, port } => (
                format!("rtu-tcp://{}:{}", host, port),
                project::TransportConfig::RtuOverTcp {
                    host: host.clone(),
                    port: *port,
                },
            ),
            Transport::Rtu(sc) => (
                format!("rtu://{}", sc.port),
                project::TransportConfig::Rtu {
                    port: sc.port.clone(),
                    baud_rate: sc.baud_rate,
                    data_bits: sc.data_bits,
                    stop_bits: sc.stop_bits,
                    parity: format!("{:?}", sc.parity).to_lowercase(),
                },
            ),
            Transport::Ascii(sc) => (
                format!("ascii://{}", sc.port),
                project::TransportConfig::Ascii {
                    port: sc.port.clone(),
                    baud_rate: sc.baud_rate,
                    data_bits: sc.data_bits,
                    stop_bits: sc.stop_bits,
                    parity: format!("{:?}", sc.parity).to_lowercase(),
                },
            ),
            Transport::TcpTls { host, port } => (
                format!("tls://{}:{}", host, port),
                project::TransportConfig::Tcp {
                    host: host.clone(),
                    port: *port,
                },
            ),
        };
        let devices = conn.devices.read().await;
        let conn_config = project::ConnectionConfig {
            id: id.clone(),
            name,
            transport: proj_transport,
            devices: devices
                .values()
                .map(|device| project::DeviceConfig {
                    slave_id: device.slave_id,
                    name: device.name.clone(),
                    register_defs: device.register_defs.clone(),
                    registers: project::RegistersConfig::default(),
                })
                .collect(),
            scan_groups: vec![],
        };
        proj.connections.push(conn_config);
    }

    project::save_project(&proj, std::path::Path::new(&path))
}

fn transport_from_project(config: project::TransportConfig) -> Transport {
    match config {
        project::TransportConfig::Tcp { host, port } => Transport::Tcp { host, port },
        project::TransportConfig::RtuOverTcp { host, port } => Transport::RtuOverTcp { host, port },
        project::TransportConfig::Rtu {
            port,
            baud_rate,
            data_bits,
            stop_bits,
            parity,
        } => Transport::Rtu(SerialConfig {
            port,
            baud_rate,
            data_bits,
            stop_bits,
            parity: parse_parity(&parity),
        }),
        project::TransportConfig::Ascii {
            port,
            baud_rate,
            data_bits,
            stop_bits,
            parity,
        } => Transport::Ascii(SerialConfig {
            port,
            baud_rate,
            data_bits,
            stop_bits,
            parity: parse_parity(&parity),
        }),
    }
}

fn legacy_register_defs(registers: project::RegistersConfig) -> Result<Vec<RegisterDef>, String> {
    let mut defs = Vec::new();
    let groups = [
        (RegisterType::Coil, registers.coils),
        (RegisterType::DiscreteInput, registers.discrete_inputs),
        (RegisterType::HoldingRegister, registers.holding),
        (RegisterType::InputRegister, registers.input),
    ];
    for (register_type, blocks) in groups {
        for block in blocks {
            let data_type = match block.data_type.as_deref() {
                Some(value) => parse_data_type(value)?,
                None if matches!(
                    register_type,
                    RegisterType::Coil | RegisterType::DiscreteInput
                ) =>
                {
                    modbussim_core::register::DataType::Bool
                }
                None => modbussim_core::register::DataType::UInt16,
            };
            let endian = match block.endian.as_deref() {
                Some(value) => parse_endian(value)?,
                None => Endian::Big,
            };
            let stride = if matches!(
                register_type,
                RegisterType::Coil | RegisterType::DiscreteInput
            ) {
                1
            } else {
                data_type.register_count()
            };
            if block.count % stride != 0 {
                return Err(format!(
                    "legacy register block at {} has count {} which is not divisible by data width {}",
                    block.address, block.count, stride
                ));
            }
            for offset in (0..block.count).step_by(stride as usize) {
                let address = block
                    .address
                    .checked_add(offset)
                    .ok_or_else(|| "project register address overflow".to_string())?;
                let name = block
                    .names
                    .get(&address.to_string())
                    .or_else(|| block.names.get(&offset.to_string()))
                    .cloned()
                    .unwrap_or_default();
                defs.push(RegisterDef {
                    address,
                    register_type,
                    data_type,
                    endian,
                    name,
                    comment: String::new(),
                    mutation: None,
                });
            }
        }
    }
    Ok(defs)
}

#[tauri::command]
pub async fn load_project_file(state: State<'_, AppState>, path: String) -> Result<usize, String> {
    let project = project::load_project(std::path::Path::new(&path))?;
    if project.project_type != project::ProjectType::Slave {
        return Err("project is not a slave project".to_string());
    }

    let mut loaded = HashMap::new();
    let mut total_devices = 0;
    for config in project.connections {
        if loaded.contains_key(&config.id) {
            return Err(format!("duplicate connection id {}", config.id));
        }
        let log_collector = Arc::new(LogCollector::new());
        let connection = SlaveConnection::new(transport_from_project(config.transport))
            .with_log_collector(log_collector.clone());
        for device_config in config.devices {
            let mut device = SlaveDevice::new(device_config.slave_id, device_config.name);
            let defs = if device_config.register_defs.is_empty() {
                legacy_register_defs(device_config.registers)?
            } else {
                device_config.register_defs
            };
            validate_register_definition_set(&defs)?;
            for def in defs {
                device.register_map.ensure_from_def(&def);
                device.register_defs.push(def);
            }
            connection
                .add_device(device)
                .await
                .map_err(|e| format!("failed to load device: {}", e))?;
            total_devices += 1;
        }
        loaded.insert(
            config.id,
            SlaveConnectionState {
                connection,
                log_collector,
            },
        );
    }

    let mut current = state.slave_connections.write().await;
    for connection in current.values_mut() {
        connection
            .connection
            .stop()
            .await
            .map_err(|e| format!("failed to stop current connection: {}", e))?;
    }
    *current = loaded;
    let next_id = current
        .keys()
        .filter_map(|id| id.strip_prefix("slave_")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    drop(current);
    state.mutation_runtime.write().await.clear();
    state.data_sources.write().await.clear();
    state.mutation_running.store(false, Ordering::Relaxed);
    *state.next_slave_id.write().await = next_id;
    Ok(total_devices)
}

// ---------------------------------------------------------------------------
// Serial Port Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_serial_ports() -> Vec<transport::SerialPortInfo> {
    transport::list_serial_ports()
}

// ---------------------------------------------------------------------------
// Data Source Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SetDataSourceRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub register_type: String,
    pub address: u16,
    pub source: DataSource,
    pub update_interval_ms: u64,
}

#[tauri::command]
pub async fn set_data_source(
    state: State<'_, AppState>,
    request: SetDataSourceRequest,
) -> Result<(), String> {
    let config = DataSourceConfig {
        source: request.source,
        update_interval_ms: request.update_interval_ms,
    };
    let key = format!(
        "{}:{}:{}:{}",
        request.connection_id, request.slave_id, request.register_type, request.address
    );
    let mut data_sources = state.data_sources.write().await;
    data_sources.insert(key, DataSourceState::new(config));
    Ok(())
}

#[tauri::command]
pub async fn remove_data_source(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
    register_type: String,
    address: u16,
) -> Result<(), String> {
    let key = format!(
        "{}:{}:{}:{}",
        connection_id, slave_id, register_type, address
    );
    let mut data_sources = state.data_sources.write().await;
    data_sources.remove(&key);
    Ok(())
}

#[tauri::command]
pub async fn list_data_sources(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let data_sources = state.data_sources.read().await;
    let prefix = format!("{}:", connection_id);
    Ok(data_sources
        .iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .map(|(k, ds)| serde_json::json!({ "key": k, "config": ds.config }))
        .collect())
}

#[tauri::command]
pub async fn start_data_source_runner(state: State<'_, AppState>) -> Result<(), String> {
    let data_sources = state.data_sources.clone();
    let connections = state.slave_connections.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            interval.tick().await;

            let mut ds = data_sources.write().await;
            let conns = connections.read().await;

            for (key, source_state) in ds.iter_mut() {
                let parts: Vec<&str> = key.splitn(4, ':').collect();
                if parts.len() != 4 {
                    continue;
                }
                let conn_id = parts[0];
                let slave_id: u8 = match parts[1].parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let reg_type = parts[2];
                let address: u16 = match parts[3].parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let value = source_state.next_value();

                if let Some(conn) = conns.get(conn_id) {
                    let mut devices = conn.connection.devices.write().await;
                    if let Some(device) = devices.get_mut(&slave_id) {
                        match reg_type {
                            "holding_register" => {
                                device.register_map.holding_registers.insert(address, value);
                                device.register_map.input_registers.insert(address, value);
                            }
                            "coil" => {
                                device.register_map.coils.insert(address, value != 0);
                                device
                                    .register_map
                                    .discrete_inputs
                                    .insert(address, value != 0);
                            }
                            "input_register" => {
                                device.register_map.input_registers.insert(address, value);
                            }
                            "discrete_input" => {
                                device
                                    .register_map
                                    .discrete_inputs
                                    .insert(address, value != 0);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use modbussim_core::project::{RegisterBlockConfig, RegistersConfig};
    use modbussim_core::register::DataType;

    #[test]
    fn bit_mutation_ignores_numeric_fields_and_normalizes_mode() {
        let mut config = MutationConfig {
            enabled: true,
            mode: MutationMode::Increment,
            period_ms: 1,
            step: -1.0,
            min: 10.0,
            max: 0.0,
        };

        normalize_mutation_config(RegisterType::Coil, &mut config).unwrap();
        assert_eq!(config.mode, MutationMode::Flip);
        assert_eq!(config.period_ms, MUTATION_BASE_TICK_MS);
    }

    #[test]
    fn legacy_wide_register_blocks_advance_by_data_width() {
        let registers = RegistersConfig {
            holding: vec![RegisterBlockConfig {
                address: 10,
                count: 4,
                data_type: Some("float32".to_string()),
                endian: Some("big".to_string()),
                values: Vec::new(),
                names: HashMap::new(),
            }],
            ..Default::default()
        };

        let defs = legacy_register_defs(registers).unwrap();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].address, 10);
        assert_eq!(defs[1].address, 12);
        assert!(defs.iter().all(|def| def.data_type == DataType::Float32));
        validate_register_definition_set(&defs).unwrap();
    }

    #[test]
    fn legacy_wide_register_blocks_reject_partial_values() {
        let registers = RegistersConfig {
            input: vec![RegisterBlockConfig {
                address: 20,
                count: 3,
                data_type: Some("uint32".to_string()),
                endian: None,
                values: Vec::new(),
                names: HashMap::new(),
            }],
            ..Default::default()
        };

        assert!(legacy_register_defs(registers).is_err());
    }
}
