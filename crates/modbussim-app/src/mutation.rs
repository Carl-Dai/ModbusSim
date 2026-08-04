//! Background point-mutation tick task.
//!
//! A single long-lived task wakes every 100 ms. Each enabled point owns an
//! independent due time and triangle-wave direction in `mutation_runtime`.

use std::collections::{HashMap, HashSet};
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
            let mut active_keys = HashSet::new();
            let conns = slave_connections.read().await;
            for (connection_id, conn_state) in conns.iter() {
                let mut devices = conn_state.connection.devices.write().await;
                for (slave_id, device) in devices.iter_mut() {
                    for def in device.register_defs.iter() {
                        let Some(cfg) = &def.mutation else { continue };
                        if !cfg.enabled {
                            continue;
                        }
                        let key = MutationKey::new(
                            connection_id,
                            *slave_id,
                            def.register_type,
                            def.address,
                        );
                        active_keys.insert(key.clone());

                        let mut runtimes = mutation_runtime.write().await;
                        let runtime = runtimes
                            .entry(key)
                            .or_insert_with(|| MutationRuntimeState::new(cfg.mode, cfg.period_ms));
                        if !is_due(now, runtime.next_due) {
                            continue;
                        }
                        runtime.direction = apply_point_mutation_thread(
                            &mut device.register_map,
                            def,
                            cfg,
                            runtime.direction,
                        );
                        runtime.next_due = next_due_after(now, cfg.period_ms);
                    }
                }
            }
            mutation_runtime
                .write()
                .await
                .retain(|key, _| active_keys.contains(key));
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
