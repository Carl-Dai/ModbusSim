use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DataSource {
    Fixed {
        value: u16,
    },
    Random {
        min: u16,
        max: u16,
    },
    Sine {
        amplitude: f64,
        frequency: f64,
        offset: f64,
        phase: f64,
    },
    Sawtooth {
        min: u16,
        max: u16,
        period_ms: u64,
    },
    Triangle {
        min: u16,
        max: u16,
        period_ms: u64,
    },
    Counter {
        start: u16,
        step: i16,
        wrap: bool,
    },
    CsvPlayback {
        values: Vec<u16>,
        loop_playback: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataSourceConfig {
    pub source: DataSource,
    #[serde(default = "default_update_interval_ms")]
    pub update_interval_ms: u64,
}

impl DataSourceConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.update_interval_ms == 0 {
            return Err("data source update interval must be greater than zero".to_string());
        }
        match &self.source {
            DataSource::Random { min, max }
            | DataSource::Sawtooth { min, max, .. }
            | DataSource::Triangle { min, max, .. }
                if min > max =>
            {
                Err("data source min must not exceed max".to_string())
            }
            DataSource::Sine {
                amplitude,
                frequency,
                offset,
                phase,
            } if !amplitude.is_finite()
                || !frequency.is_finite()
                || !offset.is_finite()
                || !phase.is_finite()
                || *frequency < 0.0 =>
            {
                Err("sine parameters must be finite and frequency must be non-negative".to_string())
            }
            DataSource::Sawtooth { period_ms: 0, .. } => {
                Err("sawtooth period must be greater than zero".to_string())
            }
            DataSource::Triangle { period_ms, .. } if *period_ms < 2 => {
                Err("triangle period must be at least 2 ms".to_string())
            }
            DataSource::CsvPlayback { values, .. } if values.is_empty() => {
                Err("CSV playback requires at least one value".to_string())
            }
            _ => Ok(()),
        }
    }
}

fn default_update_interval_ms() -> u64 {
    1000
}

pub struct DataSourceState {
    pub config: DataSourceConfig,
    pub start_time: Instant,
    pub counter_value: i32,
    pub csv_index: usize,
    pub next_due: Instant,
}

impl DataSourceState {
    pub fn new(config: DataSourceConfig) -> Self {
        let start_value = match &config.source {
            DataSource::Counter { start, .. } => *start as i32,
            _ => 0,
        };
        let next_due = Instant::now() + data_source_period(config.update_interval_ms);
        Self {
            config,
            start_time: Instant::now(),
            counter_value: start_value,
            csv_index: 0,
            next_due,
        }
    }

    pub fn is_due(&self, now: Instant) -> bool {
        now >= self.next_due
    }

    pub fn mark_updated(&mut self, now: Instant) {
        self.next_due = now + data_source_period(self.config.update_interval_ms);
    }

    pub fn next_value(&mut self) -> u16 {
        match &self.config.source.clone() {
            DataSource::Fixed { value } => *value,

            DataSource::Random { min, max } => rand::thread_rng().gen_range(*min..=*max),

            DataSource::Sine {
                amplitude,
                frequency,
                offset,
                phase,
            } => {
                let t = self.start_time.elapsed().as_secs_f64();
                let v =
                    offset + amplitude * (2.0 * std::f64::consts::PI * frequency * t + phase).sin();
                v.clamp(0.0, 65535.0) as u16
            }

            DataSource::Sawtooth {
                min,
                max,
                period_ms,
            } => {
                if *period_ms == 0 {
                    return *min;
                }
                let elapsed_ms = self.start_time.elapsed().as_millis() as u64;
                let pos = elapsed_ms % period_ms;
                let frac = pos as f64 / *period_ms as f64;
                let range = *max as f64 - *min as f64;
                (*min as f64 + frac * range) as u16
            }

            DataSource::Triangle {
                min,
                max,
                period_ms,
            } => {
                if *period_ms == 0 {
                    return *min;
                }
                let elapsed_ms = self.start_time.elapsed().as_millis() as u64;
                let pos = elapsed_ms % period_ms;
                let half = period_ms / 2;
                if half == 0 {
                    return *min;
                }
                let range = *max as f64 - *min as f64;
                if pos < half {
                    let frac = pos as f64 / half as f64;
                    (*min as f64 + frac * range) as u16
                } else {
                    let frac = (pos - half) as f64 / half as f64;
                    (*max as f64 - frac * range) as u16
                }
            }

            DataSource::Counter { step, wrap, .. } => {
                let current = self.counter_value.clamp(0, 65535) as u16;
                let next = self.counter_value + *step as i32;
                if *wrap {
                    self.counter_value = ((next % 65536) + 65536) % 65536;
                } else {
                    self.counter_value = next.clamp(0, 65535);
                }
                current
            }

            DataSource::CsvPlayback {
                values,
                loop_playback,
            } => {
                if values.is_empty() {
                    return 0;
                }
                let idx = self.csv_index.min(values.len() - 1);
                let val = values[idx];
                if *loop_playback {
                    self.csv_index = (self.csv_index + 1) % values.len();
                } else if self.csv_index < values.len() - 1 {
                    self.csv_index += 1;
                }
                val
            }
        }
    }
}

pub fn data_source_period(update_interval_ms: u64) -> Duration {
    Duration::from_millis(update_interval_ms.max(100))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(source: DataSource) -> DataSourceState {
        DataSourceState::new(DataSourceConfig {
            source,
            update_interval_ms: 1000,
        })
    }

    #[test]
    fn test_fixed_source() {
        let mut s = make_state(DataSource::Fixed { value: 42 });
        assert_eq!(s.next_value(), 42);
        assert_eq!(s.next_value(), 42);
        assert_eq!(s.next_value(), 42);
    }

    #[test]
    fn test_random_source_in_range() {
        let mut s = make_state(DataSource::Random { min: 10, max: 20 });
        for _ in 0..100 {
            let v = s.next_value();
            assert!((10..=20).contains(&v));
        }
    }

    #[test]
    fn test_counter_increment() {
        let mut s = make_state(DataSource::Counter {
            start: 0,
            step: 1,
            wrap: false,
        });
        assert_eq!(s.next_value(), 0);
        assert_eq!(s.next_value(), 1);
        assert_eq!(s.next_value(), 2);
    }

    #[test]
    fn test_counter_wrap() {
        // start at 65534, step 2, wrap=true => 65534, 65536%65536=0
        let mut s = make_state(DataSource::Counter {
            start: 65534,
            step: 2,
            wrap: true,
        });
        assert_eq!(s.next_value(), 65534);
        assert_eq!(s.next_value(), 0);
    }

    #[test]
    fn test_counter_no_wrap_clamp() {
        let mut s = make_state(DataSource::Counter {
            start: 65535,
            step: 1,
            wrap: false,
        });
        assert_eq!(s.next_value(), 65535);
        assert_eq!(s.next_value(), 65535);
    }

    #[test]
    fn test_csv_playback_loop() {
        let mut s = make_state(DataSource::CsvPlayback {
            values: vec![10, 20, 30],
            loop_playback: true,
        });
        assert_eq!(s.next_value(), 10);
        assert_eq!(s.next_value(), 20);
        assert_eq!(s.next_value(), 30);
        assert_eq!(s.next_value(), 10); // loops
    }

    #[test]
    fn test_csv_playback_no_loop() {
        let mut s = make_state(DataSource::CsvPlayback {
            values: vec![10, 20, 30],
            loop_playback: false,
        });
        assert_eq!(s.next_value(), 10);
        assert_eq!(s.next_value(), 20);
        assert_eq!(s.next_value(), 30);
        assert_eq!(s.next_value(), 30); // stays at last
    }

    #[test]
    fn test_csv_playback_empty() {
        let mut s = make_state(DataSource::CsvPlayback {
            values: vec![],
            loop_playback: true,
        });
        assert_eq!(s.next_value(), 0);
    }

    #[test]
    fn test_data_source_serde_roundtrip() {
        let cfg = DataSourceConfig {
            source: DataSource::Sine {
                amplitude: 100.0,
                frequency: 1.0,
                offset: 32768.0,
                phase: 0.5,
            },
            update_interval_ms: 500,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let cfg2: DataSourceConfig = serde_json::from_str(&json).unwrap();
        match cfg2.source {
            DataSource::Sine {
                amplitude,
                frequency,
                offset,
                phase,
            } => {
                assert_eq!(amplitude, 100.0);
                assert_eq!(frequency, 1.0);
                assert_eq!(offset, 32768.0);
                assert_eq!(phase, 0.5);
            }
            _ => panic!("wrong variant after roundtrip"),
        }
        assert_eq!(cfg2.update_interval_ms, 500);
    }

    #[test]
    fn test_sawtooth_zero_period() {
        let mut s = make_state(DataSource::Sawtooth {
            min: 5,
            max: 100,
            period_ms: 0,
        });
        assert_eq!(s.next_value(), 5);
    }

    #[test]
    fn validates_invalid_ranges_and_intervals() {
        let config = DataSourceConfig {
            source: DataSource::Random { min: 10, max: 5 },
            update_interval_ms: 100,
        };
        assert!(config.validate().is_err());

        let config = DataSourceConfig {
            source: DataSource::Fixed { value: 1 },
            update_interval_ms: 0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn per_source_due_time_is_respected_and_clamped() {
        let mut state = make_state(DataSource::Fixed { value: 1 });
        let now = Instant::now();
        state.next_due = now;
        assert!(state.is_due(now));
        state.mark_updated(now);
        assert_eq!(
            state.next_due.duration_since(now),
            Duration::from_millis(1000)
        );

        state.config.update_interval_ms = 1;
        state.mark_updated(now);
        assert_eq!(
            state.next_due.duration_since(now),
            Duration::from_millis(100)
        );
    }
}
