//! Single background scheduler for persisted per-point data sources.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use modbussim_core::parse::register_type_to_str;
use modbussim_core::register::RegisterType;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tokio::time::MissedTickBehavior;

use crate::commands::{RegisterChangePayload, RegisterValueEvent};
use crate::state::{DataSourceKey, SlaveConnectionState};
use modbussim_core::data_source::DataSourceState;

pub fn spawn_data_source_tick(
    app_handle: AppHandle,
    slave_connections: Arc<RwLock<HashMap<String, SlaveConnectionState>>>,
    data_sources: Arc<RwLock<HashMap<DataSourceKey, DataSourceState>>>,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_millis(100));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let now = Instant::now();
            let due = {
                let mut sources = data_sources.write().await;
                sources
                    .iter_mut()
                    .filter_map(|(key, source)| {
                        if !source.is_due(now) {
                            return None;
                        }
                        let value = source.next_value();
                        source.mark_updated(now);
                        Some((key.clone(), value))
                    })
                    .collect::<Vec<_>>()
            };

            let mut events: HashMap<String, Vec<RegisterChangePayload>> = HashMap::new();
            for (key, value) in due {
                let connections = slave_connections.read().await;
                let Some(connection) = connections.get(&key.connection_id) else {
                    continue;
                };
                let mut devices = connection.connection.devices.write().await;
                let Some(device) = devices.get_mut(&key.slave_id) else {
                    continue;
                };
                let updated = match key.register_type {
                    RegisterType::Coil => device
                        .register_map
                        .coils
                        .get_mut(&key.address)
                        .map(|slot| *slot = value != 0)
                        .is_some(),
                    RegisterType::DiscreteInput => device
                        .register_map
                        .discrete_inputs
                        .get_mut(&key.address)
                        .map(|slot| *slot = value != 0)
                        .is_some(),
                    RegisterType::HoldingRegister => device
                        .register_map
                        .holding_registers
                        .get_mut(&key.address)
                        .map(|slot| *slot = value)
                        .is_some(),
                    RegisterType::InputRegister => device
                        .register_map
                        .input_registers
                        .get_mut(&key.address)
                        .map(|slot| *slot = value)
                        .is_some(),
                };
                if updated {
                    events.entry(key.connection_id.clone()).or_default().push(
                        RegisterChangePayload {
                            slave_id: key.slave_id,
                            register_type: register_type_to_str(key.register_type),
                            address: key.address,
                            value,
                        },
                    );
                }
            }

            for (connection_id, changes) in events {
                let _ = app_handle.emit(
                    "register-value-changed",
                    RegisterValueEvent {
                        connection_id,
                        changes,
                    },
                );
            }
        }
    });
}
