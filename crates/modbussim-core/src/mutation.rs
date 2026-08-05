//! Per-point mutation: persisted configuration and pure value-stepping logic.
//!
//! `MutationConfig` lives on each `RegisterDef`. Scheduling state (next due
//! time and triangle-wave direction) is deliberately kept outside the
//! persisted model by the application layer.

use crate::register::{
    decode_value, encode_value, DataType, RegisterDef, RegisterMap, RegisterType,
};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// How a point's value changes on each mutation tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MutationMode {
    /// Two-state toggle. bool: invert. numeric: `value <= (min+max)/2 ? max : min`.
    #[default]
    Flip,
    /// Start by increasing; reverse direction at either bound (triangle wave).
    Increment,
    /// Start by decreasing; reverse direction at either bound (triangle wave).
    Decrement,
    /// Uniform random value in `[min, max]`.
    Random,
}

/// Non-persisted direction for increment/decrement triangle waves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationDirection {
    Up,
    Down,
}

impl MutationDirection {
    pub fn initial_for(mode: MutationMode) -> Self {
        match mode {
            MutationMode::Decrement => Self::Down,
            _ => Self::Up,
        }
    }
}

pub const DEFAULT_PERIOD_MS: u64 = 1_000;

fn default_period_ms() -> u64 {
    DEFAULT_PERIOD_MS
}

/// Per-point mutation configuration, persisted on `RegisterDef`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MutationConfig {
    pub enabled: bool,
    pub mode: MutationMode,
    /// Independent cadence for this point. The app clamps it to its base tick.
    #[serde(default = "default_period_ms")]
    pub period_ms: u64,
    /// Step size for Increment/Decrement (engineering-value units).
    pub step: f64,
    /// Lower bound (engineering-value units).
    pub min: f64,
    /// Upper bound (engineering-value units).
    pub max: f64,
}

impl Default for MutationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: MutationMode::Flip,
            period_ms: DEFAULT_PERIOD_MS,
            step: 1.0,
            min: 0.0,
            max: 100.0,
        }
    }
}

/// Quantize a raw engineering value to what `data_type` can represent:
/// rounds integer types and clamps to their value range; passes floats through.
fn quantize(value: f64, data_type: DataType) -> f64 {
    match data_type {
        DataType::Bool => {
            if value != 0.0 {
                1.0
            } else {
                0.0
            }
        }
        DataType::UInt16 => value.round().clamp(0.0, u16::MAX as f64),
        DataType::Int16 => value.round().clamp(i16::MIN as f64, i16::MAX as f64),
        DataType::UInt32 => value.round().clamp(0.0, u32::MAX as f64),
        DataType::Int32 => value.round().clamp(i32::MIN as f64, i32::MAX as f64),
        DataType::Float32 => {
            if value.is_nan() {
                0.0
            } else {
                value.clamp(f32::MIN as f64, f32::MAX as f64)
            }
        }
    }
}

/// Compute the next engineering value and triangle-wave direction.
pub fn compute_next_value(
    current: f64,
    data_type: DataType,
    cfg: &MutationConfig,
    direction: MutationDirection,
    rng: &mut impl Rng,
) -> (f64, MutationDirection) {
    let lo = cfg.min.min(cfg.max);
    let hi = cfg.min.max(cfg.max);
    let step = cfg.step.abs();
    let (next, next_direction) = match cfg.mode {
        MutationMode::Flip => {
            if current <= (lo + hi) / 2.0 {
                (hi, direction)
            } else {
                (lo, direction)
            }
        }
        MutationMode::Increment | MutationMode::Decrement => {
            if hi <= lo || step == 0.0 {
                (lo, direction)
            } else {
                match direction {
                    MutationDirection::Up => {
                        let candidate = current.max(lo) + step;
                        if candidate >= hi {
                            (hi, MutationDirection::Down)
                        } else {
                            (candidate, MutationDirection::Up)
                        }
                    }
                    MutationDirection::Down => {
                        let candidate = current.min(hi) - step;
                        if candidate <= lo {
                            (lo, MutationDirection::Up)
                        } else {
                            (candidate, MutationDirection::Down)
                        }
                    }
                }
            }
        }
        MutationMode::Random => {
            if hi <= lo {
                (lo, direction)
            } else {
                (rng.gen_range(lo..=hi), direction)
            }
        }
    };
    (quantize(next, data_type), next_direction)
}

/// Apply one mutation tick to a single point in `map`.
/// bool points (Coil/DiscreteInput) invert; numeric points decode by
/// `data_type`/`endian`, step in engineering-value space, then encode back.
pub fn apply_point_mutation(
    map: &mut RegisterMap,
    def: &RegisterDef,
    cfg: &MutationConfig,
    direction: MutationDirection,
    rng: &mut impl Rng,
) -> MutationDirection {
    match def.register_type {
        RegisterType::Coil => {
            let cur = map.coils.get(&def.address).copied().unwrap_or(false);
            map.coils.insert(def.address, !cur);
            direction
        }
        RegisterType::DiscreteInput => {
            let cur = map
                .discrete_inputs
                .get(&def.address)
                .copied()
                .unwrap_or(false);
            map.discrete_inputs.insert(def.address, !cur);
            direction
        }
        RegisterType::HoldingRegister | RegisterType::InputRegister => {
            let is_holding = def.register_type == RegisterType::HoldingRegister;
            let count = def.data_type.register_count();
            let raw: Vec<u16> = (0..count)
                .map(|i| {
                    let addr = def.address.wrapping_add(i);
                    if is_holding {
                        map.holding_registers.get(&addr).copied().unwrap_or(0)
                    } else {
                        map.input_registers.get(&addr).copied().unwrap_or(0)
                    }
                })
                .collect();
            let current = decode_value(&raw, def.data_type, def.endian).unwrap_or(0.0);
            let (next, next_direction) =
                compute_next_value(current, def.data_type, cfg, direction, rng);
            if let Ok(encoded) = encode_value(next, def.data_type, def.endian) {
                for (i, &w) in encoded.iter().enumerate() {
                    let addr = def.address.wrapping_add(i as u16);
                    if is_holding {
                        map.holding_registers.insert(addr, w);
                    } else {
                        map.input_registers.insert(addr, w);
                    }
                }
            }
            next_direction
        }
    }
}

/// Convenience wrapper using the thread-local RNG (used by the app tick task,
/// which keeps `rand` out of the app crate to avoid version-mismatch issues).
pub fn apply_point_mutation_thread(
    map: &mut RegisterMap,
    def: &RegisterDef,
    cfg: &MutationConfig,
    direction: MutationDirection,
) -> MutationDirection {
    let mut rng = rand::thread_rng();
    apply_point_mutation(map, def, cfg, direction, &mut rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register::{DataType, Endian, RegisterDef, RegisterMap, RegisterType};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn def(reg_type: RegisterType, data_type: DataType, address: u16) -> RegisterDef {
        RegisterDef {
            address,
            register_type: reg_type,
            data_type,
            endian: Endian::Big,
            name: String::new(),
            comment: String::new(),
            mutation: None,
            data_source: None,
        }
    }

    fn cfg(mode: MutationMode, step: f64, min: f64, max: f64) -> MutationConfig {
        MutationConfig {
            enabled: true,
            mode,
            period_ms: 1_000,
            step,
            min,
            max,
        }
    }

    #[test]
    fn config_json_roundtrip() {
        let c = cfg(MutationMode::Increment, 2.5, -10.0, 10.0);
        let s = serde_json::to_string(&c).unwrap();
        let back: MutationConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn coil_point_flips_each_tick() {
        let mut map = RegisterMap::new();
        map.write_coil(5, false);
        let d = def(RegisterType::Coil, DataType::Bool, 5);
        let c = cfg(MutationMode::Flip, 1.0, 0.0, 1.0);
        let mut rng = StdRng::seed_from_u64(1);
        apply_point_mutation(&mut map, &d, &c, MutationDirection::Up, &mut rng);
        assert_eq!(map.coils.get(&5), Some(&true));
        apply_point_mutation(&mut map, &d, &c, MutationDirection::Up, &mut rng);
        assert_eq!(map.coils.get(&5), Some(&false));
    }

    #[test]
    fn holding_flip_two_state_toggle() {
        let mut map = RegisterMap::new();
        map.write_holding_register(0, 0);
        let d = def(RegisterType::HoldingRegister, DataType::UInt16, 0);
        let c = cfg(MutationMode::Flip, 0.0, 10.0, 90.0);
        let mut rng = StdRng::seed_from_u64(1);
        // current 0 <= 50 -> goes to max (90)
        apply_point_mutation(&mut map, &d, &c, MutationDirection::Up, &mut rng);
        assert_eq!(map.holding_registers.get(&0), Some(&90));
        // current 90 > 50 -> goes to min (10)
        apply_point_mutation(&mut map, &d, &c, MutationDirection::Up, &mut rng);
        assert_eq!(map.holding_registers.get(&0), Some(&10));
    }

    #[test]
    fn increment_reverses_at_bounds() {
        let c = cfg(MutationMode::Increment, 4.0, 0.0, 10.0);
        let mut rng = StdRng::seed_from_u64(1);
        // 0 -> 4 -> 8 -> 10, then reverse -> 6 -> 2 -> 0.
        let mut v = 0.0;
        let mut direction = MutationDirection::Up;
        let seq: Vec<f64> = (0..6)
            .map(|_| {
                (v, direction) = compute_next_value(v, DataType::UInt16, &c, direction, &mut rng);
                v
            })
            .collect();
        assert_eq!(seq, vec![4.0, 8.0, 10.0, 6.0, 2.0, 0.0]);
        assert_eq!(direction, MutationDirection::Up);
    }

    #[test]
    fn decrement_reverses_at_bounds() {
        let c = cfg(MutationMode::Decrement, 3.0, 0.0, 10.0);
        let mut rng = StdRng::seed_from_u64(1);
        // 10 -> 7 -> 4 -> 1 -> 0, then reverse -> 3.
        let mut v = 10.0;
        let mut direction = MutationDirection::Down;
        let seq: Vec<f64> = (0..5)
            .map(|_| {
                (v, direction) = compute_next_value(v, DataType::UInt16, &c, direction, &mut rng);
                v
            })
            .collect();
        assert_eq!(seq, vec![7.0, 4.0, 1.0, 0.0, 3.0]);
        assert_eq!(direction, MutationDirection::Up);
    }

    #[test]
    fn random_stays_in_range() {
        let c = cfg(MutationMode::Random, 0.0, 100.0, 200.0);
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..50 {
            let (v, _) =
                compute_next_value(0.0, DataType::UInt16, &c, MutationDirection::Up, &mut rng);
            assert!((100.0..=200.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn float32_increment_keeps_valid_encoding() {
        let mut map = RegisterMap::new();
        let seed = encode_value(1.0, DataType::Float32, Endian::Big).unwrap();
        map.write_holding_register(10, seed[0]);
        map.write_holding_register(11, seed[1]);
        let d = def(RegisterType::HoldingRegister, DataType::Float32, 10);
        let c = cfg(MutationMode::Increment, 0.5, 0.0, 5.0);
        let mut rng = StdRng::seed_from_u64(1);
        apply_point_mutation(&mut map, &d, &c, MutationDirection::Up, &mut rng);
        let raw = vec![
            *map.holding_registers.get(&10).unwrap(),
            *map.holding_registers.get(&11).unwrap(),
        ];
        let decoded = decode_value(&raw, DataType::Float32, Endian::Big).unwrap();
        assert!((decoded - 1.5).abs() < 0.001, "expected 1.5 got {decoded}");
    }

    #[test]
    fn compute_clamps_to_uint16_range() {
        let c = cfg(MutationMode::Random, 0.0, -50.0, 200000.0);
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..50 {
            let (v, _) =
                compute_next_value(0.0, DataType::UInt16, &c, MutationDirection::Up, &mut rng);
            assert!((0.0..=65535.0).contains(&v), "uint16 out of range: {v}");
            assert_eq!(v.fract(), 0.0, "uint16 must be integral");
        }
    }

    #[test]
    fn old_config_without_period_uses_default() {
        let json = r#"{"enabled":true,"mode":"random","step":1.0,"min":0.0,"max":10.0}"#;
        let config: MutationConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.period_ms, DEFAULT_PERIOD_MS);
    }
}
