//! Background point-mutation tick task.
//!
//! A single long-lived task wakes every 100 ms. Each enabled point owns an
//! independent due time and triangle-wave direction in `mutation_runtime`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::time::{Duration, Instant, MissedTickBehavior};

use modbussim_core::mutation::apply_point_mutation_thread;

use crate::state::{
    mutation_period, MutationKey, MutationRuntimeState, SlaveConnectionState, MUTATION_BASE_TICK_MS,
};

fn is_due(now: Instant, next_due: Instant) -> bool {
    now >= next_due
}

fn next_due_after(now: Instant, period_ms: u64) -> Instant {
    now + mutation_period(period_ms)
}

/// Spawn the single mutation tick task. Cheap (just a sleep) while the master
/// switch is off.
pub fn spawn_mutation_tick(
    slave_connections: Arc<RwLock<HashMap<String, SlaveConnectionState>>>,
    mutation_running: Arc<AtomicBool>,
    mutation_runtime: Arc<RwLock<HashMap<MutationKey, MutationRuntimeState>>>,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_millis(MUTATION_BASE_TICK_MS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if !mutation_running.load(Ordering::Relaxed) {
                continue;
            }
            let now = Instant::now();
            let due = {
                let mut runtimes = mutation_runtime.write().await;
                runtimes
                    .iter_mut()
                    .filter_map(|(key, runtime)| {
                        if !is_due(now, runtime.next_due) {
                            return None;
                        }
                        runtime.next_due = next_due_after(now, runtime.config.period_ms);
                        Some((
                            key.clone(),
                            runtime.definition.clone(),
                            runtime.config.clone(),
                            runtime.direction,
                        ))
                    })
                    .collect::<Vec<_>>()
            };

            for (key, definition, config, direction) in due {
                let conns = slave_connections.read().await;
                let Some(conn_state) = conns.get(&key.connection_id) else {
                    continue;
                };
                let mut devices = conn_state.connection.devices.write().await;
                let Some(device) = devices.get_mut(&key.slave_id) else {
                    continue;
                };
                let new_direction = apply_point_mutation_thread(
                    &mut device.register_map,
                    &definition,
                    &config,
                    direction,
                );
                drop(devices);
                drop(conns);
                if let Some(runtime) = mutation_runtime.write().await.get_mut(&key) {
                    runtime.direction = new_direction;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_check_is_inclusive() {
        let now = Instant::now();
        assert!(is_due(now, now));
        assert!(!is_due(now, now + Duration::from_millis(1)));
    }

    #[test]
    fn periods_are_independent_and_clamped_to_base_tick() {
        let now = Instant::now();
        assert_eq!(
            next_due_after(now, 20).duration_since(now),
            Duration::from_millis(MUTATION_BASE_TICK_MS)
        );
        assert_eq!(
            next_due_after(now, 750).duration_since(now),
            Duration::from_millis(750)
        );
    }
}
